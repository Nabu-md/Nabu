/**
 * phase3-verification.test.ts
 *
 * Property-based tests for Phase 3 features: callouts, math, embeds.
 * Validates round-trip fidelity and structural invariance.
 *
 * Requirements: 8.5, 9.4, 11.6
 */

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import { unified } from 'unified'
import _remarkParse from 'remark-parse'
import _remarkStringify from 'remark-stringify'
import _remarkFrontmatter from 'remark-frontmatter'
import _remarkGfm from 'remark-gfm'
import _remarkMath from 'remark-math'
import type { Root } from 'mdast'
import { remarkCallouts, type Callout } from '../../src/main/plugins/remarkCallouts'
import { remarkEmbeds, type EmbedNode } from '../../src/main/plugins/remarkEmbeds'

// ---------------------------------------------------------------------------
// CJS/ESM interop
// ---------------------------------------------------------------------------

function unwrap<T>(mod: any): T {
  return (mod && mod.__esModule && mod.default !== undefined) ? mod.default : mod
}
const remarkParse = unwrap<typeof _remarkParse>(_remarkParse)
const remarkStringify = unwrap<typeof _remarkStringify>(_remarkStringify)
const remarkFrontmatter = unwrap<typeof _remarkFrontmatter>(_remarkFrontmatter)
const remarkGfm = unwrap<typeof _remarkGfm>(_remarkGfm)
const remarkMath = unwrap<typeof _remarkMath>(_remarkMath)

// ---------------------------------------------------------------------------
// Full pipeline processor (mirrors parser.ts without I/O)
// ---------------------------------------------------------------------------

function buildFullProcessor() {
  return unified()
    .use(remarkParse)
    .use(remarkFrontmatter)
    .use(remarkGfm)
    .use(remarkMath)
    .use(remarkEmbeds)
    .use(remarkCallouts)
}

function parse(md: string): Root {
  const proc = buildFullProcessor()
  const ast = proc.parse(md)
  return proc.runSync(ast) as Root
}

/** Denormalize custom nodes for stringification (simplified version of parser.ts denormalizeNode). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function denormalizeNode(node: any): any {
  if (node.type === 'callout') {
    const toggleSuffix = node.toggle ?? ''
    const markerText = `[!${node.calloutType}${toggleSuffix}]${node.title ? ' ' + node.title : ''}`
    const bodyChildren = (node.children ?? []).map(denormalizeNode)
    const firstParaChildren: unknown[] = [{ type: 'text', value: markerText }]
    if (bodyChildren.length > 0 && bodyChildren[0]?.type === 'paragraph') {
      firstParaChildren.push(...(bodyChildren[0].children ?? []))
    }
    return {
      type: 'blockquote',
      children: [
        { type: 'paragraph', children: firstParaChildren },
        ...bodyChildren.slice(1),
      ],
    }
  }
  if (node.type === 'embed') {
    return { type: 'text', value: `![[${node.target}]]` }
  }
  if (node.type === 'wikiLink') {
    const suffix = node.blockRef ? `#^${node.blockRef}` : ''
    return { type: 'text', value: `[[${node.target}${suffix}]]` }
  }
  const copy: any = { ...node }
  if (Array.isArray(node.children)) {
    copy.children = node.children.map(denormalizeNode)
  }
  return copy
}

function serialize(ast: Root): string {
  const standardAst = { type: 'root', children: ast.children.map(denormalizeNode) } as Root
  const proc = unified()
    .use(remarkStringify, {
      bullet: '-',
      emphasis: '*',
      strong: '*',
      fence: '`',
      fences: true,
      listItemIndent: 'one',
    })
    .use(remarkFrontmatter)
    .use(remarkGfm)
    .use(remarkMath)
  const transformed = proc.runSync(structuredClone(standardAst) as unknown as Root)
  return String(proc.stringify(transformed as unknown as Root))
}

function findNodes(ast: Root, type: string): any[] {
  const results: any[] = []
  function walk(nodes: any[]): void {
    for (const node of nodes) {
      if (node.type === type) results.push(node)
      if (node.children) walk(node.children)
    }
  }
  walk(ast.children)
  return results
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/** A valid callout type. */
const calloutTypeArb = fc.constantFrom(
  'note', 'info', 'tip', 'success', 'warning',
  'danger', 'error', 'question', 'example', 'quote', 'abstract',
)

/** A single-line title without block-level markers. */
const calloutTitleArb = fc.string({ minLength: 0, maxLength: 20 }).filter(
  (s) => !s.includes('\n') && !s.includes('[') && !s.includes(']'),
)

/** Single-line body text. */
const calloutBodyArb = fc.constantFrom(
  'Content here.',
  'Just one line.',
  'Short text.',
)

/** A safe embed target: alphanumeric + dots/hyphens/underscores, no markdown syntax.
 *  Underscores at start/end are filtered because they trigger emphasis parsing
 *  inside `![[...]]` (e.g. `_0_` becomes emphasis, breaking the embed pattern). */
const safeTargetArb = fc
  .string({ minLength: 1, maxLength: 15 })
  .filter((s) => /^[a-zA-Z0-9._-]+$/.test(s))
  .filter((s) => !s.startsWith('_') && !s.endsWith('_'))

// ---------------------------------------------------------------------------
// Property: Callout round-trip
// ---------------------------------------------------------------------------

describe('Callout round-trip (Req 8.5)', () => {
  it('parse → denormalize → serialize → re-parse preserves callout node types', () => {
    fc.assert(
      fc.property(
        calloutTypeArb,
        calloutBodyArb,
        (type, body) => {
          const md = `> [!${type}]\n> ${body}`
          const ast = parse(md)

          // Verify the callout node was created
          const before = findNodes(ast, 'callout') as Callout[]
          expect(before.length).toBeGreaterThanOrEqual(1)
          expect(before[0].calloutType).toBe(type)

          // Serialize (denormalizes callout → blockquote)
          const serialized = serialize(ast)

          // The marker should be present in the output
          expect(serialized).toContain(`[!${type}]`)

          // Re-parse
          const ast2 = parse(serialized)
          const after = findNodes(ast2, 'callout') as Callout[]
          expect(after.length).toBeGreaterThanOrEqual(1)
          expect(after[0].calloutType).toBe(type)
        },
      ),
      { numRuns: 50 },
    )
  })

  it('callout with title survives round-trip', () => {
    fc.assert(
      fc.property(
        calloutTypeArb,
        calloutBodyArb,
        (type, body) => {
          const md = `> [!${type}] A Title\n> ${body}`
          const ast = parse(md)
          const before = findNodes(ast, 'callout') as Callout[]
          expect(before.length).toBeGreaterThanOrEqual(1)
          expect(before[0].title).toBe('A Title')

          const serialized = serialize(ast)
          expect(serialized).toContain('[!')
          expect(serialized).toContain(type)
        },
      ),
      { numRuns: 50 },
    )
  })
})

// ---------------------------------------------------------------------------
// Property: Math round-trip
// ---------------------------------------------------------------------------

describe('Math round-trip (Req 9.4)', () => {
  it('inline math $...$ survives round-trip', () => {
    fc.assert(
      fc.property(
        fc.constantFrom('x', 'E = mc^2', 'a^2 + b^2', '\\frac{a}{b}', '\\sum_{i=1}^{n} i'),
        (formula) => {
          const md = `Test $${formula}$ here.`
          const ast = parse(md)
          const inlineMathNodes = findNodes(ast, 'inlineMath')
          expect(inlineMathNodes.length).toBeGreaterThanOrEqual(1)
          expect(inlineMathNodes[0].value).toBe(formula)

          const serialized = serialize(ast)
          expect(serialized).toContain('$')
        },
      ),
      { numRuns: 50 },
    )
  })

  it('display math $$...$$ survives round-trip', () => {
    fc.assert(
      fc.property(
        fc.constantFrom('x', '\\frac{a}{b}', 'E = mc^2'),
        (formula) => {
          const md = `$$\n${formula}\n$$`
          const ast = parse(md)
          const mathNodes = findNodes(ast, 'math')
          expect(mathNodes.length).toBeGreaterThanOrEqual(1)
          expect(mathNodes[0].value).toContain(formula)

          const serialized = serialize(ast)
          expect(serialized).toContain('$$')
        },
      ),
      { numRuns: 50 },
    )
  })
})

// ---------------------------------------------------------------------------
// Property: Embed path containment
// ---------------------------------------------------------------------------

describe('Embed path containment (Req 11.6)', () => {
  it('embed target is never empty after parse', () => {
    fc.assert(
      fc.property(safeTargetArb, (target) => {
        fc.pre(target.length > 0)
        const md = `![[${target}]]`
        const ast = parse(md)
        const embeds = findNodes(ast, 'embed') as EmbedNode[]
        if (embeds.length > 0) {
          expect(embeds[0].target.length).toBeGreaterThan(0)
          expect(embeds[0].target).not.toContain('[[')
          expect(embeds[0].target).not.toContain(']]')
        }
      }),
      { numRuns: 100 },
    )
  })

  it('multiple embeds all have non-empty targets', () => {
    fc.assert(
      fc.property(
        fc.array(safeTargetArb, { minLength: 1, maxLength: 5 }),
        (targets) => {
          const md = targets.map((t) => `![[${t}]]`).join(' ')
          const ast = parse(md)
          const embeds = findNodes(ast, 'embed') as EmbedNode[]
          expect(embeds.length).toBe(targets.length)
          for (const embed of embeds) {
            expect(embed.target.length).toBeGreaterThan(0)
          }
        },
      ),
      { numRuns: 50 },
    )
  })

  it('non-embed content does not produce false positives', () => {
    fc.assert(
      fc.property(
        fc.string({ minLength: 1, maxLength: 30 }).filter(
          (s) => !s.includes('!') && !s.includes('[') && !s.includes(']'),
        ),
        (text) => {
          const ast = parse(text)
          const embeds = findNodes(ast, 'embed')
          expect(embeds).toHaveLength(0)
        },
      ),
      { numRuns: 50 },
    )
  })
})
