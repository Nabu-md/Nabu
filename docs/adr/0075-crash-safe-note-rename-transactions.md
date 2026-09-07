---
type: ADR
id: "0075"
title: "Crash-safe note rename transactions"
status: active
date: 2026-04-22
---

## Context

Nabu's note rename path used a simple "write new file, then delete old file" flow. That was easy to implement, but it had three integrity problems called out by issue #205: it could leave a visible duplicate note if the app crashed between those steps, destination-path selection depended on check-then-use races, and backlink rewrite failures were collapsed into a generic updated-files count that made partial success look clean.

Rename is a core vault integrity operation. The app needs a flow that preserves a trustworthy visible state even when the process dies mid-rename, and it needs to surface any partial backlink rewrite failures clearly enough that users are not told everything updated when some linked files still need manual repair.

## Decision

**Nabu now stages note renames through a hidden per-vault transaction directory and recovers unfinished transactions on the next vault scan.**

- A rename that changes the file path first writes a transaction manifest plus a hidden backup path inside `<vault>/.nabu-rename-txn/`.
- Nabu moves the old note into that hidden backup, persists the new file with a no-clobber destination write, and then deletes the backup/manifest only after the new note exists.
- If the process crashes before the new note is committed, the next `scan_vault` restores the hidden backup back to the original path before listing entries.
- Manual filename renames keep their explicit conflict semantics, but the final destination is now claimed with a no-clobber write instead of relying on an existence check.
- Backlink rewrites now return both the number of successful updates and the number of failed updates so the UI can warn about partial success instead of reporting a clean rename.

## Options considered

- **Hidden transaction directory + scan-time recovery** (chosen): keeps in-flight rename artifacts out of the visible vault model, gives Nabu a deterministic recovery point after crashes, and lets the final destination use no-clobber persistence.
- **Rename in place without transaction metadata**: simpler, but it cannot recover a half-finished rename reliably after process death and still leaves either duplicate or missing-note windows.
- **Best-effort duplicate cleanup with no recovery path**: lowest implementation cost, but it leaves the user-visible vault state dependent on exact crash timing and does not meet the trustworthiness goal for rename operations.

## Consequences

- Every vault can now contain a hidden `.nabu-rename-txn/` directory managed by Nabu; scan and folder UI continue to ignore it because hidden directories are already excluded.
- Rename results are richer: the frontend must treat `failed_updates > 0` as a warning state even when the rename itself succeeded.
- Future changes to vault scanning or note rename behavior must preserve transaction recovery before entry listing, otherwise crash safety regresses.
- The rename path no longer silently overwrites a destination discovered via a stale existence check; title-driven renames retry with suffixed filenames, while explicit filename renames fail cleanly on collision.
