/**
 * path.ts
 *
 * Canonical, deterministic path-resolution utilities for Nabu.
 *
 * Phase 4.3 establishes ONE strategy for resolving vault-relative,
 * workspace-relative, and normalized filesystem paths. Previously the codebase
 * used four divergent approaches:
 *
 *   - `path.relative` + `path.sep` (state.ts)
 *   - a hand-rolled `getRelativePath` in search-query.ts (no normalization)
 *   - `notePath.replace(vaultPath, '')` in snapshots.ts / view-state.ts
 *     (no separator safety — a vault named "Notes" would corrupt a path like
 *      "/Notes/MyNotes/file.md" → "/MyNotes/file.md")
 *
 * All call sites now route through the helpers below so resolution is
 * consistent, platform-safe, and free of accidental substring collisions.
 *
 * This module is intentionally dependency-free (no Node `fs`/`path`) so it can
 * be imported from both the main process and the renderer without bundler
 * concerns. It implements a minimal, deterministic subset of `path` semantics
 * that is sufficient for vault/workspace resolution.
 */

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Platform path separator (normalized to '/' for cross-platform stability). */
const SEP = '/'

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/**
 * Normalize a filesystem path to a canonical form.
 *
 * - Collapses repeated separators (`a//b` → `a/b`)
 * - Converts backslashes to forward slashes (Windows compatibility)
 * - Removes trailing separators (`a/b/` → `a/b`)
 * - Resolves `.` and `..` segments lexically (no filesystem access)
 *
 * The result is deterministic and safe to use as a map key or for equality
 * comparisons across platforms.
 */
export function normalizePath(input: string): string {
  if (!input) return ''

  // 1. Unify separators to '/'
  let p = input.replace(/\\/g, SEP)

  // 2. Collapse repeated separators
  p = p.replace(/\/{2,}/g, SEP)

  // 3. Split into segments, resolving '.' and '..' lexically
  const isAbsolute = p.startsWith(SEP) || /^[A-Za-z]:/.test(p)
  const segments = p.split(SEP)
  const stack: string[] = []

  for (const seg of segments) {
    if (seg === '' || seg === '.') continue
    if (seg === '..') {
      if (stack.length > 0 && stack[stack.length - 1] !== '..') {
        stack.pop()
      } else if (!isAbsolute) {
        stack.push('..')
      }
      // For absolute paths, '..' above root is a no-op.
      continue
    }
    stack.push(seg)
  }

  // 4. Reassemble
  let result = stack.join(SEP)

  // Preserve a leading separator for absolute POSIX paths.
  if (isAbsolute && p.startsWith(SEP) && !result.startsWith(SEP)) {
    result = SEP + result
  }
  // Preserve a drive-letter prefix (e.g. "C:").
  const driveMatch = p.match(/^([A-Za-z]:)/)
  if (driveMatch && !result.startsWith(driveMatch[1])) {
    result = driveMatch[1] + SEP + result
  }

  // 5. Strip a single trailing separator (unless root "/")
  if (result.length > 1 && result.endsWith(SEP)) {
    result = result.slice(0, -1)
  }

  return result
}

// ---------------------------------------------------------------------------
// Vault-relative resolution
// ---------------------------------------------------------------------------

/**
 * Resolve `filePath` to a path relative to `vaultPath`.
 *
 * Returns the normalized relative path (no leading separator). If `filePath`
 * is not under `vaultPath`, the normalized `filePath` is returned unchanged.
 *
 * This replaces the previous `notePath.replace(vaultPath, '')` pattern, which
 * was unsafe when `vaultPath` was a substring of a later path segment.
 */
export function toVaultRelative(vaultPath: string, filePath: string): string {
  const base = normalizePath(vaultPath)
  const target = normalizePath(filePath)

  if (target === base) return ''

  // Exact prefix match with a separator boundary (prevents "Notes" matching
  // inside "/MyNotes/file.md").
  if (target.startsWith(base + SEP)) {
    return target.slice(base.length + 1)
  }

  return target
}

/**
 * Resolve a vault-relative path back to an absolute path under `vaultPath`.
 *
 * Inverse of {@link toVaultRelative}. Always produces a normalized, absolute
 * path that is guaranteed to live beneath `vaultPath` (path-traversal
 * segments are lexicalized by `normalizePath`).
 */
export function fromVaultRelative(vaultPath: string, relativePath: string): string {
  const base = normalizePath(vaultPath)
  const rel = normalizePath(relativePath)
  if (!rel) return base
  return normalizePath(base + SEP + rel)
}

// ---------------------------------------------------------------------------
// Workspace-relative resolution
// ---------------------------------------------------------------------------

/**
 * Resolve `filePath` to a path relative to a workspace root.
 *
 * Mirrors {@link toVaultRelative} but is scoped to workspace-level roots
 * (e.g. attachment directories, plugin folders). Kept separate so ownership
 * and intent are explicit at each call site.
 */
export function toWorkspaceRelative(workspaceRoot: string, filePath: string): string {
  return toVaultRelative(workspaceRoot, filePath)
}

// ---------------------------------------------------------------------------
// Basename / extension helpers
// ---------------------------------------------------------------------------

/**
 * Extract the file name (without directory) from a path.
 * Cross-platform safe (handles both '/' and '\').
 */
export function getFileName(filePath: string): string {
  const normalized = normalizePath(filePath)
  const idx = normalized.lastIndexOf(SEP)
  return idx >= 0 ? normalized.slice(idx + 1) : normalized
}

/**
 * Extract the file name without its extension.
 * Cross-platform safe; only strips the final extension.
 */
export function getFileNameWithoutExt(filePath: string): string {
  const name = getFileName(filePath)
  const dot = name.lastIndexOf('.')
  return dot > 0 ? name.slice(0, dot) : name
}

/**
 * Return the lower-cased extension (including the leading dot) of a path,
 * or '' when none is present.
 */
export function getExtension(filePath: string): string {
  const name = getFileName(filePath)
  const dot = name.lastIndexOf('.')
  return dot > 0 ? name.slice(dot).toLowerCase() : ''
}
