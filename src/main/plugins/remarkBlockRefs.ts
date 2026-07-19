/**
 * remarkBlockRefs.ts
 *
 * Remark plugin that supports Obsidian-style block references.
 *
 * Two transformations:
 *   1. Trailing `^identifier` on block-level nodes (paragraph, heading,
 *      listItem, blockquote) is extracted and stored as `data.blockId`.
 *      The `^identifier` is removed from the text content.
 *   2. `[[target#^id]]` wiki-link targets are split so the wikiLink node
 *      receives `target` and `blockRef` fields separately.
 *
 * Must be placed *after* the wikiLink plugin in the pipeline so that
 * `[[target#^id]]` has already been turned into a wikiLink node.
 *
 * Re-exported from shared/plugins for backward compatibility.
 *
 * Requirements: 20.1, 20.2, 20.5, 20.6
 */

export * from '@shared/plugins/remarkBlockRefs'
export { remarkBlockRefs as default } from '@shared/plugins/remarkBlockRefs'
