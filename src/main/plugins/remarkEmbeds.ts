/**
 * remarkEmbeds.ts
 *
 * Remark plugin that transforms `![[target]]` embed syntax into `embed`
 * AST nodes. Must be placed *before* the wikiLink plugin so that
 * `![[target]]` is consumed as an embed and not partially matched as
 * text `!` + wikiLink `[[target]]`.
 *
 * Re-exported from shared/plugins for backward compatibility.
 *
 * Requirements: 11.1, 11.7
 */

export * from '../../shared/plugins/remarkEmbeds'
export { remarkEmbeds as default } from '../../shared/plugins/remarkEmbeds'
