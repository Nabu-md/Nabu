/**
 * Property-based tests for buildFullTextIndex() and buildTagIndex()
 *
 * Validates: Requirements 7.1, 8.1
 */

import { describe, it } from 'vitest'
import * as fc from 'fast-check'
import type { Root } from 'mdast'
import { buildFullTextIndex, buildTagIndex } from '@shared/indexing'
import type { FileEntry } from '@shared/types'

// ---- Helpers ----

/**
 * Build a minimal Root AST whose first child is a yaml frontmatter node
 * containing tags in inline-array format: `tags: [tag1, tag2]`.
 * If `tags` is empty, the yaml node omits the tags field entirely.
 */
function makeRootWithTags(tags: string[]): Root {
  const yamlValue = tags.length > 0 ? `tags: [${tags.join(', ')}]` : 'title: no tags here'

  return {
    type: 'root',
    children: [
      {
        type: 'yaml',
        value: yamlValue,
        position: undefined
      } as any
    ]
  } as Root
}

/** Generates a non-empty tag string (letters + digits, no commas/brackets). */
const tagArb = fc
  .string({ minLength: 1, maxLength: 15 })
  // Restrict to characters that are safe inside `tags: [t1, t2]` inline YAML
  .filter((s) => /^[a-zA-Z0-9_-]+$/.test(s))

/** Generates a unique file path. */
const filePathArb = (suffix: string) =>
  fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-${suffix}.md`)

/** Generates a FileEntry. */
const fileEntryArb = (suffix: string) =>
  fc.record({
    path: filePathArb(suffix),
    name: fc.string({ minLength: 1, maxLength: 20 }),
    mtime: fc.nat()
  })

// ---- Full-text AST helpers ----

/**
 * Build a Root AST that contains only plain text nodes (no yaml).
 * This represents the body of a markdown file without frontmatter.
 */
function makeRootWithText(text: string): Root {
  return {
    type: 'root',
    children: [
      {
        type: 'paragraph',
        children: [
          {
            type: 'text',
            value: text,
            position: undefined
          } as any
        ],
        position: undefined
      } as any
    ]
  } as Root
}

/**
 * Build a Root AST with a yaml frontmatter node followed by a text body.
 * Words in yamlContent appear only in the yaml node (frontmatter).
 * Words in bodyText appear in the non-frontmatter text node.
 */
function makeRootWithYamlAndText(yamlContent: string, bodyText: string): Root {
  return {
    type: 'root',
    children: [
      {
        type: 'yaml',
        value: yamlContent,
        position: undefined
      } as any,
      {
        type: 'paragraph',
        children: [
          {
            type: 'text',
            value: bodyText,
            position: undefined
          } as any
        ],
        position: undefined
      } as any
    ]
  } as Root
}

/**
 * Tokenise text the same way buildFullTextIndex does:
 * lower-case, split on whitespace + Unicode punctuation, drop empty strings.
 */
function tokenise(text: string): string[] {
  return text
    .toLowerCase()
    .split(/[\s\p{P}]+/u)
    .filter((w) => w.length > 0)
}

/** Generates a word-like string safe as a standalone token. */
const wordArb = fc.string({ minLength: 2, maxLength: 12 }).filter((s) => /^[a-zA-Z0-9]+$/.test(s))

describe('buildFullTextIndex — property-based tests', () => {
  /**
   * Validates: Requirements 7.1
   *
   * Property 1 — Lookup consistency:
   * For any word W in the index, every path stored under W belongs to a file
   * whose non-frontmatter text actually contains W.
   */
  it('Property 1 — Lookup consistency: every path under word W belongs to a file containing W', () => {
    fc.assert(
      fc.property(
        fc.array(
          fc.tuple(
            fc.record({
              path: fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-lc.md`),
              name: fc.string({ minLength: 1, maxLength: 15 }),
              mtime: fc.nat()
            }),
            fc.array(wordArb, { minLength: 0, maxLength: 6 })
          ),
          { minLength: 0, maxLength: 8 }
        ),
        (fileWordPairs) => {
          // Deduplicate by path
          const seen = new Set<string>()
          const unique: Array<{ file: FileEntry; words: string[] }> = []
          for (const [file, words] of fileWordPairs) {
            if (!seen.has(file.path)) {
              seen.add(file.path)
              unique.push({ file, words })
            }
          }

          const files = unique.map((u) => u.file)
          const wordsByPath = new Map(unique.map((u) => [u.file.path, u.words]))

          const getAST = (path: string): Root | undefined => {
            const words = wordsByPath.get(path)
            if (words === undefined) return undefined
            return makeRootWithText(words.join(' '))
          }

          const index = buildFullTextIndex(files, getAST)

          // For every word W in the index, every path must be a file containing W
          for (const [word, paths] of index) {
            for (const filePath of paths) {
              const fileWords = wordsByPath.get(filePath) ?? []
              const tokens = tokenise(fileWords.join(' '))
              if (!tokens.includes(word)) return false
            }
          }
          return true
        }
      )
    )
  })

  /**
   * Validates: Requirements 7.1
   *
   * Property 2 — Completeness:
   * For any file F and any word W extracted from F's non-yaml text,
   * index.get(W) must contain F.path.
   */
  it('Property 2 — Completeness: every (file, word) from non-yaml text appears in the index', () => {
    fc.assert(
      fc.property(
        fc.array(
          fc.tuple(
            fc.record({
              path: fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-cp.md`),
              name: fc.string({ minLength: 1, maxLength: 15 }),
              mtime: fc.nat()
            }),
            fc.array(wordArb, { minLength: 0, maxLength: 6 })
          ),
          { minLength: 0, maxLength: 8 }
        ),
        (fileWordPairs) => {
          const seen = new Set<string>()
          const unique: Array<{ file: FileEntry; words: string[] }> = []
          for (const [file, words] of fileWordPairs) {
            if (!seen.has(file.path)) {
              seen.add(file.path)
              unique.push({ file, words })
            }
          }

          const files = unique.map((u) => u.file)
          const wordsByPath = new Map(unique.map((u) => [u.file.path, u.words]))

          const getAST = (path: string): Root | undefined => {
            const words = wordsByPath.get(path)
            if (words === undefined) return undefined
            return makeRootWithText(words.join(' '))
          }

          const index = buildFullTextIndex(files, getAST)

          // For every file and every token derived from its text, the index must have the file
          for (const { file, words } of unique) {
            const tokens = tokenise(words.join(' '))
            for (const token of tokens) {
              const paths = index.get(token)
              if (!paths?.has(file.path)) return false
            }
          }
          return true
        }
      )
    )
  })

  /**
   * Validates: Requirements 7.1
   *
   * Property 3 — Frontmatter exclusion:
   * Words appearing ONLY inside a yaml node must NOT be indexed for that file.
   */
  it('Property 3 — Frontmatter exclusion: words only in yaml frontmatter are not indexed', () => {
    fc.assert(
      fc.property(
        fc.record({
          path: fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-fe.md`),
          name: fc.string({ minLength: 1, maxLength: 15 }),
          mtime: fc.nat()
        }),
        // A word that appears only in yaml and NOT in the body
        wordArb,
        // Body text that is guaranteed not to contain the yaml-only word
        fc.array(wordArb, { minLength: 1, maxLength: 5 }),
        (file, yamlOnlyWord, bodyWords) => {
          // Ensure the yaml-only word is absent from the body
          const filteredBody = bodyWords.filter(
            (w) => w.toLowerCase() !== yamlOnlyWord.toLowerCase()
          )

          const getAST = (path: string): Root | undefined => {
            if (path !== file.path) return undefined
            return makeRootWithYamlAndText(`secret: ${yamlOnlyWord}`, filteredBody.join(' '))
          }

          const index = buildFullTextIndex([file], getAST)

          // The yaml-only word must not appear in the index for this file
          const paths = index.get(yamlOnlyWord.toLowerCase())
          if (paths?.has(file.path)) return false

          return true
        }
      )
    )
  })

  /**
   * Validates: Requirements 7.1
   *
   * Property 4 — Case insensitivity:
   * Indexing a file with "TestWord" and querying index.get('testword')
   * returns a Set containing that file's path.
   */
  it('Property 4 — Case insensitivity: mixed-case words are stored and queryable in lowercase', () => {
    fc.assert(
      fc.property(
        fc.record({
          path: fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-ci.md`),
          name: fc.string({ minLength: 1, maxLength: 15 }),
          mtime: fc.nat()
        }),
        // Generate a word with at least one uppercase letter
        wordArb.filter((w) => w !== w.toLowerCase()),
        (file, mixedCaseWord) => {
          const getAST = (path: string): Root | undefined => {
            if (path !== file.path) return undefined
            return makeRootWithText(mixedCaseWord)
          }

          const index = buildFullTextIndex([file], getAST)

          const lowerWord = mixedCaseWord.toLowerCase()
          const paths = index.get(lowerWord)
          return paths?.has(file.path) === true
        }
      )
    )
  })
})

describe('buildTagIndex — property-based tests', () => {
  // ---------------------------------------------------------------------------
  // Property 1 — Tag presence
  // Every path stored under a tag T in the index belongs to a file whose
  // frontmatter actually contains T.
  // ---------------------------------------------------------------------------
  it('Property 1 — Tag presence: every path under tag T belongs to a file with tag T', () => {
    fc.assert(
      fc.property(
        // Generate up to 8 files each with up to 4 tags
        fc.array(fc.tuple(fileEntryArb('p1'), fc.array(tagArb, { minLength: 0, maxLength: 4 })), {
          minLength: 0,
          maxLength: 8
        }),
        (fileTagPairs) => {
          // Deduplicate by path
          const seen = new Set<string>()
          const unique: Array<{ file: FileEntry; tags: string[] }> = []
          for (const [file, tags] of fileTagPairs) {
            if (!seen.has(file.path)) {
              seen.add(file.path)
              unique.push({ file, tags })
            }
          }

          const files = unique.map((u) => u.file)
          const tagsByPath = new Map(unique.map((u) => [u.file.path, u.tags]))

          const getAST = (path: string): Root | undefined => {
            const tags = tagsByPath.get(path)
            if (tags === undefined) return undefined
            return makeRootWithTags(tags)
          }

          const index = buildTagIndex(files, getAST)

          // For every tag T in the index, every path under it must have T in its tags
          for (const [tag, paths] of index) {
            for (const filePath of paths) {
              const fileTags = tagsByPath.get(filePath) ?? []
              if (!fileTags.includes(tag)) return false
            }
          }
          return true
        }
      )
    )
  })

  // ---------------------------------------------------------------------------
  // Property 2 — Completeness
  // For any file F with tag T in its frontmatter, index.get(T) contains F.path.
  // ---------------------------------------------------------------------------
  it('Property 2 — Completeness: every (file, tag) pair appears in the index', () => {
    fc.assert(
      fc.property(
        fc.array(fc.tuple(fileEntryArb('p2'), fc.array(tagArb, { minLength: 0, maxLength: 4 })), {
          minLength: 0,
          maxLength: 8
        }),
        (fileTagPairs) => {
          const seen = new Set<string>()
          const unique: Array<{ file: FileEntry; tags: string[] }> = []
          for (const [file, tags] of fileTagPairs) {
            if (!seen.has(file.path)) {
              seen.add(file.path)
              unique.push({ file, tags })
            }
          }

          const files = unique.map((u) => u.file)
          const tagsByPath = new Map(unique.map((u) => [u.file.path, u.tags]))

          const getAST = (path: string): Root | undefined => {
            const tags = tagsByPath.get(path)
            if (tags === undefined) return undefined
            return makeRootWithTags(tags)
          }

          const index = buildTagIndex(files, getAST)

          // For every file + tag combination, the file must appear in the index
          for (const { file, tags } of unique) {
            for (const tag of tags) {
              const paths = index.get(tag)
              if (!paths?.has(file.path)) return false
            }
          }
          return true
        }
      )
    )
  })

  // ---------------------------------------------------------------------------
  // Property 3 — OR filter correctness
  // Given selectedTags = [T1, T2], the union of index.get(T1) and index.get(T2)
  // equals the set of files that the OR predicate would match.
  // ---------------------------------------------------------------------------
  it('Property 3 — OR filter: union of index entries matches OR-predicate result', () => {
    fc.assert(
      fc.property(
        fc.array(fc.tuple(fileEntryArb('p3'), fc.array(tagArb, { minLength: 0, maxLength: 4 })), {
          minLength: 0,
          maxLength: 10
        }),
        // Two tags to use as the OR filter
        tagArb,
        tagArb,
        (fileTagPairs, t1, t2) => {
          const seen = new Set<string>()
          const unique: Array<{ file: FileEntry; tags: string[] }> = []
          for (const [file, tags] of fileTagPairs) {
            if (!seen.has(file.path)) {
              seen.add(file.path)
              unique.push({ file, tags })
            }
          }

          const files = unique.map((u) => u.file)
          const tagsByPath = new Map(unique.map((u) => [u.file.path, u.tags]))

          const getAST = (path: string): Root | undefined => {
            const tags = tagsByPath.get(path)
            if (tags === undefined) return undefined
            return makeRootWithTags(tags)
          }

          const index = buildTagIndex(files, getAST)

          // Build the union from the index
          const unionFromIndex = new Set<string>()
          for (const tag of [t1, t2]) {
            const paths = index.get(tag)
            if (paths) {
              for (const p of paths) unionFromIndex.add(p)
            }
          }

          // Build the expected set using the OR predicate directly on file tags
          const selectedTags = [t1, t2]
          const orPredicateResult = new Set(
            unique
              .filter(({ tags }) => selectedTags.some((st) => tags.includes(st)))
              .map(({ file }) => file.path)
          )

          // Both sets must be identical
          if (unionFromIndex.size !== orPredicateResult.size) return false
          for (const p of unionFromIndex) {
            if (!orPredicateResult.has(p)) return false
          }
          return true
        }
      )
    )
  })
})
