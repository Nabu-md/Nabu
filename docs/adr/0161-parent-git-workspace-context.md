# ADR 0161: Parent Git workspace context

- Status: active
- Date: 2026-07-19

## Context

Nabu allows any folder to be a vault, including a documentation folder inside a larger repository. Git itself discovers an ancestor work tree from that folder, but Git output remains relative to the repository root. Treating those paths as vault-relative can expose sibling Markdown files, produce incorrect note paths, and let an app-created commit include staged or unstaged work outside the selected vault.

The vault is a content boundary: indexing, search, attachments, navigation, status, and app-created commits must not expand merely because repository metadata lives in an ancestor. Branches, remotes, fetch, pull, push, merge, and rebase state are repository-wide concerns and must use the actual work-tree root.

## Decision

Every vault-facing Git operation resolves a `GitWorkspace` through the active Git provider. The context has three values:

- `vault_root`: the selected Nabu content boundary
- `git_root`: the nearest work-tree root discovered by Git
- `vault_pathspec`: the vault path relative to `git_root`

Provider output from `git rev-parse --show-prefix` determines the pathspec; Nabu derives the native Git root from that prefix so WSL-backed paths never need unsafe reverse translation. Vault-scoped commands run at `git_root` with an explicit pathspec, and one mapping boundary converts repository-relative output back to vault-relative paths.

Nabu stages a vault with `git add -A -- <vault_pathspec>` and commits with `git commit --only -- <vault_pathspec>`. This includes all vault changes while preserving pre-existing staged and unstaged entries outside the vault. Repository-wide sync commands run at `git_root`, identify parent-repository feedback explicitly, and return only vault-relative changed paths to note refresh flows.

Settings exposes the resolved Git root. Analytics records only `git_root_relation` (`vault`, `parent`, or `none`) and stable resolution-failure categories; filesystem paths are never analytics properties.

## Consequences

- A nested vault can share its parent repository without indexing or committing sibling content.
- Status, diffs, history, dates, pulse, conflicts, and remote file URLs share one repository-to-vault path conversion rule.
- Pull, push, branch, remote, merge, and rebase behavior remains repository-wide and is labeled as such when the repository root is above the vault.
- Root-level and Gitless vault behavior remains unchanged.
- Arbitrary unrelated repository selection remains unsupported; the Git provider's nearest ancestor work tree is authoritative.
