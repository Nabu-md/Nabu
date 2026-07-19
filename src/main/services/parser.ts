/**
 * parser.ts
 *
 * Unified/remark pipeline for parsing Markdown files into mdast ASTs.
 * Implements encoding detection (BOM → UTF-8 → system locale fallback),
 * custom plugin pipeline, error recovery, and AST serialisation.
 *
 * Requirements: 2.1 – 2.10
 */

import { readFile } from 'fs/promises'
import { unified } from 'unified'
import _remarkStringify from 'remark-stringify'
import _remarkFrontmatter from 'remark-frontmatter'
import _remarkGfm from 'remark-gfm'
import _remarkMath from 'remark-math'
import type { Root } from 'mdast'
import type { VFile } from 'vfile'

// CJS/ESM interop: electron-vite bundles ESM packages as CJS require() calls
// which return { __esModule: true, default: fn }. Unwrap .default if needed.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function unwrap<T>(mod: any): T {
  return mod && mod.__esModule && mod.default !== undefined ? mod.default : mod
}
const remarkStringify = unwrap<typeof _remarkStringify>(_remarkStringify)
const remarkFrontmatter = unwrap<typeof _remarkFrontmatter>(_remarkFrontmatter)
const remarkGfm = unwrap<typeof _remarkGfm>(_remarkGfm)
const remarkMath = unwrap<typeof _remarkMath>(_remarkMath)

// Import the shared buildProcessor for the markdown pipeline
import { buildProcessor } from '@shared/markdown'

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type EncodingLabel = 'utf-8' | 'utf-16' | 'system'

export interface ParseOptions {
  /** Whether to attempt automatic encoding detection. Default: true */
  detectEncoding?: boolean
}

export interface ParserResult {
  /** mdast Root node (may be partial if a parse error occurred) */
  ast: Root
  /** Present when the file could not be fully parsed */
  error?: { line: number; column: number; message: string }
}

// ---------------------------------------------------------------------------
// AST metadata helpers
// ---------------------------------------------------------------------------

/** Symbol used to attach parser metadata to the Root node without polluting
 *  the serialised output. */
const META_KEY = Symbol('parserMeta')

interface ParserMeta {
  encoding: EncodingLabel
}

/** Attach encoding info to the AST root (survives in-process use; not serialised). */
function attachMeta(ast: Root, meta: ParserMeta): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ;(ast as any)[META_KEY] = meta
}

/** Retrieve encoding info previously attached to the AST root. */
export function getASTMeta(ast: Root): ParserMeta | undefined {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (ast as any)[META_KEY] as ParserMeta | undefined
}

// ---------------------------------------------------------------------------
// Encoding detection
// ---------------------------------------------------------------------------

/**
 * Detect encoding from a raw buffer and return the decoded string plus the
 * detected encoding label.
 *
 * Strategy (Requirement 2.3):
 *   1. Check for UTF-8 BOM (EF BB BF) or UTF-16 BOM (FF FE / FE FF).
 *   2. Attempt UTF-8 decoding via TextDecoder (fatal mode – throws on invalid bytes).
 *   3. Fall back to the system locale encoding ('latin1' node buffer default).
 */
function detectAndDecode(buffer: Buffer): { content: string; encoding: EncodingLabel } {
  // --- BOM checks ---
  if (buffer[0] === 0xef && buffer[1] === 0xbb && buffer[2] === 0xbf) {
    // UTF-8 BOM – strip it and decode as UTF-8
    return {
      content: buffer.slice(3).toString('utf8'),
      encoding: 'utf-8'
    }
  }

  if ((buffer[0] === 0xff && buffer[1] === 0xfe) || (buffer[0] === 0xfe && buffer[1] === 0xff)) {
    // UTF-16 BOM
    const bom = buffer[0] === 0xff ? 'utf16le' : 'utf16le' // Node only supports LE natively
    return {
      content: buffer.toString(bom),
      encoding: 'utf-16'
    }
  }

  // --- Attempt strict UTF-8 ---
  try {
    const decoder = new TextDecoder('utf-8', { fatal: true })
    const content = decoder.decode(buffer)
    return { content, encoding: 'utf-8' }
  } catch {
    // --- Fall back to system locale (latin1 / binary) ---
    return {
      content: buffer.toString('latin1'),
      encoding: 'system'
    }
  }
}

// ---------------------------------------------------------------------------
// Core public API
// ---------------------------------------------------------------------------

/**
 * Parse a Markdown file on disk and return an mdast Root plus optional error.
 *
 * Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8
 */
export async function parseFile(
  filePath: string,
  options: ParseOptions = {}
): Promise<ParserResult> {
  const detectEncoding = options.detectEncoding !== false // default true

  // 1. Read raw bytes
  const buffer = await readFile(filePath)

  // 2. Encoding detection (Requirement 2.3)
  const { content, encoding } = detectEncoding
    ? detectAndDecode(buffer)
    : { content: buffer.toString('utf8'), encoding: 'utf-8' as EncodingLabel }

  // 3. Build pipeline and attempt parse (Requirement 2.1, 2.2)
  const processor = buildProcessor()

  let ast: Root
  let parseError: ParserResult['error']

  try {
    // unified's parse step is synchronous; run is async (transforms)
    const rawAst = processor.parse(content) as Root
    ast = (await processor.run(rawAst)) as Root
  } catch (err) {
    // 4. Error recovery – return partial AST + error node (Requirement 2.8)
    const message = err instanceof Error ? err.message : String(err)

    // Try to extract line/column from the error (unified wraps VFile messages)
    let line = 1
    let column = 1

    if (err instanceof Error) {
      // unified errors sometimes embed position in the message
      const posMatch = /(\d+):(\d+)/.exec(err.message)
      if (posMatch) {
        line = parseInt(posMatch[1], 10)
        column = parseInt(posMatch[2], 10)
      }

      // Check VFile-style error with .position property
      const vfileErr = err as Error & { position?: { start?: { line?: number; column?: number } } }
      if (vfileErr.position?.start) {
        line = vfileErr.position.start.line ?? line
        column = vfileErr.position.start.column ?? column
      }
    }

    parseError = { line, column, message }

    // Build a best-effort partial AST from whatever was parsed
    try {
      ast = processor.parse(content) as Root
    } catch {
      // Absolute fallback: empty root
      ast = { type: 'root', children: [] }
    }
  }

  // 5. Store encoding in AST metadata (Requirement 2.3, 2.4)
  attachMeta(ast, { encoding })

  return { ast, error: parseError }
}

// ---------------------------------------------------------------------------
// Serialisation helpers – reverse transforms for custom nodes
// ---------------------------------------------------------------------------

/**
 * Deeply clone a node tree while converting custom plugin nodes back to
 * standard mdast nodes that remark-stringify understands.
 *
 * Custom nodes produced by our plugins:
 *   • callout      → blockquote with > [!type] marker (and +/- toggle) restored
 *   • toggleBlock  → heading (with "[toggle] " prefix restored) + children
 *   • taskList     → list of listItems with `checked` property
 *   • wikiLink     → text node containing the original [[target]] syntax
 *
 * Requirements: 2.9, 2.10
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function denormalizeNode(node: any): any {
  // ── callout → blockquote with restored marker ─────────────────────────
  if (node.type === 'callout') {
    const toggleSuffix = node.toggle ?? ''
    const markerText = `[!${node.calloutType}${toggleSuffix}]${node.title ? ' ' + node.title : ''}`

    // Denormalize body children (recurses into nested structures).
    const bodyChildren = denormalizeChildren(node.children ?? [])

    // First paragraph contains the marker as a text node, followed by
    // any inline content that was part of the original first line.
    // If the first body child is a paragraph we inline its children here
    // so they appear on the same `> [!type] Title` line.
    const firstParaChildren: unknown[] = [{ type: 'text', value: markerText }]

    if (bodyChildren.length > 0 && bodyChildren[0]?.type === 'paragraph') {
      firstParaChildren.push(...(bodyChildren[0].children ?? []))
    }

    const blockquoteChildren: unknown[] = [
      { type: 'paragraph', children: firstParaChildren },
      ...bodyChildren.slice(1)
    ]

    return {
      type: 'blockquote',
      children: blockquoteChildren
    }
  }

  // ── toggleBlock → heading + sibling children ──────────────────────────
  // We can't expand one node into multiple at this level, so we wrap in a
  // "virtual root" that will be spliced in by the parent.  The caller
  // (denormalizeChildren) handles this by detecting the sentinel.
  if (node.type === 'toggleBlock') {
    const heading = denormalizeNode({
      ...node.heading,
      // Restore [toggle] prefix
      children: [
        {
          type: 'text',
          value: `[toggle] ${node.heading?.children?.[0]?.value ?? ''}`.trimEnd()
        },
        ...(node.heading?.children?.slice(1) ?? [])
      ]
    })
    const kids = denormalizeChildren(node.children ?? [])
    return { __expanded: true, nodes: [heading, ...kids] }
  }

  // ── taskList → GFM list ───────────────────────────────────────────────
  if (node.type === 'taskList') {
    const items = (node.items ?? []).map((item: any) => ({
      type: 'listItem',
      checked: item.checked,
      spread: false,
      children: [
        {
          type: 'paragraph',
          children: denormalizeChildren(item.children ?? [])
        }
      ]
    }))
    return {
      type: 'list',
      ordered: false,
      spread: false,
      children: items
    }
  }

  // ── embed → text ─────────────────────────────────────────────────────
  if (node.type === 'embed') {
    return { type: 'text', value: `![[${(node as { target: string }).target}]]` }
  }

  // ── wikiLink → text ───────────────────────────────────────────────────
  if (node.type === 'wikiLink') {
    const linkNode = node as { target: string; blockRef?: string }
    const suffix = linkNode.blockRef ? `#^${linkNode.blockRef}` : ''
    return { type: 'text', value: `[[${linkNode.target}${suffix}]]` }
  }

  // ── Recurse into standard nodes ───────────────────────────────────────
  const copy: any = { ...node }

  if (Array.isArray(node.children)) {
    copy.children = denormalizeChildren(node.children)
  }

  // Append trailing `^blockId` to last text child for round-trip
  const blockId: string | undefined = (node.data as Record<string, unknown> | undefined)
    ?.blockId as string | undefined
  if (blockId && Array.isArray(copy.children) && copy.children.length > 0) {
    const lastChild = copy.children[copy.children.length - 1]
    if (lastChild?.type === 'text' && typeof lastChild.value === 'string') {
      lastChild.value += ` ^${blockId}`
    }
  }

  // taskItem.children are phrasing content – handled above inside taskList
  // toggleBlock.children handled above
  return copy
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function denormalizeChildren(children: any[]): any[] {
  const result: any[] = []
  for (const child of children) {
    const converted = denormalizeNode(child)
    if (converted?.__expanded) {
      result.push(...converted.nodes)
    } else {
      result.push(converted)
    }
  }
  return result
}

/**
 * Return a new Root that contains only standard mdast nodes (no custom plugin
 * nodes).  The original AST is not mutated.
 */
function toStandardAST(ast: Root): Root {
  return {
    type: 'root',
    children: denormalizeChildren(ast.children)
  } as Root
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

/**
 * Serialise an mdast Root back to a Markdown string.
 *
 * Uses `remark-stringify` with the same GFM / frontmatter settings so that a
 * round-trip parse → serialise → re-parse produces a structurally equivalent AST.
 *
 * Requirements: 2.9, 2.10
 */
export async function serializeAST(ast: Root): Promise<string> {
  const processor = unified()
    .use(remarkStringify, {
      bullet: '-',
      emphasis: '*',
      strong: '*',
      fence: '`',
      fences: true,
      listItemIndent: 'one'
    })
    .use(remarkFrontmatter)
    .use(remarkGfm)
    .use(remarkMath) // stringify math/inlineMath nodes → $...$ / $$...$$

  // Convert custom plugin nodes back to standard mdast before stringifying
  const standardAst = toStandardAST(ast)

  // unified stringify is synchronous but we expose async for forward-compat
  const vfile: VFile = await processor.run(standardAst).then((transformedAst) => {
    return processor.stringify(transformedAst as Root) as unknown as VFile
  })

  // processor.stringify returns a string (VFile-compatible)
  return String(vfile)
}
