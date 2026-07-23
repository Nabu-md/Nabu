/**
 * schemas/index.ts
 *
 * Canonical Zod validation schemas for shared domain models and IPC payloads.
 *
 * Design rules (Phase 1.4):
 *  - Schemas validate runtime inputs only.
 *  - They are independent of Electron and React.
 *  - They are reusable by both main and renderer.
 *  - They contain NO application behavior.
 *
 * The existing `../schemas` module already defines the bulk of the channel
 * schemas. To avoid duplicate definitions, this module re-exports them and
 * augments the set with the few channel payloads that were previously
 * validated ad-hoc (bookmarks, widget channels, vault:get-current).
 */

import { z } from 'zod'

// Re-export every existing channel schema so this folder is the single
// canonical entry point for schema consumers.
export * from '../schemas'

// ---------------------------------------------------------------------------
// Augmentation: schemas for channels previously validated ad-hoc
// ---------------------------------------------------------------------------

/**
 * vault:get-current (Renderer → Main)
 * Renderer pulls current vault state on mount. No payload is required.
 */
export const VaultGetCurrentSchema = z.object({})

export type VaultGetCurrentPayload = z.infer<typeof VaultGetCurrentSchema>

// bookmarks:get (Renderer → Main)
export const BookmarksGetSchema = z.object({
  vaultPath: z.string()
})

export const BookmarksGetResultSchema = z.object({
  bookmarks: z.record(z.string(), z.array(z.string()))
})

// bookmarks:add (Renderer → Main)
export const BookmarksAddSchema = z.object({
  vaultPath: z.string(),
  listName: z.string(),
  filePath: z.string()
})

export const BookmarksAddResultSchema = BookmarksGetResultSchema

// bookmarks:remove (Renderer → Main)
export const BookmarksRemoveSchema = z.object({
  vaultPath: z.string(),
  listName: z.string(),
  filePath: z.string()
})

export const BookmarksRemoveResultSchema = BookmarksGetResultSchema

export type BookmarksGetPayload = z.infer<typeof BookmarksGetSchema>
export type BookmarksGetResult = z.infer<typeof BookmarksGetResultSchema>
export type BookmarksAddPayload = z.infer<typeof BookmarksAddSchema>
export type BookmarksAddResult = z.infer<typeof BookmarksAddResultSchema>
export type BookmarksRemovePayload = z.infer<typeof BookmarksRemoveSchema>
export type BookmarksRemoveResult = z.infer<typeof BookmarksRemoveResultSchema>

// ---------------------------------------------------------------------------
// Widget channels (clipboard-widget window)
// ---------------------------------------------------------------------------

export const WidgetToggleSchemaCanonical = z.object({
  start: z.boolean().optional()
})

export const WidgetMoveSchema = z.object({
  dx: z.number(),
  dy: z.number()
})

export const WidgetResizeSchema = z.object({
  width: z.number(),
  height: z.number()
})

export const WidgetCreateNoteSchema = z.object({
  name: z.string(),
  content: z.string(),
  timestamp: z.boolean().optional()
})

export const WidgetFetchTitleSchema = z.object({
  url: z.string()
})

export const WidgetSetShortcutSchema = z.object({
  shortcut: z.string()
})

export type WidgetTogglePayload = z.infer<typeof WidgetToggleSchemaCanonical>
export type WidgetMovePayload = z.infer<typeof WidgetMoveSchema>
export type WidgetResizePayload = z.infer<typeof WidgetResizeSchema>
export type WidgetCreateNotePayload = z.infer<typeof WidgetCreateNoteSchema>
export type WidgetFetchTitlePayload = z.infer<typeof WidgetFetchTitleSchema>
export type WidgetSetShortcutPayload = z.infer<typeof WidgetSetShortcutSchema>
