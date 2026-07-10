/**
 * search-query.ts
 *
 * AST_Walk_Query — a pure search-query module that parses operator:value
 * tokens from a query string and executes them against the ExtendedSearchIndex
 * plus an AST accessor.
 *
 * All functions are pure (side-effect free, no I/O) so the module is fully
 * unit-testable in isolation.
 *
 * Requirements: 3.2, 3.3, 3.4, 3.5, 3.6, 3.8
 */

import type { Root } from 'mdast'
import type { ExtendedSearchIndex } from './extended-indexing'
import type { FileEntry } from './types'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * A single match within a line of a search result.
 */
export interface SearchQueryMatch {
  /** 1-indexed line number in the source file. */
  line: number
  /** The line's text content (truncated to SNIPPET_MAX_LENGTH). */
  snippet: string
  /** 0-indexed start column of the match in the snippet. */
  startCol: number
  /** 0-indexed end column of the match (exclusive) in the snippet. */
  endCol: number
}

/**
 * A file-level search result.
 */
export interface SearchQueryResult {
  filePath: string
  /** File name without leading path or .md extension. */
  name: string
  /** Path relative to the vault root. */
  relativePath: string
  /** Relevance score (higher = better match). */
  score: number
  /** All matching lines within the file. */
  matches: SearchQueryMatch[]
}

/**
 * Parsed representation of a search query string.
 */
export interface ParsedQuery {
  /** path:<fragment> — filter by path substring. */
  path?: string
  /** tag:<name> — filter by tag membership. */
  tag?: string
  /** line:<text> — match against line content (exact substring). */
  line?: string
  /** content:<text> — same as line: (alias for consistency). */
  content?: string
  /** file:<name> — filter by file name. */
  file?: string
  /** property:<name>:<value> — filter by frontmatter property. */
  property?: { name: string; value: string }
  /** regex:<pattern> — match against line content via RegExp. */
  regex?: string
  /** Bare (un-prefixed) terms — all must appear in the file's tokens. */
  bareTerms: string[]
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** RegExp to match leading operator:value tokens. */
const OPERATOR_RE = /^(path|tag|line|content|file|property|regex):(.+)$/i

/** RegExp to match leading property:<name>:<value> (two colons). */
const PROPERTY_RE = /^property:([^:]+):(.*)$/i

/** Maximum length for line snippets in match results. */
const SNIPPET_MAX_LENGTH = 120

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

/**
 * Parse a raw query string into a structured ParsedQuery.
 *
 * Leading operator:value tokens are stripped and stored in the appropriate
 * field.  All remaining whitespace-delimited tokens are collected in
 * `bareTerms`.  Operators may appear in any order; the last occurrence wins
 * for duplicate operators.
 *
 * Requirements: 3.2
 */
export function parseQuery(query: string): ParsedQuery {
  const result: ParsedQuery = { bareTerms: [] }

  if (!query || !query.trim()) return result

  const tokens = query.trim().split(/\s+/)

  for (const token of tokens) {
    // Check property:name:value first (it has two colons, so the generic
    // OPERATOR_RE would incorrectly capture "property:name" as the operator
    // and "value" as... wait, OPERATOR_RE would do property:name and
    // value would be a separate token. Let me reconsider.
    //
    // property:name:value can be parsed by trying PROPERTY_RE first.
    const propMatch = token.match(PROPERTY_RE)
    if (propMatch) {
      result.property = { name: propMatch[1].toLowerCase(), value: propMatch[2] }
      continue
    }

    const match = token.match(OPERATOR_RE)
    if (match) {
      const [, operator, value] = match
      switch (operator.toLowerCase()) {
        case 'path':
          result.path = value
          break
        case 'tag':
          result.tag = value
          break
        case 'line':
          result.line = value
          break
        case 'content':
          result.content = value
          break
        case 'file':
          result.file = value
          break
        case 'regex':
          result.regex = value
          break
        // property is handled above before OPERATOR_RE
      }
      continue
    }

    // Bare term
    result.bareTerms.push(token.toLowerCase())
  }

  return result
}

// ---------------------------------------------------------------------------
// Query execution
// ---------------------------------------------------------------------------

/**
 * Execute a parsed query against the extended search index and return matching
 * files with their highlighted line snippets.
 *
 * All operators are AND-combined — a file must satisfy every supplied operator
 * to appear in the result set.  Bare terms are treated as content-word matches
 * and are also AND-combined.
 *
 * Results are sorted by score descending (best match first).
 *
 * Requirements: 3.3, 3.4, 3.5, 3.6, 3.8
 */
export function executeQuery(
  query: ParsedQuery,
  files: FileEntry[],
  vaultPath: string,
  extIndex: ExtendedSearchIndex,
  _getAST: (path: string) => Root | undefined
): SearchQueryResult[] {
  // Empty query → no results
  if (
    !query.bareTerms.length &&
    !query.path &&
    !query.tag &&
    !query.line &&
    !query.content &&
    !query.file &&
    !query.property &&
    !query.regex
  ) {
    return []
  }

  // Build a map for O(1) file lookup
  const fileMap = new Map<string, FileEntry>()
  for (const f of files) fileMap.set(f.path, f)

  // 0. Promote bare key:value terms that match known frontmatter properties
  //    (Req 13.2 — bare name:value form for unambiguous property keys)
  const parsed = promoteBarePropertyTerms(query, extIndex)

  // 1. Determine candidate files from fast index-based filters
  const candidatePaths = getCandidatesByIndexChecks(parsed, extIndex, files, vaultPath)

  // 2. For line:/content:/regex, also require the text to appear in snippets
  let candidates: SearchQueryResult[]
  if (
    parsed.line !== undefined ||
    parsed.content !== undefined ||
    parsed.regex !== undefined ||
    parsed.bareTerms.length > 0
  ) {
    candidates = filterBySnippetScan(candidatePaths, parsed, extIndex, vaultPath)
  } else {
    // No text-based filtering needed — build results from index-only matches
    candidates = candidatePaths
      .map((filePath) => {
        const file = fileMap.get(filePath)
        if (!file) return null
        const relativePath = vaultPath ? getRelativePath(vaultPath, filePath) : filePath
        return {
          filePath,
          name: file.name,
          relativePath,
          score: 1,
          matches: [] as SearchQueryMatch[]
        }
      })
      .filter((r): r is SearchQueryResult => r !== null)
  }

  // 3. Sort by score descending, tie-break by path
  candidates.sort((a, b) => b.score - a.score || a.filePath.localeCompare(b.filePath))

  return candidates
}

/**
 * Convenience wrapper: parse a raw query string and execute it in one call.
 */
export function search(
  queryString: string,
  files: FileEntry[],
  vaultPath: string,
  extIndex: ExtendedSearchIndex,
  getAST: (path: string) => Root | undefined
): SearchQueryResult[] {
  const parsed = parseQuery(queryString)
  return executeQuery(parsed, files, vaultPath, extIndex, getAST)
}

// ---------------------------------------------------------------------------
// Private helpers — candidate filtering
// ---------------------------------------------------------------------------

/**
 * Scan bare terms for `key:value` patterns that match known frontmatter
 * properties in the index.  When a match is found the term is promoted to a
 * `property:` filter and removed from bare terms.
 *
 * This implements Requirement 13.2 — bare `name:value` form for unambiguous
 * property keys.
 *
 * The returned ParsedQuery is either the original (if no promotion occurred)
 * or a new object with the promoted property filter and filtered bare terms.
 */
function promoteBarePropertyTerms(query: ParsedQuery, extIndex: ExtendedSearchIndex): ParsedQuery {
  if (extIndex.propertyIndex.size === 0) return query

  const promoted: ParsedQuery = { ...query, bareTerms: [...query.bareTerms] }
  let changed = false

  for (let i = promoted.bareTerms.length - 1; i >= 0; i--) {
    const term = promoted.bareTerms[i]
    const match = term.match(/^([\w\s-]+?):(.+)$/)
    if (match) {
      const name = match[1].trim().toLowerCase()
      const value = match[2].trim()
      if (extIndex.propertyIndex.has(name)) {
        // Promote to property filter if one isn't already set
        if (!promoted.property) {
          promoted.property = { name, value }
        }
        promoted.bareTerms.splice(i, 1)
        changed = true
      }
    }
  }

  return changed ? promoted : query
}

/**
 * Use fast index-based lookups (path, tag, file, property) to narrow the
 * candidate set, and collect bare-term matches from positions.
 *
 * Returns a Set of file paths that pass all index-based filters.
 */
function getCandidatesByIndexChecks(
  query: ParsedQuery,
  extIndex: ExtendedSearchIndex,
  files: FileEntry[],
  vaultPath: string
): string[] {
  let candidatePaths: string[] | null = null

  // Apply each index-based filter as an intersection.
  // Start with the full file list, narrow on each operator.

  const allPaths = files.map((f) => f.path)

  // --- tag: filter ---
  if (query.tag !== undefined) {
    const tagMatch = getFilesForTag(query.tag, extIndex)
    candidatePaths = intersect(candidatePaths ?? allPaths, tagMatch)
    if (candidatePaths.length === 0) return []
  }

  // --- path: filter ---
  if (query.path !== undefined) {
    const pathLower = query.path.toLowerCase()
    const pathMatch = allPaths.filter((fp) => {
      const rel = getRelativePath(vaultPath, fp)
      return rel.toLowerCase().includes(pathLower)
    })
    candidatePaths = intersect(candidatePaths ?? allPaths, pathMatch)
    if (candidatePaths.length === 0) return []
  }

  // --- file: filter ---
  if (query.file !== undefined) {
    const fileLower = query.file.toLowerCase()
    const fileMatch = files
      .filter((f) => f.name.toLowerCase().includes(fileLower))
      .map((f) => f.path)
    candidatePaths = intersect(candidatePaths ?? allPaths, fileMatch)
    if (candidatePaths.length === 0) return []
  }

  // --- property: filter ---
  if (query.property !== undefined) {
    const { name, value } = query.property
    const valueMap = extIndex.propertyIndex.get(name)
    if (!valueMap) return []
    const propMatch = valueMap.get(value) ?? new Set<string>()
    candidatePaths = intersect(candidatePaths ?? allPaths, Array.from(propMatch))
    if (candidatePaths.length === 0) return []
  }

  // --- bare terms: filter via positions map ---
  for (const term of query.bareTerms) {
    const fileMap = extIndex.positions.get(term)
    if (!fileMap) return [] // term not found anywhere → empty
    const termMatch = Array.from(fileMap.keys())
    candidatePaths = intersect(candidatePaths ?? allPaths, termMatch)
    if (candidatePaths.length === 0) return []
  }

  return candidatePaths ?? allPaths
}

/**
 * For line:/content:/regex queries, scan the lineSnippets of each candidate
 * and produce SearchQueryResult entries with match information.
 *
 * Also re-checks bare terms for content-based files (in case position index
 * had the word but the snippet scan needs to locate it).
 */
function filterBySnippetScan(
  candidates: string[],
  query: ParsedQuery,
  extIndex: ExtendedSearchIndex,
  vaultPath: string
): SearchQueryResult[] {
  const results: SearchQueryResult[] = []

  for (const filePath of candidates) {
    const snippets = extIndex.lineSnippets.get(filePath)
    if (!snippets) continue
    const _snippets: string[] = snippets

    const file = { path: filePath, name: getNameFromPath(filePath) }
    const relativePath = vaultPath ? getRelativePath(vaultPath, filePath) : filePath
    const matches: SearchQueryMatch[] = []
    const seenLines = new Set<number>()

    // Helper: add a match for a given line if not already recorded
    function addMatch(lineIdx: number, startCol: number, endCol: number): void {
      if (seenLines.has(lineIdx)) return
      seenLines.add(lineIdx)
      const snippet = _snippets[lineIdx]?.slice(0, SNIPPET_MAX_LENGTH) ?? ''
      matches.push({
        line: lineIdx + 1, // convert 0-indexed array to 1-indexed line
        snippet,
        startCol,
        endCol
      })
    }

    // --- line:/content: substring match ---
    if (query.line !== undefined) {
      const needle = query.line.toLowerCase()
      for (let i = 0; i < snippets.length; i++) {
        const lineText = snippets[i].toLowerCase()
        const col = lineText.indexOf(needle)
        if (col !== -1) {
          addMatch(i, col, col + query.line.length)
        }
      }
    }

    if (query.content !== undefined) {
      const needle = query.content.toLowerCase()
      for (let i = 0; i < snippets.length; i++) {
        const lineText = snippets[i].toLowerCase()
        const col = lineText.indexOf(needle)
        if (col !== -1) {
          addMatch(i, col, col + query.content.length)
        }
      }
    }

    // --- regex: match ---
    if (query.regex !== undefined) {
      let regex: RegExp
      try {
        regex = new RegExp(query.regex, 'gi')
      } catch {
        // Invalid regex — skip this file
        continue
      }
      for (let i = 0; i < snippets.length; i++) {
        const execResult = regex.exec(snippets[i])
        if (execResult !== null) {
          addMatch(i, execResult.index, execResult.index + execResult[0].length)
        }
      }
    }

    // --- bare terms: ensure each term appears in the file's position map ---
    // (already verified in getCandidatesByIndexChecks, but we need to locate
    //  the matches in the snippet text for highlighting)
    for (const term of query.bareTerms) {
      const fileMap = extIndex.positions.get(term)
      if (!fileMap?.has(filePath)) continue
      // Locate the term in snippets for highlighting
      for (let i = 0; i < snippets.length; i++) {
        const lineText = snippets[i].toLowerCase()
        const col = lineText.indexOf(term)
        if (col !== -1) {
          addMatch(i, col, col + term.length)
        }
      }
    }

    // Only include files with at least one match when content/regex/line/terms are searched
    const hasContentQuery =
      query.line !== undefined ||
      query.content !== undefined ||
      query.regex !== undefined ||
      query.bareTerms.length > 0

    if (hasContentQuery && matches.length === 0) continue

    results.push({
      filePath,
      name: file.name,
      relativePath,
      score: matches.length,
      matches
    })
  }

  return results
}

// ---------------------------------------------------------------------------
// Private helpers — set operations
// ---------------------------------------------------------------------------

/**
 * Intersect two string arrays, returning a new array with elements present
 * in both (using Set for O(n) intersection).
 */
function intersect(a: string[], b: string[]): string[] {
  const setB = new Set(b)
  return a.filter((x) => setB.has(x))
}

/**
 * Get all file paths that match a given tag (exact or namespace prefix).
 */
function getFilesForTag(tag: string, extIndex: ExtendedSearchIndex): string[] {
  const matched = new Set<string>()

  // Exact match
  const exact = extIndex.tagIndex.get(tag)
  if (exact) {
    for (const p of exact) matched.add(p)
  }

  // Namespace prefix match: tags starting with "tag/"
  for (const [key, paths] of extIndex.tagIndex) {
    if (key.startsWith(tag + '/')) {
      for (const p of paths) matched.add(p)
    }
  }

  return Array.from(matched)
}

/**
 * Compute relative path from vaultPath to filePath.
 */
function getRelativePath(vaultPath: string, filePath: string): string {
  // Simple path-relative computation without requiring Node path module
  // (since this is shared code that may run in the renderer)
  if (filePath.startsWith(vaultPath)) {
    const rel = filePath.slice(vaultPath.length)
    return rel.startsWith('/') ? rel.slice(1) : rel
  }
  return filePath
}

/**
 * Extract the file name (without directory or extension) from a path string.
 */
function getNameFromPath(filePath: string): string {
  const lastSlash = filePath.lastIndexOf('/')
  const basename = lastSlash >= 0 ? filePath.slice(lastSlash + 1) : filePath
  return basename.endsWith('.md') ? basename.slice(0, -3) : basename
}
