/**
 * pipeline.ts
 *
 * Renderer-side unified/remark pipeline for Live Preview and other inline
 * markdown rendering in the renderer process.
 *
 * Uses the shared buildProcessor from src/shared/markdown.ts to ensure
 * parity with the main-process parser.
 *
 * Requirements: 23.3, 23.7
 */

import type { Root } from 'mdast'

// Import the shared buildProcessor for the markdown pipeline
import {
  buildProcessor,
  remarkStringify,
  remarkFrontmatter,
  remarkGfm,
  remarkMath
} from '@shared/markdown'

import { unified } from 'unified'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RenderResult {
  /** The rendered HTML string */
  html: string
  /** The parsed AST (for inspection/debugging) */
  ast: Root
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/**
 * Parse markdown content into an mdast AST using the shared pipeline.
 *
 * This is the renderer-side parser used by Live Preview to re-parse
 * document content on each change.
 *
 * Requirements: 23.3, 23.7
 */
export function parseMarkdown(content: string): Root {
  const processor = buildProcessor()
  const ast = processor.parse(content) as Root
  return processor.runSync(ast) as Root
}

// ---------------------------------------------------------------------------
// Serializer
// ---------------------------------------------------------------------------

/**
 * Serialize an mdast AST back to markdown string.
 *
 * Uses the shared stringifier configuration for consistency.
 */
export function serializeMarkdown(ast: Root): string {
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
    .use(remarkMath)

  return String(processor.stringify(ast))
}
