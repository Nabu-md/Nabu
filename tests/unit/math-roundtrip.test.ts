/**
 * math-roundtrip.test.ts
 *
 * Dedicated round-trip tests for inlineMath and math AST nodes.
 * Verifies that the parser correctly produces inlineMath/math nodes and that
 * serialization faithfully reproduces the original syntax.
 *
 * Requirements: 10.1
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import _remarkParse from 'remark-parse'
import _remarkStringify from 'remark-stringify'
import _remarkMath from 'remark-math'
import _remarkGfm from 'remark-gfm'
import type { Root } from 'mdast'

// ---------------------------------------------------------------------------
// CJS/ESM interop
// ---------------------------------------------------------------------------

function unwrap<T>(mod: any): T {
  return mod && mod.__esModule && mod.default !== undefined ? mod.default : mod
}
const remarkParse = unwrap<typeof _remarkParse>(_remarkParse)
const remarkStringify = unwrap<typeof _remarkStringify>(_remarkStringify)
const remarkMath = unwrap<typeof _remarkMath>(_remarkMath)
const remarkGfm = unwrap<typeof _remarkGfm>(_remarkGfm)

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createProcessor() {
  return unified().use(remarkParse).use(remarkGfm).use(remarkMath)
}

function createStringifyProcessor() {
  return unified()
    .use(remarkStringify, {
      bullet: '-',
      emphasis: '*',
      strong: '*',
      fence: '`',
      fences: true,
      listItemIndent: 'one'
    })
    .use(remarkGfm)
    .use(remarkMath)
}

function parse(md: string): Root {
  const proc = createProcessor()
  const ast = proc.parse(md)
  return proc.runSync(ast) as Root
}

function serialize(ast: Root): string {
  const proc = createStringifyProcessor()
  const transformed = proc.runSync(structuredClone(ast) as unknown as Root)
  return String(proc.stringify(transformed as unknown as Root))
}

/** Find a node by type in the AST. */
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
// Tests
// ---------------------------------------------------------------------------

describe('Math round-trip (Req 10.1)', () => {
  describe('inlineMath parsing', () => {
    it('parses $...$ as inlineMath nodes', () => {
      const ast = parse('The formula $E = mc^2$ is famous.')
      const inlineMathNodes = findNodes(ast, 'inlineMath')
      expect(inlineMathNodes).toHaveLength(1)
      expect(inlineMathNodes[0].type).toBe('inlineMath')
      expect(typeof inlineMathNodes[0].value).toBe('string')
      expect(inlineMathNodes[0].value).toBe('E = mc^2')
    })

    it('parses multiple $...$ expressions', () => {
      const ast = parse('$a^2 + b^2 = c^2$ and $\sum_{i=1}^{n} i$')
      const inlineMathNodes = findNodes(ast, 'inlineMath')
      expect(inlineMathNodes).toHaveLength(2)
      expect(inlineMathNodes[0].value).toContain('a^2')
      expect(inlineMathNodes[1].value).toContain('sum')
    })
  })

  describe('math (display) parsing', () => {
    it('parses $$...$$ as math nodes', () => {
      const ast = parse('$$\n\\frac{a}{b}\n$$')
      const mathNodes = findNodes(ast, 'math')
      expect(mathNodes).toHaveLength(1)
      expect(mathNodes[0].type).toBe('math')
      expect(typeof mathNodes[0].value).toBe('string')
      expect(mathNodes[0].value).toContain('frac')
    })
  })

  describe('serialization round-trip', () => {
    it('inlineMath serializes back to $...$', () => {
      const md = 'The formula $E = mc^2$ is famous.'
      const ast = parse(md)
      const serialized = serialize(ast)
      expect(serialized).toContain('$E = mc^2$')
    })

    it('display math serializes back to $$...$$', () => {
      const md = '$$\n\\frac{a}{b}\n$$'
      const ast = parse(md)
      const serialized = serialize(ast)
      expect(serialized).toContain('$$')
      expect(serialized).toContain('\\frac{a}{b}')
    })

    it('multiple math expressions round-trip correctly', () => {
      const md = '$one$ and $two$ and $$\nthree\n$$'
      const ast = parse(md)
      const serialized = serialize(ast)
      // Should contain both inline and display math markers
      expect(serialized).toContain('$one$')
      expect(serialized).toContain('$two$')
      expect(serialized).toContain('$$')
    })
  })

  describe('integration with full parser', () => {
    it('inlineMath and math nodes survive full parse→serialize→re-parse', () => {
      const md = '# Math\n\nInline: $x$.\n\n$$\ny = x^2\n$$\n\nBoth: $a$ and $$b$$.'
      const ast = parse(md)
      const serialized = serialize(ast)

      // Re-parse the serialized output
      const ast2 = parse(serialized)
      const inlineNodes = findNodes(ast2, 'inlineMath')
      const displayNodes = findNodes(ast2, 'math')

      expect(inlineNodes.length).toBeGreaterThanOrEqual(2)
      expect(displayNodes.length).toBeGreaterThanOrEqual(1)
    })
  })
})
