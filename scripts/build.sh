#!/usr/bin/env bash
# Nabu production build script — the single entry point for release builds.
#
# Detects the host platform/architecture, validates prerequisites, and
# produces release-ready desktop bundles. Supports:
#   * native builds for the current host arch
#   * universal macOS builds (Intel + Apple Silicon fat binary, via lipo)
#   * explicit --bundles override (app | dmg | deb | rpm | msi | nsis | ...)
#
# Usage:
#   scripts/build.sh                # native release for this machine
#   scripts/build.sh --universal    # macOS universal binary + DMG
#   scripts/build.sh --bundles app  # skip installers, app bundle only
#   scripts/build.sh --help
#
# Exits non-zero with a clear message when prerequisites are missing
# (run scripts/check-env.sh for a full diagnostic).
set -uo pipefail

# Colour helpers (no-op when not a TTY)
if [ -t 1 ]; then
  RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; BOLD=''; RESET=''
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

OS="$(uname -s)"
ARCH="$(uname -m)"
UNIVERSAL=0
BUNDLES=""

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
}

# ── Arguments ───────────────────────────────────────────────────────────────
while [ $# -gt 0 ]; do
  case "$1" in
    --help|-h) usage ;;
    --universal) UNIVERSAL=1 ;;
    --bundles)
      shift
      [ $# -eq 0 ] && { echo "error: --bundles requires a value (e.g. app,dmg)" >&2; exit 1; }
      BUNDLES="$1"
      ;;
    *)
      echo "error: unknown argument '$1' (try --help)" >&2
      exit 1
      ;;
  esac
  shift
done

# ── Prerequisites ───────────────────────────────────────────────────────────
if ! command -v cargo >/dev/null 2>&1; then
  echo "${RED}error: cargo not found. Install the Rust toolchain (rustup or mise).${RESET}" >&2
  exit 1
fi
if ! command -v trunk >/dev/null 2>&1; then
  echo "${RED}error: trunk not found. Install it:  cargo install trunk  (or: mise install).${RESET}" >&2
  echo "       trunk builds the Leptos/WASM frontend before packaging." >&2
  exit 1
fi
if ! cargo tauri --version >/dev/null 2>&1; then
  echo "${RED}error: tauri-cli not found. Install it:  cargo install tauri-cli --version ^2${RESET}" >&2
  exit 1
fi
if [ ! -d node_modules/.bin ]; then
  echo "${RED}error: node_modules/ missing. Run:  npm install${RESET}" >&2
  exit 1
fi
if ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
  echo "${RED}error: wasm32-unknown-unknown target not installed. Run:  rustup target add wasm32-unknown-unknown${RESET}" >&2
  exit 1
fi

# ── Architecture detection ──────────────────────────────────────────────────
host_triple() {
  case "$OS:$ARCH" in
    Darwin:x86_64)   echo x86_64-apple-darwin ;;
    Darwin:arm64)    echo aarch64-apple-darwin ;;
    Linux:x86_64)    echo x86_64-unknown-linux-gnu ;;
    Linux:aarch64)   echo aarch64-unknown-linux-gnu ;;
    MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64) echo x86_64-pc-windows-msvc ;;
    Darwin:*)
      echo "error: unrecognised architecture '$ARCH' on macOS — refusing to guess." >&2
      echo "       Supported: x86_64 (Intel), arm64 (Apple Silicon)." >&2
      exit 1
      ;;
    *)
      echo "error: unsupported platform '$OS' / arch '$ARCH'" >&2
      exit 1
      ;;
  esac
}

if [ "$UNIVERSAL" = 1 ]; then
  if [ "$OS" != "Darwin" ]; then
    echo "${RED}error: --universal is only supported on macOS (host is $OS).${RESET}" >&2
    echo "       Use a plain \`scripts/build.sh\` for native builds on this platform." >&2
    exit 1
  fi
  if ! command -v lipo >/dev/null 2>&1; then
    echo "${RED}error: lipo not found. Install macOS command-line tools:  xcode-select --install${RESET}" >&2
    exit 1
  fi
  for T in x86_64-apple-darwin aarch64-apple-darwin; do
    if ! rustup target list --installed 2>/dev/null | grep -qx "$T"; then
      echo "${RED}error: target '$T' not installed. Run:  rustup target add $T${RESET}" >&2
      exit 1
    fi
  done
  TARGET="universal-apple-darwin"
  echo "${GREEN}==> Building universal macOS bundle (Intel + Apple Silicon)${RESET}"
  # Keep the DMG explicit (matching the historical release script) so the
  # universal path always produces the distributable installer.
  if [ -z "${BUNDLES:-}" ]; then
    BUNDLES="app,dmg"
  fi
else
  TARGET="$(host_triple)"
  echo "${GREEN}==> Building native release for $TARGET${RESET}"
fi

# ── Build ───────────────────────────────────────────────────────────────────
echo "${BOLD}==> Building frontend (Tailwind + trunk/WASM)${RESET}"
npm run css:build
env -u NO_COLOR trunk build --config Trunk.toml --release

echo "${BOLD}==> Building desktop bundles with cargo-tauri${RESET}"
CARGS=(tauri build)
if [ "$UNIVERSAL" = 1 ]; then
  CARGS+=(--target "$TARGET")
fi
if [ -n "${BUNDLES:-}" ]; then
  CARGS+=(--bundles "$BUNDLES")
fi
cargo "${CARGS[@]}"

# ── Report ──────────────────────────────────────────────────────────────────
BUNDLE_ROOT="src-tauri/target"
if [ "$UNIVERSAL" = 1 ]; then
  BUNDLE_ROOT="$BUNDLE_ROOT/universal-apple-darwin/release"
else
  BUNDLE_ROOT="$BUNDLE_ROOT/$TARGET/release"
fi

BUNDLE_DIR="$BUNDLE_ROOT/bundle"

report_artifacts() { # report_artifacts <dir>
  local d="$1"
  if [ ! -d "$d" ]; then
    echo "  (no bundle directory at $d)"
    return
  fi
  # App bundles are directories; installers/binaries are files. List both.
  find "$d" -maxdepth 3 \(
    -type d -name '*.app' -o \
    -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.rpm' -o -name '*.msi' \
               -o -name '*.exe' -o -name '*.AppImage' \)
  \) -print 2>/dev/null | sed 's/^/  /'
}

if [ "$UNIVERSAL" = 1 ]; then
  # Universal build: single bundle dir for the merged binary.
  report_artifacts "$BUNDLE_DIR"
else
  # Native builds can leave artifacts under both the host-triple dir and an
  # arch-agnostic dir; report whichever exists.
  report_artifacts "$BUNDLE_DIR"
  REPORTED=0
  if [ -d "$BUNDLE_DIR" ]; then REPORTED=1; fi
  if [ "$REPORTED" = 0 ] && [ -d "src-tauri/target/release/bundle" ]; then
    echo "  (also checking src-tauri/target/release/bundle)"
    report_artifacts "src-tauri/target/release/bundle"
  fi
fi
