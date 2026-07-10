/**
 * extended-indexing.ts
 *
 * Extended search index providing token positions, line snippets, a unified
 * tag index (frontmatter + inline #tags), an alias map, a property index,
 * and block-reference tracking.
 *
 * All functions are pure (side-effect free, no I/O) — they receive file
 * metadata and an AST accessor callback and return plain Maps.
 *
 * Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8
 */

import { visit, SKIP } from 'unist-util-visit'
import type { VisitorResult } from 'unist-util-visit'
import type { Root, Yaml } from 'mdast'
import type { FileEntry } from './types'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ExtendedSearchIndex {
  /** word → filePath → sorted unique line numbers on which the token occurs (Req 2.1) */
  positions: Map<string, Map<string, number[]>>
  /** filePath → per-line source snippets, indexed by (lineNumber - 1) (Req 2.2) */
  lineSnippets: Map<string, string[]>
  /** tag → set of file paths — unified from frontmatter + inline #tags (Req 2.3, 2.4) */
  tagIndex: Map<string, Set<string>>
  /** lowercase alias → array of owning file paths (Req 2.5) */
  aliasIndex: Map<string, string[]>
  /** property name → value → set of file paths */
  propertyIndex: Map<string, Map<string, Set<string>>>
  /** filePath → blockId → node position key */
  blockRefs: Map<string, Map<string, string>>
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Maximum length to which a line snippet is truncated (Req 2.2). */
const SNIPPET_MAX_LENGTH = 120

/**
 * Regex for extracting inline #tags from text nodes.
 *
 * Matches a `#` followed by one or more word characters, digits, underscores,
 * hyphens, or forward slashes (for namespaced tags like `parent/child`).
 * Uses a negative lookbehind to avoid matching `##` (ATX headings) and a
 * negative lookahead to stop at punctuation or whitespace.
 *
 * Exported so the renderer can use the same pattern for clickable tag chips.
 */
export const INLINE_TAG_RE = /(?<!\w)(#[\p{L}\p{N}_/\-]+)(?!\w)/gu

/**
 * Regex for detecting block identifiers (`^block-id`) at the end of a line.
 */
const BLOCK_REF_RE = /[ \t]?\^([\w-]+)$/

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Create a new empty extended search index.
 */
export function createEmptyIndex(): ExtendedSearchIndex {
  return {
    positions: new Map(),
    lineSnippets: new Map(),
    tagIndex: new Map(),
    aliasIndex: new Map(),
    propertyIndex: new Map(),
    blockRefs: new Map()
  }
}

/**
 * Build a complete ExtendedSearchIndex from the given file list.
 *
 * For each file the AST is retrieved via `getAST`; if `undefined` is returned
 * the file is silently skipped. Frontmatter (yaml/toml) content is excluded
 * from token positions (Req 2.7) while still being parsed for tags, aliases,
 * and properties.
 *
 * Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.7, 2.8
 */
export function buildExtendedIndex(
  files: FileEntry[],
  getAST: (path: string) => Root | undefined
): ExtendedSearchIndex {
  const index = createEmptyIndex()

  for (const file of files) {
    const root = getAST(file.path)
    if (root === undefined) continue

    indexFile(index, file.path, root)
  }

  return index
}

/**
 * Incrementally update an ExtendedSearchIndex for a single changed file.
 *
 * Removes all existing entries for `filePath` from every sub-index, then
 * re-indexes the file if the AST is provided. If `ast` is `undefined` the
 * file's entries are simply removed (handles deletion).
 *
 * Returns the same index object (mutated in place for efficiency).
 *
 * Requirements: 2.6, 2.8
 */
export function updateExtendedIndexForFile(
  index: ExtendedSearchIndex,
  filePath: string,
  ast: Root | undefined
): ExtendedSearchIndex {
  removeFileFromIndex(index, filePath)
  if (ast) {
    indexFile(index, filePath, ast)
  }
  return index
}

// ---------------------------------------------------------------------------
// Private helpers — index a single file into an existing index
// ---------------------------------------------------------------------------

/**
 * Walk the given file's AST and add all its entries to the index.
 */
function indexFile(index: ExtendedSearchIndex, filePath: string, root: Root): void {
  // 1. Token positions + line snippets from text/code nodes (skip yaml/toml)
  const snippetMap = new Map<number, string>()

  visit(root, (node): VisitorResult => {
    const nodeType = (node as { type: string }).type
    if (nodeType === 'yaml' || nodeType === 'toml') {
      return SKIP
    }

    if (nodeType === 'text' || nodeType === 'inlineCode' || nodeType === 'code') {
      const value = (node as { value: string }).value
      if (!value) return

      const line = (node as { position?: { start: { line: number } } }).position?.start?.line
      if (line !== undefined) {
        // Accumulate snippet text per line
        const existing = snippetMap.get(line) ?? ''
        snippetMap.set(line, existing ? existing + ' ' + value.trim() : value.trim())

        // Tokenize and record positions
        const tokens = tokenize(value)
        for (const token of tokens) {
          addTokenPosition(index, token, filePath, line)
        }
      }

      // Inline #tag extraction — skip code blocks and inline code
      if (nodeType === 'text') {
        extractInlineTags(index, value, filePath)
        extractBlockRefs(index, value, filePath, line)
      }
    }
  })

  // Convert snippet map to per-file line array
  if (snippetMap.size > 0) {
    const maxLine = Math.max(...snippetMap.keys())
    const snippets: string[] = new Array(maxLine).fill('')
    for (const [line, text] of snippetMap) {
      const truncated = text.slice(0, SNIPPET_MAX_LENGTH)
      snippets[line - 1] = truncated
    }
    index.lineSnippets.set(filePath, snippets)
  }

  // 2. Structured fields from YAML frontmatter
  let yamlNode: Yaml | undefined
  visit(root, 'yaml', (node: Yaml) => {
    yamlNode = node
    return SKIP
  })

  if (yamlNode) {
    const { tags, aliases, properties } = parseYamlFields(yamlNode.value)
    for (const tag of tags) {
      if (!tag || !/[\p{L}\p{N}]/u.test(tag)) continue
      addToSetMap(index.tagIndex, tag, filePath)
      // Index parent segments for namespaced tags (Req 2.4)
      addNamespacedParentTags(index.tagIndex, tag, filePath)
    }
    for (const alias of aliases) {
      addToArrayMap(index.aliasIndex, alias.toLowerCase(), filePath)
    }
    for (const [key, value] of properties) {
      addToPropertyIndex(index.propertyIndex, key, value, filePath)
    }
  }

  // 3. Block references from text content
  // (extractBlockRefs is called inline during the visit above)
}

// ---------------------------------------------------------------------------
// Private helpers — remove a file from an existing index
// ---------------------------------------------------------------------------

/**
 * Remove all entries for `filePath` from every sub-index.
 */
function removeFileFromIndex(index: ExtendedSearchIndex, filePath: string): void {
  // Remove from positions
  for (const [, fileMap] of index.positions) {
    fileMap.delete(filePath)
  }
  // Clean up empty word entries in positions
  for (const [word, fileMap] of index.positions) {
    if (fileMap.size === 0) {
      index.positions.delete(word)
    }
  }

  // Remove from lineSnippets
  index.lineSnippets.delete(filePath)

  // Remove from tagIndex
  for (const [, paths] of index.tagIndex) {
    paths.delete(filePath)
  }
  for (const [tag, paths] of index.tagIndex) {
    if (paths.size === 0) {
      index.tagIndex.delete(tag)
    }
  }

  // Remove from aliasIndex
  for (const [alias, paths] of index.aliasIndex) {
    const filtered = paths.filter((p) => p !== filePath)
    if (filtered.length === 0) {
      index.aliasIndex.delete(alias)
    } else {
      index.aliasIndex.set(alias, filtered)
    }
  }

  // Remove from propertyIndex
  for (const [, valueMap] of index.propertyIndex) {
    for (const [, paths] of valueMap) {
      paths.delete(filePath)
    }
  }
  for (const [propName, valueMap] of index.propertyIndex) {
    for (const [value, paths] of valueMap) {
      if (paths.size === 0) {
        valueMap.delete(value)
      }
    }
    if (valueMap.size === 0) {
      index.propertyIndex.delete(propName)
    }
  }

  // Remove from blockRefs
  index.blockRefs.delete(filePath)
}

// ---------------------------------------------------------------------------
// Private helpers — index updates
// ---------------------------------------------------------------------------

/**
 * Record that a token appears on a specific line in a file.
 * Deduplicates line numbers (line numbers are visited in order, so we only
 * check the last entry).
 */
function addTokenPosition(
  index: ExtendedSearchIndex,
  token: string,
  filePath: string,
  line: number
): void {
  let fileMap = index.positions.get(token)
  if (!fileMap) {
    fileMap = new Map<string, number[]>()
    index.positions.set(token, fileMap)
  }

  let lines = fileMap.get(filePath)
  if (!lines) {
    lines = []
    fileMap.set(filePath, lines)
  }

  // Dedup: only push if the last entry differs (nodes are visited in order)
  if (lines.length === 0 || lines[lines.length - 1] !== line) {
    lines.push(line)
  }
}

/**
 * Extract inline #tags from a text value and add them to the tag index.
 * Tags are normalised by stripping the leading `#`.
 * For namespaced tags (`parent/child`), both the full path and parent
 * segment are indexed (Req 2.4).
 */
function extractInlineTags(index: ExtendedSearchIndex, text: string, filePath: string): void {
  const matches = text.matchAll(INLINE_TAG_RE)
  for (const match of matches) {
    const rawTag = match[1] // includes the leading #
    const tag = rawTag.slice(1).trim() // strip #
    if (!tag || !/[\p{L}\p{N}]/u.test(tag)) continue // require at least one letter or digit

    addToSetMap(index.tagIndex, tag, filePath)
    addNamespacedParentTags(index.tagIndex, tag, filePath)
  }
}

/**
 * For a namespaced tag like `parent/child/grandchild`, also index the parent
 * segments (`parent`, `parent/child`) so that tag-folder navigation works.
 */
function addNamespacedParentTags(
  tagIndex: Map<string, Set<string>>,
  tag: string,
  filePath: string
): void {
  const parts = tag.split('/')
  if (parts.length <= 1) return

  for (let i = 1; i < parts.length; i++) {
    const parentTag = parts.slice(0, i).join('/')
    // Skip empty parent segments (e.g. when the tag starts with '/')
    if (parentTag.length === 0) continue
    addToSetMap(tagIndex, parentTag, filePath)
  }
}

/**
 * Extract block references (^block-id) from a text value.
 */
function extractBlockRefs(
  index: ExtendedSearchIndex,
  text: string,
  filePath: string,
  line: number | undefined
): void {
  const lines = text.split('\n')
  for (let i = 0; i < lines.length; i++) {
    const match = lines[i].match(BLOCK_REF_RE)
    if (match) {
      const blockId = match[1]
      const actualLine = line !== undefined ? line + i : i
      const nodeKey = `L${actualLine}`

      let fileMap = index.blockRefs.get(filePath)
      if (!fileMap) {
        fileMap = new Map<string, string>()
        index.blockRefs.set(filePath, fileMap)
      }
      fileMap.set(blockId, nodeKey)
    }
  }
}

// ---------------------------------------------------------------------------
// Private helpers — YAML frontmatter parsing
// ---------------------------------------------------------------------------

/**
 * Parse a raw YAML frontmatter string and extract tags, aliases, and other
 * key-value properties.
 *
 * Returns a flat list of property entries (each entry is [key, value]) where
 * list-valued properties have one entry per value element.
 */
function parseYamlFields(yaml: string): {
  tags: string[]
  aliases: string[]
  properties: [string, string][]
} {
  const tags: string[] = []
  const aliases: string[] = []
  const properties: [string, string][] = []

  const lines = yaml.split('\n')

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const keyMatch = line.match(/^\s*(\w[\w\s]*?)\s*:\s*(.*)$/)
    if (!keyMatch) continue

    const key = keyMatch[1].trim().toLowerCase()
    const afterColon = keyMatch[2].trim()

    if (key === 'tags') {
      // Inline array: tags: [t1, t2]
      const inlineMatch = afterColon.match(/^\[([^\]]*)\]/)
      if (inlineMatch) {
        const items = inlineMatch[1]
          .split(',')
          .map((t) => t.trim())
          .filter((t) => t.length > 0)
        tags.push(...items)
      } else if (afterColon.length > 0 && !afterColon.startsWith('-')) {
        // Single scalar — not a typical tags format, skip
      } else {
        // Block list: subsequent lines with `  - item`
        for (let j = i + 1; j < lines.length; j++) {
          const blockItem = lines[j].match(/^\s+-\s+(.*)/)
          if (blockItem) {
            tags.push(blockItem[1].trim())
          } else if (lines[j].trim().length > 0) {
            break
          }
        }
      }
    } else if (key === 'aliases') {
      // Inline array: aliases: [a1, a2]
      const inlineMatch = afterColon.match(/^\[([^\]]*)\]/)
      if (inlineMatch) {
        const items = inlineMatch[1]
          .split(',')
          .map((t) => t.trim())
          .filter((t) => t.length > 0)
        aliases.push(...items)
      } else if (afterColon.length > 0 && !afterColon.startsWith('-')) {
        // Single scalar value
        if (afterColon.length > 0) aliases.push(afterColon)
      } else if (afterColon.length === 0 || afterColon.startsWith('-')) {
        // Block list
        if (afterColon.startsWith('-') && afterColon.length > 2) {
          aliases.push(afterColon.replace(/^-\s*/, '').trim())
        }
        for (let j = i + 1; j < lines.length; j++) {
          const blockItem = lines[j].match(/^\s+-\s+(.*)/)
          if (blockItem) {
            aliases.push(blockItem[1].trim())
          } else if (lines[j].trim().length > 0) {
            break
          }
        }
      }
    } else if (key !== 'title') {
      // Other properties — skip 'title' as it's usually the note title
      // and would produce too much noise in the property index

      if (afterColon.length > 0 && afterColon.startsWith('[')) {
        // Inline array — index each element
        const items = afterColon
          .slice(1, -1)
          .split(',')
          .map((t) => t.trim())
          .filter((t) => t.length > 0)
        for (const item of items) {
          properties.push([key, item])
        }
      } else if (afterColon.length > 0 && !afterColon.startsWith('-')) {
        // Scalar value
        properties.push([key, afterColon])
      } else if (afterColon.length === 0 || afterColon.startsWith('-')) {
        // Block list
        if (afterColon.startsWith('-') && afterColon.length > 2) {
          properties.push([key, afterColon.replace(/^-\s*/, '').trim()])
        }
        for (let j = i + 1; j < lines.length; j++) {
          const blockItem = lines[j].match(/^\s+-\s+(.*)/)
          if (blockItem) {
            properties.push([key, blockItem[1].trim()])
          } else if (lines[j].trim().length > 0) {
            break
          }
        }
      }
    }
  }

  return { tags: deduplicate(tags), aliases: deduplicate(aliases), properties }
}

// ---------------------------------------------------------------------------
// Private helpers — map utilities
// ---------------------------------------------------------------------------

/**
 * Add `value` to the Set stored under `key` in a Map-of-Sets, creating the
 * Set if it doesn't exist.
 */
function addToSetMap(map: Map<string, Set<string>>, key: string, value: string): void {
  let set = map.get(key)
  if (!set) {
    set = new Set<string>()
    map.set(key, set)
  }
  set.add(value)
}

/**
 * Add `value` to the array stored under `key` in a Map-of-arrays, creating
 * the array if it doesn't exist. Skips duplicates.
 */
function addToArrayMap(map: Map<string, string[]>, key: string, value: string): void {
  let arr = map.get(key)
  if (!arr) {
    arr = []
    map.set(key, arr)
  }
  if (!arr.includes(value)) {
    arr.push(value)
  }
}

/**
 * Add a property entry to the property index.
 */
function addToPropertyIndex(
  propertyIndex: Map<string, Map<string, Set<string>>>,
  key: string,
  value: string,
  filePath: string
): void {
  if (!value) return

  let valueMap = propertyIndex.get(key)
  if (!valueMap) {
    valueMap = new Map<string, Set<string>>()
    propertyIndex.set(key, valueMap)
  }

  let paths = valueMap.get(value)
  if (!paths) {
    paths = new Set<string>()
    valueMap.set(value, paths)
  }
  paths.add(filePath)
}

/**
 * Tokenise a string the same way as the v1 full-text index:
 * lower-case, split on whitespace and Unicode punctuation, discard empty strings.
 */
function tokenize(value: string): string[] {
  return value
    .toLowerCase()
    .split(/[\s\p{P}]+/u)
    .filter((w) => w.length > 0)
}

/**
 * Deduplicate a string array while preserving insertion order.
 */
function deduplicate(arr: string[]): string[] {
  const seen = new Set<string>()
  return arr.filter((item) => {
    if (seen.has(item)) return false
    seen.add(item)
    return true
  })
}
