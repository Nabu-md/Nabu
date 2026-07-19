/**
 * remarkCallouts.ts
 *
 * Remark plugin that transforms blockquote nodes starting with `> [!TYPE]`
 * into callout AST nodes. Supports `[!type]+` (expanded toggle) and
 * `[!type]-` (collapsed toggle) suffixes.
 *
 * Re-exported from shared/plugins for backward compatibility.
 *
 * Requirements: 8.1, 8.3, 8.6
 */

export * from '@shared/plugins/remarkCallouts'
export { remarkCallouts as default } from '@shared/plugins/remarkCallouts'
