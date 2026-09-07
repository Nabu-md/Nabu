#!/bin/bash
set -euo pipefail

REPO="/Users/macbook/github code/Nabu"
cd "$REPO"

echo "=== Reverting sidebar/gitpanel commits ==="

# Revert the three commits that broke the sidebar and git panel
git revert --no-commit d345a450 dcd0c09f f07c72c5

git commit -m "revert: restore sidebar/gitpanel to pre-change state

Reverts:
- d345a450 Add open-source attribution and remove sidebar feature
- dcd0c09f chore: remove pre-commit hook
- f07c72c5 rebrand

These changes broke sidebar opacity, FluidVoice/Harper/AnyDoc settings
updates, and general sidebar/gitpanel functionality."

echo "=== Rebuild ==="
pnpm tauri build

echo "=== Update app ==="
pkill -f "Nabu.app" 2>/dev/null || true
sleep 2
cp -R "$REPO/src-tauri/target/release/bundle/macos/Nabu.app" "/Applications/Nabu.app"

echo "=== Done ==="
