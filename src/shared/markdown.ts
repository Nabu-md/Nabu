/**
 * markdown.ts
 *
 * Shared unified/remark processor pipeline for parsing Markdown into mdast ASTs.
 * This is the single source of truth for the plugin pipeline used by both the
 * main process and the renderer process (Live Preview, Outline, Properties validation).
 *
 * Requirements: 23.3, 23.7
 */

import { unified } from 'unified'
import type { Processor } from 'unified'
import type { Root } from 'mdast'

// CJS/ESM interop: electron-vite bundles ESM packages as CJS require() calls
// which return { __esModule: true, default: fn }. Unwrap .default if needed.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function unwrap<T>(mod: any): T {
  return mod && mod.__esModule && mod.default !== undefined ? mod.default : mod
}

// Import remark-parse and related packages - these are renderer-safe
import _remarkParse from 'remark-parse'
import _remarkStringify from 'remark-stringify'
import _remarkFrontmatter from 'remark-frontmatter'
import _remarkGfm from 'remark-gfm'
import _remarkMath from 'remark-math'

const remarkParse = unwrap<typeof _remarkParse>(_remarkParse)
const remarkStringify = unwrap<typeof _remarkStringify>(_remarkStringify)
const remarkFrontmatter = unwrap<typeof _remarkFrontmatter>(_remarkFrontmatter)
const remarkGfm = unwrap<typeof _remarkGfm>(_remarkGfm)
const remarkMath = unwrap<typeof _remarkMath>(_remarkMath)

// Import the custom plugins - they are pure and renderer-safe (no Node.js dependencies)
import { remarkToggleBlocks } from './plugins/remarkToggleBlocks'
import { remarkTaskBlocks } from './plugins/remarkTaskBlocks'
import { remarkWikiLinks } from './plugins/remarkWikiLinks'
import { remarkCallouts } from './plugins/remarkCallouts'
import { remarkEmbeds } from './plugins/remarkEmbeds'
import { remarkBlockRefs } from './plugins/remarkBlockRefs'

// ---------------------------------------------------------------------------
// buildProcessor
// ---------------------------------------------------------------------------

/**
 * Builds a unified processor with the canonical plugin pipeline in the required order:
 *   remarkParse → remarkFrontmatter → remarkGfm → remarkMath → remarkEmbeds →
 *   remarkCallouts → remarkToggleBlocks → remarkTaskBlocks → remarkWikiLinks → remarkBlockRefs
 *
 * This is the single source of truth for the markdown pipeline, used by:
 * - The main process (src/main/parser.ts) for canonical file parsing
 * - The renderer process (src/renderer/src/markdown/pipeline.ts) for Live Preview
 *
 * Requirements: 23.3, 23.7
 */
export function buildProcessor(): Processor<Root, Root, Root, undefined, undefined> {
  return unified()
    .use(remarkParse)
    .use(remarkFrontmatter) // 1. YAML / TOML front matter
    .use(remarkGfm) // 2. GFM tables, strikethrough, task lists syntax
    .use(remarkMath) // 3. $...$ / $$...$$ → inlineMath / math nodes
    .use(remarkEmbeds) // 4. ![[target]] → embed nodes (must precede wikiLink)
    .use(remarkCallouts) // 5. >[!type] blockquotes → Callout nodes
    .use(remarkToggleBlocks) // 6. [toggle] headings → ToggleBlock nodes
    .use(remarkTaskBlocks) // 7. - [ ] / - [x] → TaskList / TaskItem nodes
    .use(remarkWikiLinks) // 8. [[Page Name]] → WikiLink nodes
    .use(remarkBlockRefs) // 9. ^id trailing on blocks + [[note#^id]] refs
}

// ---------------------------------------------------------------------------
// Exports for convenience
// ---------------------------------------------------------------------------

export {
  remarkParse,
  remarkStringify,
  remarkFrontmatter,
  remarkGfm,
  remarkMath,
  remarkToggleBlocks,
  remarkTaskBlocks,
  remarkWikiLinks,
  remarkCallouts,
  remarkEmbeds,
  remarkBlockRefs
}
