#!/usr/bin/env bash
#
# fix-author-history.sh — rewrite the author (and committer) identity of the
# most recent commits on the current branch.
#
#   SAFETY:
#     • Never pushes anything. It only prints the push command for you to run.
#     • Creates a backup branch before rewriting.
#     • Requires an interactive [y/N] confirmation before touching history.
#     • Refuses to run with a dirty working tree.
#     • Supports --dry-run to preview without changing anything.
#
# USAGE:
#   ./fix-author-history.sh "Full Name" "email@example.com" [commit-count] [--dry-run]
#
# EXAMPLES:
#   ./fix-author-history.sh --dry-run
#   ./fix-author-history.sh "Pablo Gutierrez" "150186168+pablogutil@users.noreply.github.com" 5
#   ./fix-author-history.sh "Pablo Gutierrez" "150186168+pablogutil@users.noreply.github.com" 5 --dry-run
#
set -euo pipefail

NAME="${1:-}"
EMAIL="${2:-}"
COUNT=5
DRY_RUN=0

# Scan all args: `--dry-run` sets the dry-run flag; the first purely-numeric
# arg is the commit count (so `--dry-run` in any position is never misread as
# a count, e.g. `./fix-author-history.sh "Name" "email" --dry-run`).
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    *[!0-9]*|'') ;;
    *) COUNT="$arg" ;;
  esac
done

usage() {
  echo "Usage: $0 \"Full Name\" \"email@example.com\" [commit-count] [--dry-run]" >&2
  exit 1
}

# --- input validation ------------------------------------------------------
# ./fix-author-history.sh --dry-run  (bare)  => preview with placeholder identity
if [[ "${1:-}" == "--dry-run" && -z "${2:-}" ]]; then
  NAME="Current Author"
  EMAIL="current@example.com"
fi
if [[ -z "$NAME" || -z "$EMAIL" ]]; then
  usage
fi

if ! command -v git >/dev/null 2>&1; then
  echo "error: git not found" >&2
  exit 1
fi

# --- preconditions ----------------------------------------------------------
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "error: not inside a git repository" >&2
  exit 1
fi

if ! git symbolic-ref -q HEAD >/dev/null 2>&1; then
  echo "error: HEAD is detached. Check out a branch first, e.g. 'git switch main'." >&2
  exit 1
fi

CUR_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
ROOT="$(git rev-parse --show-toplevel)"
UPSTREAM="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || echo 'none')"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is not clean." >&2
  echo "       Commit or stash first, e.g.:  git stash push --include-untracked" >&2
  exit 1
fi

if [[ ! "$COUNT" =~ ^[0-9]+$ || "$COUNT" -lt 1 ]]; then
  echo "error: commit-count must be a positive integer, got '$COUNT'" >&2
  exit 1
fi

if ! git rev-parse -q --verify "HEAD~$COUNT" >/dev/null 2>&1; then
  echo "error: only $(git rev-list --count HEAD) commits exist; cannot go back $COUNT." >&2
  exit 1
fi

RANGE="HEAD~$COUNT..HEAD"

if [[ "$(git rev-list --merges --count "$RANGE" 2>/dev/null || echo 0)" -gt 0 ]]; then
  echo "error: the selected range contains merge commit(s); a plain 'git rebase -i'" >&2
  echo "       would flatten them. Use git-filter-repo or '--rebase-merges' instead." >&2
  exit 1
fi

echo "================================================================"
echo " Repository : $ROOT"
echo " Branch     : $CUR_BRANCH   (upstream: $UPSTREAM)"
echo " Scope      : last $COUNT commit(s): $RANGE"
echo " New author : $NAME <$EMAIL>"
echo "================================================================"
echo
echo "The following commits WILL be rewritten (new SHAs):"
git log --format='  %h  %an <%ae>  |  %s' "$RANGE"
echo
echo "Their current identity:"
git log -1 --format='  %an <%ae>' "$RANGE"
echo

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "DRY RUN — no changes made. Re-run without --dry-run to execute."
  exit 0
fi

# --- confirmation ------------------------------------------------------------
echo "NOTE: if this branch has been pushed, rewriting is a FORCE-PUSH (history"
echo "      rewrite). Everyone else with a clone of '$CUR_BRANCH' will need to"
echo "      reset. A backup branch will be created first."
read -r -p "Create backup and rewrite these commits? [y/N] " answer
case "$answer" in
  [yY]*) ;;
  *) echo "Aborted — nothing was changed."; exit 0 ;;
esac

# --- backup -------------------------------------------------------------------
BACKUP="backup/$CUR_BRANCH-before-author-fix"
if git rev-parse -q --verify "refs/heads/$BACKUP" >/dev/null 2>&1; then
  echo "note: backup branch '$BACKUP' already exists — keeping it."
else
  git branch "$BACKUP"
  echo "Backup branch created: $BACKUP"
fi

# --- rewrite via interactive rebase ------------------------------------------
# GIT_SEQUENCE_EDITOR=true accepts the default todo list unchanged (all "pick").
# --exec runs after every picked commit: amend it with the new identity.
# --reset-author sets the author to the current committer identity (from the
# exported env vars), renewing the author timestamp so a new SHA is produced.
echo
echo "Rewriting $COUNT commit(s) with author  $NAME <$EMAIL> ..."
export GIT_AUTHOR_NAME="$NAME"
export GIT_AUTHOR_EMAIL="$EMAIL"
export GIT_COMMITTER_NAME="$NAME"
export GIT_COMMITTER_EMAIL="$EMAIL"

GIT_SEQUENCE_EDITOR=true \
  git rebase -i "HEAD~$COUNT" \
  --exec 'git commit --amend --no-edit --reset-author'

# --- verification -------------------------------------------------------------
echo
echo "Done. Rewritten commits now show:"
git log --format='  %h  %an <%ae>  |  %s' "$RANGE"
echo
echo "Backup (if you need to undo):"
  echo "  git reset --hard $BACKUP"
echo
echo "To publish the rewritten history (ONLY if you intend to):"
echo "  git push --force-with-lease origin $CUR_BRANCH"
echo
echo "To fix the LOCAL identity going forward (so future commits are correct):"
echo "  git config user.name  \"$NAME\""
echo "  git config user.email \"$EMAIL\""
