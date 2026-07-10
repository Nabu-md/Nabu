/**
 * Unit tests for src/main/parser.ts
 * Validates Requirements 2.1 – 2.10
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest'
import { writeFile, unlink, mkdtemp } from 'fs/promises'
import { tmpdir } from 'os'
import { join } from 'path'
import { parseFile, serializeAST, getASTMeta } from '@main/parser'
import type { Root } from 'mdast'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpDir: string

beforeAll(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), 'nabu-parser-test-'))
})

afterAll(async () => {
  // best-effort cleanup handled by OS temp dir
})

async function writeTmp(name: string, content: string | Buffer): Promise<string> {
  const filePath = join(tmpDir, name)
  await writeFile(filePath, content)
  return filePath
}

// ---------------------------------------------------------------------------
// Requirement 2.1 – Pipeline produces a valid mdast Root
// ---------------------------------------------------------------------------

describe('parseFile – basic pipeline (Req 2.1, 2.2)', () => {
  it('returns a Root node for a minimal markdown file', async () => {
    const file = await writeTmp('basic.md', '# Hello\n\nWorld\n')
    const { ast, error } = await parseFile(file)

    expect(error).toBeUndefined()
    expect(ast.type).toBe('root')
    expect(Array.isArray(ast.children)).toBe(true)
    expect(ast.children.length).toBeGreaterThan(0)
  })

  it('parses YAML front matter (remarkFrontmatter step)', async () => {
    const md = `---
title: Test
tags: [a, b]
---

# Body
`
    const file = await writeTmp('frontmatter.md', md)
    const { ast, error } = await parseFile(file)

    expect(error).toBeUndefined()
    const yamlNode = ast.children.find((n) => n.type === 'yaml') as any
    expect(yamlNode).toBeDefined()
    expect(yamlNode.value).toContain('title: Test')
  })

  it('parses GFM tables (remarkGfm step)', async () => {
    const md = `| A | B |\n|---|---|\n| 1 | 2 |\n`
    const file = await writeTmp('table.md', md)
    const { ast } = await parseFile(file)

    const tableNode = ast.children.find((n) => n.type === 'table')
    expect(tableNode).toBeDefined()
  })
})

// ---------------------------------------------------------------------------
// Requirement 2.4 – toggleBlock nodes
// ---------------------------------------------------------------------------

describe('parseFile – remarkToggleBlocks (Req 2.4)', () => {
  it('converts [toggle] headings into toggleBlock nodes', async () => {
    const md = `## [toggle] My Section\n\nSome content here\n`
    const file = await writeTmp('toggle.md', md)
    const { ast } = await parseFile(file)

    const toggle = ast.children.find((n) => n.type === 'toggleBlock') as any
    expect(toggle).toBeDefined()
    expect(toggle.heading).toBeDefined()
    expect(Array.isArray(toggle.children)).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// Requirement 2.5 – taskList / taskItem nodes
// ---------------------------------------------------------------------------

describe('parseFile – remarkTaskBlocks (Req 2.5)', () => {
  it('converts task list items into taskList node', async () => {
    const md = `- [ ] Unchecked\n- [x] Checked\n`
    const file = await writeTmp('tasks.md', md)
    const { ast } = await parseFile(file)

    const taskList = ast.children.find((n) => n.type === 'taskList') as any
    expect(taskList).toBeDefined()
    expect(taskList.items.length).toBe(2)
    expect(taskList.items[0].checked).toBe(false)
    expect(taskList.items[1].checked).toBe(true)
  })

  it('taskItem has 0-based lineIndex', async () => {
    const md = `- [ ] First\n- [x] Second\n`
    const file = await writeTmp('tasks-line.md', md)
    const { ast } = await parseFile(file)

    const taskList = ast.children.find((n) => n.type === 'taskList') as any
    for (const item of taskList.items) {
      expect(typeof item.lineIndex).toBe('number')
      expect(item.lineIndex).toBeGreaterThanOrEqual(0)
    }
  })
})

// ---------------------------------------------------------------------------
// Requirement 2.6 – wikiLink nodes
// ---------------------------------------------------------------------------

describe('parseFile – remarkWikiLinks (Req 2.6)', () => {
  it('converts [[Page Name]] into wikiLink node', async () => {
    const md = `See [[My Note]] for details.\n`
    const file = await writeTmp('wikilinks.md', md)
    const { ast } = await parseFile(file)

    // wikiLink nodes sit inside a paragraph > text sibling structure
    let found = false
    function search(node: any) {
      if (node.type === 'wikiLink') {
        found = true
        expect(node.target).toBe('My Note')
        expect(node.resolved).toBe(false)
      }
      if (node.children) node.children.forEach(search)
      if (node.items) node.items.forEach(search)
    }
    search(ast)
    expect(found).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// Requirement 2.3 – encoding detection
// ---------------------------------------------------------------------------

describe('parseFile – encoding detection (Req 2.3)', () => {
  it('detects plain UTF-8 file and attaches encoding metadata', async () => {
    const file = await writeTmp('utf8.md', '# UTF-8\n')
    const { ast } = await parseFile(file)

    const meta = getASTMeta(ast)
    expect(meta).toBeDefined()
    expect(meta!.encoding).toBe('utf-8')
  })

  it('detects UTF-8 BOM and strips it', async () => {
    // BOM bytes: EF BB BF
    const bom = Buffer.from([0xef, 0xbb, 0xbf])
    const body = Buffer.from('# BOM File\n', 'utf8')
    const file = await writeTmp('utf8bom.md', Buffer.concat([bom, body]))
    const { ast } = await parseFile(file)

    expect(ast.type).toBe('root')
    const meta = getASTMeta(ast)
    expect(meta!.encoding).toBe('utf-8')

    // Heading text must not contain BOM character
    const heading = ast.children[0] as any
    expect(heading.type).toBe('heading')
    const textNode = heading.children[0]
    expect(textNode.value).not.toContain('\ufeff')
  })

  it('skips encoding detection when detectEncoding: false', async () => {
    const file = await writeTmp('nodetect.md', '# Plain\n')
    const { ast } = await parseFile(file, { detectEncoding: false })

    // encoding metadata still set; just forced utf-8
    const meta = getASTMeta(ast)
    expect(meta!.encoding).toBe('utf-8')
  })
})

// ---------------------------------------------------------------------------
// Requirement 2.8 – error recovery
// ---------------------------------------------------------------------------

describe('parseFile – error recovery (Req 2.8)', () => {
  it('returns partial AST without throwing on well-formed input', async () => {
    // Even complex input should not throw
    const md = `# Title\n\n- [ ] task\n\n[[link]]\n\n| A | B |\n|---|---|\n| 1 | 2 |\n`
    const file = await writeTmp('complex.md', md)
    const result = await parseFile(file)

    expect(result.ast).toBeDefined()
    expect(result.ast.type).toBe('root')
  })
})

// ---------------------------------------------------------------------------
// Requirement 2.9, 2.10 – serializeAST round-trip
// ---------------------------------------------------------------------------

describe('serializeAST – round-trip fidelity (Req 2.9, 2.10)', () => {
  it('serialises a Root back to a string', async () => {
    const md = '# Hello\n\nWorld\n'
    const file = await writeTmp('rt-basic.md', md)
    const { ast } = await parseFile(file)

    const output = await serializeAST(ast)
    expect(typeof output).toBe('string')
    expect(output.trim().length).toBeGreaterThan(0)
  })

  it('round-trip: serialize then re-parse preserves heading', async () => {
    const md = '# My Heading\n\nSome paragraph.\n'
    const file = await writeTmp('rt-heading.md', md)
    const { ast: original } = await parseFile(file)

    const serialized = await serializeAST(original)

    // Write serialized output and re-parse
    const roundTripFile = await writeTmp('rt-heading-2.md', serialized)
    const { ast: reparsed } = await parseFile(roundTripFile)

    const origHeading = original.children.find((n) => n.type === 'heading') as any
    const reparsedHeading = reparsed.children.find((n) => n.type === 'heading') as any

    expect(origHeading).toBeDefined()
    expect(reparsedHeading).toBeDefined()
    expect(origHeading.depth).toBe(reparsedHeading.depth)
    expect(origHeading.children[0].value).toBe(reparsedHeading.children[0].value)
  })

  it('round-trip: serialize then re-parse preserves front matter', async () => {
    const md = `---\ntitle: Test\n---\n\n# Body\n`
    const file = await writeTmp('rt-fm.md', md)
    const { ast: original } = await parseFile(file)

    const serialized = await serializeAST(original)

    const rtFile = await writeTmp('rt-fm-2.md', serialized)
    const { ast: reparsed } = await parseFile(rtFile)

    const origYaml = original.children.find((n) => n.type === 'yaml') as any
    const rtYaml = reparsed.children.find((n) => n.type === 'yaml') as any

    expect(origYaml).toBeDefined()
    expect(rtYaml).toBeDefined()
    expect(origYaml.value).toBe(rtYaml.value)
  })

  it('round-trip: serialize then re-parse preserves GFM table structure', async () => {
    const md = `| Col A | Col B |\n|-------|-------|\n| val1  | val2  |\n`
    const file = await writeTmp('rt-table.md', md)
    const { ast: original } = await parseFile(file)

    const serialized = await serializeAST(original)

    const rtFile = await writeTmp('rt-table-2.md', serialized)
    const { ast: reparsed } = await parseFile(rtFile)

    const origTable = original.children.find((n) => n.type === 'table') as any
    const rtTable = reparsed.children.find((n) => n.type === 'table') as any

    expect(origTable).toBeDefined()
    expect(rtTable).toBeDefined()
    // Same number of rows
    expect(origTable.children.length).toBe(rtTable.children.length)
  })
})
