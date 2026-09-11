#!/usr/bin/env bash
# Check the Tolaria upstream remote for new commits worth cherry-picking.
#
# Usage:
#   scripts/tolaria-sync-check.sh [upstream-branch]
#
# Environment:
#   TOLARIA_REMOTE   name of the git remote pointing at Tolaria (default: tolarria)
#
# Exits 0 when up to date or when new commits are reported (informational);
# exits 1 when the remote/branch cannot be resolved.
set -euo pipefail

UPSTREAM_REMOTE="${TOLARIA_REMOTE:-tolarria}"
UPSTREAM_BRANCH="${1:-main}"
LOCAL_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

if ! git remote get-url "$UPSTREAM_REMOTE" >/dev/null 2>&1; then
  echo "error: remote '$UPSTREAM_REMOTE' not found." >&2
  echo "Add it with: git remote add $UPSTREAM_REMOTE https://github.com/refactoringhq/tolaria.git" >&2
  exit 1
fi

if ! git rev-parse --verify --quiet "$UPSTREAM_REMOTE/$UPSTREAM_BRANCH" >/dev/null; then
  echo "error: branch '$UPSTREAM_REMOTE/$UPSTREAM_BRANCH' not found. Run: git fetch $UPSTREAM_REMOTE" >&2
  exit 1
fi

echo "Fetching $UPSTREAM_REMOTE..."
git fetch "$UPSTREAM_REMOTE" --quiet

NEW_COMMITS="$(git log --oneline "$LOCAL_BRANCH..$UPSTREAM_REMOTE/$UPSTREAM_BRANCH")"

if [ -z "$NEW_COMMITS" ]; then
  echo "Up to date: no new commits on $UPSTREAM_REMOTE/$UPSTREAM_BRANCH."
  exit 0
fi

echo "Tolaria has new commits:"
echo "$NEW_COMMITS"
echo ""
echo "Diff stat against $LOCAL_BRANCH:"
git diff "$LOCAL_BRANCH..$UPSTREAM_REMOTE/$UPSTREAM_BRANCH" --stat
