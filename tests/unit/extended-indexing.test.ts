/**
 * Unit and property-based tests for buildExtendedIndex and
 * updateExtendedIndexForFile.
 *
 * Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8
 */

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import type { Root, Yaml } from 'mdast'
import {
  buildExtendedIndex,
  updateExtendedIndexForFile,
  createEmptyIndex
} from '@shared/extended-indexing'
import type { ExtendedSearchIndex } from '@shared/extended-indexing'
import type { FileEntry } from '@shared/types'

// ---------------------------------------------------------------------------
// AST factory helpers
// ---------------------------------------------------------------------------

/**
 * Build a Root AST with a yaml frontmatter node and one or more paragraph
 * text nodes at the given lines.
 */
function makeRoot(opts: { yaml?: string; paragraphs?: { line: number; text: string }[] }): Root {
  const children: any[] = []

  if (opts.yaml !== undefined) {
    children.push({
      type: 'yaml',
      value: opts.yaml,
      position: { start: { line: 1, column: 1 }, end: { line: 1, column: opts.yaml.length + 1 } }
    } as Yaml & { position: any })
  }

  if (opts.paragraphs) {
    // Track the current line offset. The yaml node occupies line 1, so text
    // nodes start at line 2 if yaml is present, or line 1 if not.
    let currentLine = opts.yaml !== undefined ? 2 : 1
    for (const p of opts.paragraphs) {
      const textNode: any = {
        type: 'text',
        value: p.text,
        position: {
          start: { line: p.line ?? currentLine, column: 1 },
          end: { line: p.line ?? currentLine, column: p.text.length + 1 }
        }
      }
      const paraNode: any = {
        type: 'paragraph',
        children: [textNode],
        position: {
          start: { line: p.line ?? currentLine, column: 1 },
          end: { line: p.line ?? currentLine, column: p.text.length + 1 }
        }
      }
      children.push(paraNode)
      currentLine = (p.line ?? currentLine) + 1
    }
  }

  return { type: 'root', children } as unknown as Root
}

/**
 * Build a Root AST containing inline code and fenced code nodes to test
 * that inline tags inside code are skipped.
 */
function makeRootWithCodeBlocks(opts: {
  textWithTags?: { line: number; text: string }
  inlineCode?: { line: number; text: string }
  codeBlock?: { line: number; text: string }
}): Root {
  const children: any[] = []

  if (opts.textWithTags) {
    children.push({
      type: 'paragraph',
      children: [
        {
          type: 'text',
          value: opts.textWithTags.text,
          position: {
            start: { line: opts.textWithTags.line, column: 1 },
            end: { line: opts.textWithTags.line, column: opts.textWithTags.text.length + 1 }
          }
        }
      ],
      position: {
        start: { line: opts.textWithTags.line, column: 1 },
        end: { line: opts.textWithTags.line, column: opts.textWithTags.text.length + 1 }
      }
    })
  }

  if (opts.inlineCode) {
    children.push({
      type: 'paragraph',
      children: [
        {
          type: 'inlineCode',
          value: opts.inlineCode.text,
          position: {
            start: { line: opts.inlineCode.line, column: 1 },
            end: { line: opts.inlineCode.line, column: opts.inlineCode.text.length + 6 }
          }
        }
      ],
      position: {
        start: { line: opts.inlineCode.line, column: 1 },
        end: { line: opts.inlineCode.line, column: opts.inlineCode.text.length + 6 }
      }
    })
  }

  if (opts.codeBlock) {
    children.push({
      type: 'code',
      lang: 'js',
      value: opts.codeBlock.text,
      position: {
        start: { line: opts.codeBlock.line, column: 1 },
        end: { line: opts.codeBlock.line + opts.codeBlock.text.split('\n').length, column: 3 }
      }
    })
  }

  return { type: 'root', children } as unknown as Root
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const fileEntry = (path: string, name?: string): FileEntry => ({
  path,
  name: name ?? path.split('/').pop() ?? 'note',
  mtime: Date.now()
})

function astGetter(astMap: Map<string, Root>): (path: string) => Root | undefined {
  return (path: string) => astMap.get(path)
}

// ---------------------------------------------------------------------------
// Tests — buildExtendedIndex
// ---------------------------------------------------------------------------

describe('buildExtendedIndex', () => {
  // -------------------------------------------------------------------------
  // Req 2.1 — Token positions
  // -------------------------------------------------------------------------
  it('records line numbers for tokens (Req 2.1)', () => {
    const root = makeRoot({
      paragraphs: [
        { line: 2, text: 'hello world' },
        { line: 4, text: 'hello again' }
      ],
      yaml: 'title: test'
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    // 'hello' appears on lines 2 and 4
    expect(index.positions.get('hello')?.get('/vault/note.md')).toEqual([2, 4])
    // 'world' appears on line 2 only
    expect(index.positions.get('world')?.get('/vault/note.md')).toEqual([2])
    // 'again' appears on line 4 only
    expect(index.positions.get('again')?.get('/vault/note.md')).toEqual([4])
  })

  it('deduplicates line numbers for tokens on the same line (Req 2.1)', () => {
    // Two separate text nodes on the same line
    const root = {
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            {
              type: 'text',
              value: 'hello world',
              position: { start: { line: 2, column: 1 }, end: { line: 2, column: 12 } }
            },
            {
              type: 'text',
              value: 'hello again',
              position: { start: { line: 2, column: 13 }, end: { line: 2, column: 24 } }
            }
          ],
          position: { start: { line: 2, column: 1 }, end: { line: 2, column: 24 } }
        }
      ]
    } as unknown as Root

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    // 'hello' appears only once in line 2 (deduplicated)
    expect(index.positions.get('hello')?.get('/vault/note.md')).toEqual([2])
    // 'world' and 'again' each appear once
    expect(index.positions.get('world')?.get('/vault/note.md')).toEqual([2])
    expect(index.positions.get('again')?.get('/vault/note.md')).toEqual([2])
  })

  // -------------------------------------------------------------------------
  // Req 2.2 — Line snippets
  // -------------------------------------------------------------------------
  it('captures line snippets for indexed lines (Req 2.2)', () => {
    const root = makeRoot({
      paragraphs: [
        { line: 2, text: 'hello world' },
        { line: 4, text: 'foo bar baz' }
      ]
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    const snippets = index.lineSnippets.get('/vault/note.md')
    expect(snippets).toBeDefined()
    // Line 2 (index 1) → "hello world" (at least 4 elements in array, line 1 empty)
    expect(snippets![1]).toBe('hello world')
    // Line 4 (index 3) → "foo bar baz"
    expect(snippets![3]).toBe('foo bar baz')
  })

  it('truncates line snippets at SNIPPET_MAX_LENGTH (Req 2.2)', () => {
    const longText = 'x'.repeat(300)
    const root = makeRoot({ paragraphs: [{ line: 1, text: longText }] })
    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    const snippets = index.lineSnippets.get('/vault/note.md')
    expect(snippets![0].length).toBe(120) // SNIPPET_MAX_LENGTH
  })

  // -------------------------------------------------------------------------
  // Req 2.3 — Inline #tag extraction
  // -------------------------------------------------------------------------
  it('extracts inline #tags from text nodes (Req 2.3)', () => {
    const root = makeRoot({
      paragraphs: [{ line: 2, text: 'This is a #tag and another #project/feature' }]
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.tagIndex.has('tag')).toBe(true)
    expect(index.tagIndex.get('tag')!.has('/vault/note.md')).toBe(true)
    expect(index.tagIndex.has('project/feature')).toBe(true)
    expect(index.tagIndex.get('project/feature')!.has('/vault/note.md')).toBe(true)
  })

  it('skips inline tags inside inlineCode and code blocks', () => {
    const root = makeRootWithCodeBlocks({
      textWithTags: { line: 2, text: 'valid #tag here' },
      inlineCode: { line: 3, text: 'should #notBeTag' },
      codeBlock: { line: 4, text: 'also #notTagged' }
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.tagIndex.has('tag')).toBe(true)
    expect(index.tagIndex.has('notBeTag')).toBe(false)
    expect(index.tagIndex.has('notTagged')).toBe(false)
  })

  it('does not treat ## heading as an inline tag', () => {
    const root = makeRoot({ paragraphs: [{ line: 2, text: '## Section Title' }] })
    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    // '##' should not match as a tag
    for (const tag of index.tagIndex.keys()) {
      expect(tag.startsWith('#')).toBe(false)
    }
  })

  // -------------------------------------------------------------------------
  // Req 2.4 — Namespaced tag parent segments
  // -------------------------------------------------------------------------
  it('indexes parent segments for namespaced inline tags (Req 2.4)', () => {
    const root = makeRoot({
      paragraphs: [{ line: 2, text: '#project/feature/sub' }]
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    // Full path indexed
    expect(index.tagIndex.has('project/feature/sub')).toBe(true)
    // Parent segments indexed
    expect(index.tagIndex.has('project')).toBe(true)
    expect(index.tagIndex.has('project/feature')).toBe(true)
  })

  it('indexes parent segments for namespaced frontmatter tags (Req 2.4)', () => {
    const root = makeRoot({
      yaml: 'tags: [parent/child]'
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.tagIndex.has('parent/child')).toBe(true)
    expect(index.tagIndex.has('parent')).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Req 2.5 — Alias map
  // -------------------------------------------------------------------------
  it('extracts aliases from YAML inline array (Req 2.5)', () => {
    const root = makeRoot({
      yaml: 'aliases: [My Note, Reference]'
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.aliasIndex.get('my note')).toEqual(['/vault/note.md'])
    expect(index.aliasIndex.get('reference')).toEqual(['/vault/note.md'])
  })

  it('extracts aliases from YAML block list (Req 2.5)', () => {
    const root = makeRoot({
      yaml: 'aliases:\n  - Alias One\n  - Alias Two'
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.aliasIndex.get('alias one')).toEqual(['/vault/note.md'])
    expect(index.aliasIndex.get('alias two')).toEqual(['/vault/note.md'])
  })

  // -------------------------------------------------------------------------
  // Req 2.7 — Frontmatter exclusion from token positions
  // -------------------------------------------------------------------------
  it('excludes yaml frontmatter content from token positions (Req 2.7)', () => {
    const root = makeRoot({
      yaml: 'title: secret\nstatus: active',
      paragraphs: [{ line: 3, text: 'visible content' }]
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    // 'secret' and 'active' appear ONLY in yaml — should NOT be indexed
    expect(index.positions.has('secret')).toBe(false)
    expect(index.positions.has('active')).toBe(false)

    // 'visible' and 'content' appear in body text — should be indexed
    expect(index.positions.get('visible')?.get('/vault/note.md')).toEqual([3])
    expect(index.positions.get('content')?.get('/vault/note.md')).toEqual([3])
  })

  // -------------------------------------------------------------------------
  // Unified tag index — frontmatter + inline
  // -------------------------------------------------------------------------
  it('unifies frontmatter tags and inline tags in the same index (Req 2.3)', () => {
    const root = makeRoot({
      yaml: 'tags: [frontmatter-tag]',
      paragraphs: [{ line: 3, text: 'inline #inline-tag here' }]
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.tagIndex.has('frontmatter-tag')).toBe(true)
    expect(index.tagIndex.has('inline-tag')).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Property index
  // -------------------------------------------------------------------------
  it('builds property index from YAML frontmatter', () => {
    const root = makeRoot({
      yaml: 'status: active\nowner: alice\npriority: high'
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.propertyIndex.get('status')?.get('active')?.has('/vault/note.md')).toBe(true)
    expect(index.propertyIndex.get('owner')?.get('alice')?.has('/vault/note.md')).toBe(true)
    expect(index.propertyIndex.get('priority')?.get('high')?.has('/vault/note.md')).toBe(true)
  })

  it('indexes array properties as individual entries', () => {
    const root = makeRoot({
      yaml: 'genres: [rock, jazz, blues]'
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.propertyIndex.get('genres')?.get('rock')?.has('/vault/note.md')).toBe(true)
    expect(index.propertyIndex.get('genres')?.get('jazz')?.has('/vault/note.md')).toBe(true)
    expect(index.propertyIndex.get('genres')?.get('blues')?.has('/vault/note.md')).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Block references
  // -------------------------------------------------------------------------
  it('extracts block references (^id) from text', () => {
    const root = makeRoot({
      paragraphs: [
        { line: 2, text: 'Some paragraph content. ^ref-1' },
        { line: 3, text: 'Another line with ^ref-2' }
      ]
    })

    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))

    expect(index.blockRefs.get('/vault/note.md')?.has('ref-1')).toBe(true)
    expect(index.blockRefs.get('/vault/note.md')?.has('ref-2')).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Multiple files
  // -------------------------------------------------------------------------
  it('handles multiple files correctly', () => {
    const ast1 = makeRoot({ paragraphs: [{ line: 2, text: 'alpha beta' }] })
    const ast2 = makeRoot({ paragraphs: [{ line: 3, text: 'beta gamma' }] })

    const files = [fileEntry('/vault/a.md'), fileEntry('/vault/b.md')]
    const asts = new Map([
      ['/vault/a.md', ast1],
      ['/vault/b.md', ast2]
    ])
    const index = buildExtendedIndex(files, astGetter(asts))

    // 'beta' appears in both files
    const betaPositions = index.positions.get('beta')
    expect(betaPositions?.has('/vault/a.md')).toBe(true)
    expect(betaPositions?.has('/vault/b.md')).toBe(true)

    // 'alpha' only in a.md
    expect(index.positions.get('alpha')?.has('/vault/a.md')).toBe(true)
    expect(index.positions.get('alpha')?.has('/vault/b.md')).toBe(false)

    // 'gamma' only in b.md
    expect(index.positions.get('gamma')?.has('/vault/b.md')).toBe(true)
    expect(index.positions.get('gamma')?.has('/vault/a.md')).toBe(false)
  })

  // -------------------------------------------------------------------------
  // Edge cases
  // -------------------------------------------------------------------------
  it('returns empty index for empty file list', () => {
    const index = buildExtendedIndex([], astGetter(new Map()))
    expect(index.positions.size).toBe(0)
    expect(index.lineSnippets.size).toBe(0)
    expect(index.tagIndex.size).toBe(0)
    expect(index.aliasIndex.size).toBe(0)
    expect(index.propertyIndex.size).toBe(0)
    expect(index.blockRefs.size).toBe(0)
  })

  it('skips files with undefined AST', () => {
    const files = [fileEntry('/vault/missing.md')]
    const index = buildExtendedIndex(files, astGetter(new Map()))
    expect(index.positions.size).toBe(0)
  })

  it('skips empty text values', () => {
    const root = makeRoot({ paragraphs: [{ line: 2, text: '' }] })
    const files = [fileEntry('/vault/note.md')]
    const asts = new Map([['/vault/note.md', root]])
    const index = buildExtendedIndex(files, astGetter(asts))
    expect(index.positions.size).toBe(0)
  })
})

// ---------------------------------------------------------------------------
// Tests — updateExtendedIndexForFile
// ---------------------------------------------------------------------------

describe('updateExtendedIndexForFile', () => {
  it('removes a file from all sub-indexes when ast is undefined (Req 2.6)', () => {
    const root1 = makeRoot({
      yaml: 'tags: [tag-a]',
      paragraphs: [{ line: 2, text: 'hello world' }]
    })
    const root2 = makeRoot({
      yaml: 'aliases: [My Note]',
      paragraphs: [{ line: 3, text: 'foo bar' }]
    })

    const files = [fileEntry('/vault/a.md'), fileEntry('/vault/b.md')]
    const asts = new Map([
      ['/vault/a.md', root1],
      ['/vault/b.md', root2]
    ])
    const index = buildExtendedIndex(files, astGetter(asts))

    // Remove /vault/a.md
    updateExtendedIndexForFile(index, '/vault/a.md', undefined)

    // a.md should be gone from positions
    expect(index.positions.get('hello')?.has('/vault/a.md')).toBeUndefined()
    expect(index.positions.get('world')?.has('/vault/a.md')).toBeUndefined()
    // Clean up empty word entries
    expect(index.positions.has('hello')).toBe(false)
    expect(index.positions.has('world')).toBe(false)

    // a.md should be gone from lineSnippets
    expect(index.lineSnippets.has('/vault/a.md')).toBe(false)

    // a.md should be gone from tagIndex
    expect(index.tagIndex.has('tag-a')).toBe(false)

    // b.md entries should survive
    expect(index.positions.get('foo')?.has('/vault/b.md')).toBe(true)
    expect(index.positions.get('bar')?.has('/vault/b.md')).toBe(true)
    expect(index.aliasIndex.get('my note')).toEqual(['/vault/b.md'])
    expect(index.lineSnippets.has('/vault/b.md')).toBe(true)
  })

  it('re-indexes a file when a new AST is provided (Req 2.6)', () => {
    const initialState = makeRoot({
      yaml: 'tags: [old-tag]',
      paragraphs: [{ line: 2, text: 'old content' }]
    })
    const updatedState = makeRoot({
      yaml: 'tags: [new-tag]',
      paragraphs: [{ line: 5, text: 'new content' }]
    })

    const files = [fileEntry('/vault/a.md'), fileEntry('/vault/b.md')]
    const asts = new Map([
      ['/vault/a.md', initialState],
      ['/vault/b.md', initialState]
    ])
    const index = buildExtendedIndex(files, astGetter(asts))

    // Update /vault/a.md with new content
    updateExtendedIndexForFile(index, '/vault/a.md', updatedState)

    // Old entries for a.md should be gone
    expect(index.tagIndex.has('old-tag')).toBe(true) // still has b.md
    expect(index.positions.get('old')?.has('/vault/a.md')).toBe(false)
    expect(index.positions.get('content')?.get('/vault/a.md')).toEqual([5]) // updated line

    // New entries should be present
    expect(index.tagIndex.get('new-tag')?.has('/vault/a.md')).toBe(true)

    // b.md should be untouched
    expect(index.tagIndex.get('old-tag')?.has('/vault/b.md')).toBe(true)
  })

  it('idempotence: update then remove produces same index as full rebuild without file', () => {
    const buildAst = (text: string) => makeRoot({ paragraphs: [{ line: 2, text }] })

    const astA = buildAst('alpha')
    const astB = buildAst('beta')
    const astC = buildAst('gamma')

    const allFiles = [fileEntry('/vault/a.md'), fileEntry('/vault/b.md'), fileEntry('/vault/c.md')]
    const allAsts = new Map([
      ['/vault/a.md', astA],
      ['/vault/b.md', astB],
      ['/vault/c.md', astC]
    ])

    // Full rebuild including all three files
    const fullIndex = buildExtendedIndex(allFiles, astGetter(allAsts))

    // Remove c.md via incremental update
    updateExtendedIndexForFile(fullIndex, '/vault/c.md', undefined)

    // Full rebuild without c.md
    const twoFiles = allFiles.filter((f) => f.path !== '/vault/c.md')
    const twoAsts = new Map(allAsts)
    twoAsts.delete('/vault/c.md')
    const referenceIndex = buildExtendedIndex(twoFiles, astGetter(twoAsts))

    // The incremental-update result should match the full rebuild
    // Compare positions
    expect(fullIndex.positions).toEqual(referenceIndex.positions)
    // Compare lineSnippets
    expect(fullIndex.lineSnippets).toEqual(referenceIndex.lineSnippets)
  })

  it('idempotence: update with changed AST matches full rebuild with changed file', () => {
    const buildAst = (text: string) => makeRoot({ paragraphs: [{ line: 2, text }] })

    const astA = buildAst('alpha')
    const astB = buildAst('beta')

    // Build index with a.md = 'alpha'
    const files = [fileEntry('/vault/a.md')]
    const asts = new Map([['/vault/a.md', astA]])
    const index = buildExtendedIndex(files, astGetter(asts))

    // Incrementally update a.md to 'beta'
    updateExtendedIndexForFile(index, '/vault/a.md', astB)

    // Build fresh index with a.md = 'beta'
    const freshAsts = new Map([['/vault/a.md', astB]])
    const freshIndex = buildExtendedIndex(files, astGetter(freshAsts))

    // Should match
    expect(index.positions).toEqual(freshIndex.positions)
    expect(index.lineSnippets).toEqual(freshIndex.lineSnippets)
  })

  it('handles deleting a file that has no entries', () => {
    const index = createEmptyIndex()
    const result = updateExtendedIndexForFile(index, '/vault/nonexistent.md', undefined)
    expect(result).toBe(index) // mutated in place
    expect(index.positions.size).toBe(0)
  })
})

// ---------------------------------------------------------------------------
// Property-based tests
// ---------------------------------------------------------------------------

describe('buildExtendedIndex - property-based', () => {
  /** Generates a word-like string safe as a standalone token. */
  const wordArb = fc.string({ minLength: 2, maxLength: 12 }).filter((s) => /^[a-zA-Z]+$/.test(s))

  /**
   * Property: Lookup consistency for positions (Req 2.1).
   * For any word W in positions, every file F stored under W must have a
   * parsed AST whose non-yaml text actually contains W.
   */
  it('every file under position entry for word W actually contains W in non-yaml text', () => {
    fc.assert(
      fc.property(
        fc.array(
          fc.record({
            path: fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-lc.md`),
            name: fc.string({ minLength: 1, maxLength: 15 }),
            mtime: fc.nat()
          }),
          { minLength: 0, maxLength: 6 }
        ),
        fc.array(wordArb, { minLength: 0, maxLength: 8 }),
        (files, words) => {
          if (files.length === 0) return true

          const asts = new Map<string, Root>()
          for (const file of files) {
            asts.set(
              file.path,
              makeRoot({
                paragraphs: [{ line: 2, text: words.join(' ') }]
              })
            )
          }

          const index = buildExtendedIndex(files, astGetter(asts))
          const allText = words.join(' ').toLowerCase()

          for (const [word, fileMap] of index.positions) {
            for (const filePath of fileMap.keys()) {
              if (!allText.includes(word)) return false
            }
          }
          return true
        }
      )
    )
  })

  /**
   * Property: Frontmatter exclusion for positions (Req 2.7).
   * A word that appears ONLY in the YAML frontmatter must NOT appear in
   * positions for that file.
   */
  it('words exclusive to yaml frontmatter are not in positions (Req 2.7)', () => {
    fc.assert(
      fc.property(
        wordArb,
        fc.array(wordArb, { minLength: 1, maxLength: 5 }),
        (yamlOnlyWord, bodyWords) => {
          const file = fileEntry('/vault/test.md')
          const filteredBody = bodyWords.filter(
            (w) => w.toLowerCase() !== yamlOnlyWord.toLowerCase()
          )

          const ast = makeRoot({
            yaml: `secret: ${yamlOnlyWord}`,
            paragraphs: [{ line: 3, text: filteredBody.join(' ') }]
          })

          const asts = new Map([['/vault/test.md', ast]])
          const index = buildExtendedIndex([file], astGetter(asts))

          // The yaml-only word must not appear in positions for this file
          const fileMap = index.positions.get(yamlOnlyWord.toLowerCase())
          if (fileMap?.has('/vault/test.md')) return false

          return true
        }
      )
    )
  })

  /**
   * Property: Tag normalisation (Req 2.4).
   * All tags in tagIndex have no leading # and are non-empty.
   */
  it('all tags in tagIndex are normalised (no leading #, non-empty)', () => {
    fc.assert(
      fc.property(
        fc.array(
          fc.record({
            path: fc.string({ minLength: 1, maxLength: 20 }).map((s) => `/vault/${s}-tn.md`),
            name: fc.string({ minLength: 1, maxLength: 15 }),
            mtime: fc.nat()
          }),
          { minLength: 1, maxLength: 5 }
        ),
        fc.string({ minLength: 1, maxLength: 10 }).filter((s) => /^[a-zA-Z0-9/-]+$/.test(s)),
        (files, tagBody) => {
          const tags = [`#${tagBody}`]
          const ast = makeRoot({
            paragraphs: [{ line: 2, text: `tag ${tags.join(' ')} here` }]
          })
          const asts = new Map<string, Root>()
          for (const file of files) {
            asts.set(file.path, ast)
          }

          const index = buildExtendedIndex(files, astGetter(asts))

          for (const tag of index.tagIndex.keys()) {
            if (tag.startsWith('#')) return false
            if (tag.length === 0) return false
          }
          return true
        }
      )
    )
  })
})
