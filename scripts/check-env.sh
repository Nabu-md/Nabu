#!/usr/bin/env bash
# Preflight check for building Nabu from source.
#
# Verifies every prerequisite required by the build pipeline and reports
# clear, actionable messages for anything missing. Exits non-zero when a
# required tool is absent so the build fails fast instead of halfway through.
#
# Usage:  scripts/check-env.sh [--strict]
set -uo pipefail

# Colour helpers (no-op when not a TTY)
if [ -t 1 ]; then
  RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; BOLD=''; RESET=''
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MISSING=0
WARNINGS=0
STRICT=0
if [ "${1:-}" = "--strict" ]; then
  STRICT=1
fi

report_ok()   { printf '  %s✔%s %s\n' "$GREEN" "$RESET" "$1"; }
report_miss() { printf '  %s✘ %s%s%s\n' "$RED" "$1" "$RESET" "$2"; MISSING=1; }
report_warn() { printf '  %s! %s%s%s\n' "$YELLOW" "$1" "$RESET" "$2"; WARNINGS=1; }

need_cmd() { # need_cmd <name> <hint>
  if command -v "$1" >/dev/null 2>&1; then report_ok "$1 ($(command -v "$1"))"; else report_miss "$1" "${2:-}"; fi
}

echo "==> Checking prerequisites for building Nabu"
echo

# ── Rust toolchain ──────────────────────────────────────────────────────────
echo "Rust toolchain"
need_cmd rustc "  Install with:  rustup toolchain install stable  (or use mise:  mise install)"
need_cmd cargo "  Install with:  rustup toolchain install stable"
# version_ge <a> <b>: exit 0 if a >= b (dotted versions), else 1
version_ge() {
  local IFS=.
  read -ra AV <<< "$1"
  read -ra BV <<< "$2"
  for i in 0 1 2; do
    local a="${AV[$i]:-0}" b="${BV[$i]:-0}"
    if [ "$a" -gt "$b" ] 2>/dev/null; then return 0; fi
    if [ "$a" -lt "$b" ] 2>/dev/null; then return 1; fi
  done
  return 0
}

if command -v rustc >/dev/null 2>&1; then
  MSRV=$(awk -F'=' '/^rust-version/{gsub(/[ "]/, "", $2); print $2}' src-tauri/Cargo.toml)
  HAVE=$(rustc --version | awk '{print $2}' | tr -d '\r')
  if [ -n "$MSRV" ]; then
    if version_ge "$HAVE" "$MSRV"; then
      report_ok "rustc $HAVE (>= pinned rust-version $MSRV)"
    else
      report_miss "rustc $HAVE is older than the pinned rust-version $MSRV" \
        "  Install with:  rustup toolchain install $MSRV  (or: mise install)"
    fi
  fi
fi

# ── rustup targets (only needed for release bundles) ───────────────────────
echo
echo "Rust targets"
add_target() { # add_target <triple> <why> <required>  (required=1 => hard error when missing)
  local required="${3:-1}"
  if rustup target list --installed 2>/dev/null | grep -qx "$1"; then
    report_ok "target $1"
  elif [ "$required" = 1 ]; then
    report_miss "target $1" "  Install with:  rustup target add $1   ($2)"
  else
    report_warn "target $1" "  Optional for native builds; required for universal/CI:  rustup target add $1   ($2)"
  fi
}
add_target wasm32-unknown-unknown "frontend is compiled to WebAssembly" 1
case "$(uname -s)" in
  Darwin)
    add_target x86_64-apple-darwin "Intel macOS support" 1
    add_target aarch64-apple-darwin "Apple Silicon macOS support (universal builds)" "$STRICT"
    ;;
  Linux)
    add_target x86_64-unknown-linux-gnu "Linux desktop builds" 1
    ;;
  MINGW*|MSYS*|CYGWIN*)
    add_target x86_64-pc-windows-msvc "Windows desktop builds" 1
    ;;
esac

# ── Frontend tooling ────────────────────────────────────────────────────────
echo
echo "Frontend tooling"
need_cmd node "  Install with:  mise install  (or https://nodejs.org)"
need_cmd npm  "  Installed together with Node.js"
need_cmd trunk "  Install with:  cargo install trunk  (or:  mise install)"
if command -v trunk >/dev/null 2>&1; then
  report_ok "trunk $(trunk --version 2>/dev/null | sed 's/trunk //')"
fi

echo
echo "npm dependencies"
if [ -d node_modules/.bin ]; then
  report_ok "node_modules present (tailwindcss at node_modules/.bin/tailwindcss)"
else
  report_miss "node_modules/ (project dependencies)" "  Install with:  npm install"
fi

# ── Tauri CLI ───────────────────────────────────────────────────────────────
echo
echo "Tauri CLI"
need_cmd cargo
if cargo tauri --version >/dev/null 2>&1; then
  report_ok "tauri-cli $(cargo tauri --version 2>/dev/null | sed 's/tauri-cli //')"
else
  report_miss "tauri-cli" "  Install with:  cargo install tauri-cli --version ^2"
fi

# ── Platform packaging tools ────────────────────────────────────────────────
echo
echo "Platform packaging tools"
case "$(uname -s)" in
  Darwin)
    need_cmd lipo "  Built into macOS command-line tools (xcode-select --install)"
    need_cmd iconutil "  Built into macOS command-line tools"
    ;;
esac

echo
if [ "$MISSING" = 1 ]; then
  printf '%s==> Some required tools are missing. Install them, then re-run this check.%s\n' "$RED" "$RESET"
  exit 1
fi
if [ "$WARNINGS" = 1 ]; then
  printf '%s==> Check passed with warnings.%s\n' "$YELLOW" "$RESET"
else
  printf '%s==> All prerequisites satisfied. You can build Nabu.%s\n' "$GREEN" "$RESET"
fi
exit 0
