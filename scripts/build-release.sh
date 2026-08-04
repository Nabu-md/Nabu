#!/usr/bin/env bash
# Deprecated — use `scripts/build.sh` instead.
#
# Historical wrapper that built a universal macOS DMG. Kept for back-compat;
# delegates to the full pipeline (prerequisite checks, arch detection,
# frontend build, packaging).
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "error: build-release.sh builds a universal macOS DMG and must run on macOS." >&2
  echo "       On this platform use:  scripts/build.sh" >&2
  exit 1
fi

echo "Note: build-release.sh is deprecated — use scripts/build.sh (or scripts/build.sh --universal)."
exec "$ROOT/scripts/build.sh" --universal
