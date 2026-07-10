import { z } from 'zod'

// vault:open (Renderer → Main)
export const VaultOpenSchema = z.object({
  path: z.string().optional() // if omitted, show native picker
})

export const VaultScanResultSchema = z.object({
  path: z.string(),
  files: z.array(
    z.object({
      path: z.string(),
      name: z.string(),
      mtime: z.number()
    })
  )
})

// vault:close (Renderer → Main)
export const VaultCloseSchema = z.object({
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

// vault:switch (Renderer → Main) — switch active vault
export const VaultSwitchSchema = z.object({
  vaultId: z.string()
})

const VaultSwitchResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

export type VaultSwitchPayload = z.infer<typeof VaultSwitchSchema>
export type VaultSwitchResult = z.infer<typeof VaultSwitchResultSchema>

// vault:get-recents (Renderer → Main) — get recent vaults list
export const VaultGetRecentsSchema = z.object({})

const VaultGetRecentsResultSchema = z.object({
  recents: z.array(
    z.object({
      path: z.string(),
      name: z.string(),
      lastOpened: z.number()
    })
  )
})

export type VaultGetRecentsResult = z.infer<typeof VaultGetRecentsResultSchema>

// file:get (Renderer → Main)
export const FileGetSchema = z.object({
  path: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const FileGetResultSchema = z.object({
  path: z.string(),
  ast: z.any(), // mdast Root (complex recursive structure)
  error: z
    .object({
      line: z.number(),
      column: z.number(),
      message: z.string()
    })
    .optional()
})

// note:loaded (Main → Renderer)
export const NoteLoadedSchema = z.object({
  path: z.string(),
  ast: z.any()
})

// note:updated (Main → Renderer)
export const NoteUpdatedSchema = z.object({
  path: z.string(),
  ast: z.any(),
  isExternal: z.boolean()
})

// note:deleted (Main → Renderer)
export const NoteDeletedSchema = z.object({
  path: z.string()
})

// notes:loaded (Main → Renderer) - bulk file list
export const NotesLoadedSchema = z.object({
  vaultPath: z.string().optional(), // included when vault changes so renderer can update state.vault
  files: z.array(
    z.object({
      path: z.string(),
      name: z.string(),
      mtime: z.number()
    })
  )
})

// task:toggle (Renderer → Main)
export const TaskToggleSchema = z.object({
  path: z.string(),
  lineIndex: z.number().int().nonnegative(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const TaskToggleResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

// context:query (Renderer → Main)
export const ContextQuerySchema = z.object({
  text: z.string(),
  excludePath: z.string().optional()
})

export const ContextSearchResultSchema = z.object({
  results: z.array(
    z.object({
      path: z.string(),
      score: z.number().min(0).max(1),
      tokenCount: z.number().int().nonnegative()
    })
  ),
  /** Set to true when the vector index is disabled or empty (Requirement 1.7). */
  disabled: z.boolean().optional(),
  /** Human-readable reason why results are unavailable (Requirement 1.7). */
  reason: z.string().optional()
})

// context:reindex (Renderer → Main) — trigger full re-embed of all vault files
export const ContextReindexSchema = z.object({
  vaultPath: z.string()
})

export const ContextReindexResultSchema = z.object({
  processed: z.number().int().nonnegative()
})

// vector:status (Renderer → Main) — get vector index status
export const VectorStatusSchema = z.object({})

export const VectorStatusResultSchema = z.object({
  disabled: z.boolean(),
  reason: z.string().nullable(),
  items: z.number().int().nonnegative()
})

// activity:log (bidirectional)
export const ActivityLogSchema = z.object({
  level: z.enum(['info', 'warn', 'error']),
  message: z.string(),
  timestamp: z.number()
})

// vault:create (Renderer → Main)
export const VaultCreateSchema = z.object({
  parentPath: z.string(),
  name: z.string()
})

export const VaultCreateResultSchema = VaultScanResultSchema

// folder:create (Renderer → Main)
export const FolderCreateSchema = z.object({
  path: z.string()
})

export const FolderCreateResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

// note:create (Renderer → Main)
export const NoteCreateSchema = z.object({
  vaultPath: z.string(),
  name: z.string(),
  templateContent: z.string().optional()
})

export const NoteCreateResultSchema = FileGetResultSchema

// note:save (Renderer → Main)
export const NoteSaveSchema = z.object({
  path: z.string(),
  content: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const NoteSaveResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

// note:rename (Renderer → Main)
export const NoteRenameSchema = z.object({
  oldPath: z.string(),
  newPath: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const NoteRenameResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

// note:delete (Renderer → Main)
export const NoteDeleteSchema = z.object({
  path: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const NoteDeleteResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

// note:get-raw (Renderer → Main)
export const NoteGetRawSchema = z.object({
  path: z.string()
})

export const NoteGetRawResultSchema = z.object({
  path: z.string(),
  content: z.string().optional(),
  error: z.string().optional()
})

// templates:list (Renderer → Main)
export const TemplatesListSchema = z.object({
  vaultPath: z.string()
})

export const TemplatesListResultSchema = z.object({
  templates: z.array(
    z.object({
      name: z.string(),
      path: z.string(),
      content: z.string()
    })
  )
})

// note:export-html (Renderer → Main)
export const NoteExportHtmlSchema = z.object({
  path: z.string(),
  html: z.string()
})

export const NoteExportHtmlResultSchema = z.object({
  success: z.boolean(),
  savedPath: z.string().optional(),
  error: z.string().optional()
})

// settings:get (Renderer → Main)
export const SettingsGetSchema = z.object({
  key: z.string()
})

export const SettingsGetResultSchema = z.object({
  value: z.unknown().optional()
})

// settings:set (Renderer → Main)
export const SettingsSetSchema = z.object({
  key: z.string(),
  value: z.unknown()
})

export const SettingsSetResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

/**
 * Serialised shape of ExtendedSearchIndex for IPC transport.
 * All Maps/Sets are converted to plain objects/arrays.
 */
const ExtendedIndexPayloadSchema = z.object({
  positions: z.record(z.string(), z.record(z.string(), z.array(z.number()))),
  lineSnippets: z.record(z.string(), z.array(z.string())),
  tagIndex: z.record(z.string(), z.array(z.string())),
  aliasIndex: z.record(z.string(), z.array(z.string())),
  propertyIndex: z.record(z.string(), z.record(z.string(), z.array(z.string()))),
  blockRefs: z.record(z.string(), z.record(z.string(), z.string()))
})

// index:build (Main → Renderer push channel)
export const IndexBuildSchema = z.object({
  ftIndex: z.record(z.string(), z.array(z.string())),
  tagIndex: z.record(z.string(), z.array(z.string())),
  edges: z.array(
    z.object({
      source: z.string(),
      target: z.string(),
      snippet: z.string()
    })
  ),
  /** Extended search index (token positions, line snippets, aliases, properties, block refs). */
  extendedIndex: ExtendedIndexPayloadSchema
})

// search:query (Renderer → Main)
export const SearchQuerySchema = z.object({
  query: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

const SearchMatchSchema = z.object({
  line: z.number().int().nonnegative(),
  snippet: z.string(),
  startCol: z.number().int().nonnegative(),
  endCol: z.number().int().nonnegative()
})

const SearchResultItemSchema = z.object({
  filePath: z.string(),
  name: z.string(),
  relativePath: z.string(),
  score: z.number().int().nonnegative(),
  matches: z.array(SearchMatchSchema)
})

export const SearchResponseSchema = z.object({
  results: z.array(SearchResultItemSchema)
})

// TypeScript type inference
export type VaultOpenPayload = z.infer<typeof VaultOpenSchema>
export type VaultScanResult = z.infer<typeof VaultScanResultSchema>
export type VaultClosePayload = z.infer<typeof VaultCloseSchema>
export type FileGetPayload = z.infer<typeof FileGetSchema>
export type FileGetResult = z.infer<typeof FileGetResultSchema>
export type NoteLoaded = z.infer<typeof NoteLoadedSchema>
export type NoteUpdated = z.infer<typeof NoteUpdatedSchema>
export type NoteDeleted = z.infer<typeof NoteDeletedSchema>
export type NotesLoaded = z.infer<typeof NotesLoadedSchema>
export type TaskTogglePayload = z.infer<typeof TaskToggleSchema>
export type TaskToggleResult = z.infer<typeof TaskToggleResultSchema>
export type ContextQueryPayload = z.infer<typeof ContextQuerySchema>
export type ContextSearchResult = z.infer<typeof ContextSearchResultSchema>
export type ContextReindexPayload = z.infer<typeof ContextReindexSchema>
export type ContextReindexResult = z.infer<typeof ContextReindexResultSchema>
export type VectorStatusResult = z.infer<typeof VectorStatusResultSchema>
export type ActivityLog = z.infer<typeof ActivityLogSchema>
// v1 types
export type VaultCreatePayload = z.infer<typeof VaultCreateSchema>
export type VaultCreateResult = z.infer<typeof VaultCreateResultSchema>
export type FolderCreatePayload = z.infer<typeof FolderCreateSchema>
export type FolderCreateResult = z.infer<typeof FolderCreateResultSchema>
export type NoteCreatePayload = z.infer<typeof NoteCreateSchema>
export type NoteCreateResult = z.infer<typeof NoteCreateResultSchema>
export type NoteSavePayload = z.infer<typeof NoteSaveSchema>
export type NoteSaveResult = z.infer<typeof NoteSaveResultSchema>
export type NoteRenamePayload = z.infer<typeof NoteRenameSchema>
export type NoteRenameResult = z.infer<typeof NoteRenameResultSchema>
export type NoteDeletePayload = z.infer<typeof NoteDeleteSchema>
export type NoteDeleteResult = z.infer<typeof NoteDeleteResultSchema>
export type NoteGetRawPayload = z.infer<typeof NoteGetRawSchema>
export type NoteGetRawResult = z.infer<typeof NoteGetRawResultSchema>
export type TemplatesListPayload = z.infer<typeof TemplatesListSchema>
export type TemplatesListResult = z.infer<typeof TemplatesListResultSchema>
export type NoteExportHtmlPayload = z.infer<typeof NoteExportHtmlSchema>
export type NoteExportHtmlResult = z.infer<typeof NoteExportHtmlResultSchema>
export type SettingsGetPayload = z.infer<typeof SettingsGetSchema>
export type SettingsGetResult = z.infer<typeof SettingsGetResultSchema>
export type SettingsSetPayload = z.infer<typeof SettingsSetSchema>
export type SettingsSetResult = z.infer<typeof SettingsSetResultSchema>
export type IndexBuild = z.infer<typeof IndexBuildSchema>
export type SearchQueryPayload = z.infer<typeof SearchQuerySchema>
export type SearchResponse = z.infer<typeof SearchResponseSchema>

// asset:read (Sandboxed HTML → Main) — read a local file as base64 for the sandboxed iframe
export const AssetReadSchema = z.object({
  path: z.string()
})

export const AssetReadResultSchema = z.object({
  path: z.string(),
  dataUri: z.string().optional(),
  error: z.string().optional()
})

export type AssetReadPayload = z.infer<typeof AssetReadSchema>
export type AssetReadResult = z.infer<typeof AssetReadResultSchema>

// properties:read (Renderer → Main) — read YAML frontmatter properties
export const PropertiesReadSchema = z.object({
  path: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const PropertiesReadResultSchema = z.object({
  path: z.string(),
  properties: z.record(z.string(), z.unknown()),
  yaml: z.string()
})

// properties:write (Renderer → Main) — write YAML frontmatter properties
export const PropertiesWriteSchema = z.object({
  path: z.string(),
  yaml: z.string(),
  vaultId: z.string().optional() // defaults to active vault when omitted (Req 22.9)
})

export const PropertiesWriteResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

// note:daily (Renderer → Main) — open or create today's daily note
export const NoteDailySchema = z.object({
  vaultPath: z.string()
})

export const NoteDailyResultSchema = z.object({
  path: z.string(),
  ast: z.any(),
  created: z.boolean(),
  error: z.string().optional()
})

export type NoteDailyPayload = z.infer<typeof NoteDailySchema>
export type NoteDailyResult = z.infer<typeof NoteDailyResultSchema>

// favorites:get (Renderer → Main) — get favorites list for a vault
export const FavoritesGetSchema = z.object({
  vaultPath: z.string()
})

export const FavoritesGetResultSchema = z.object({
  favorites: z.array(z.string())
})

// favorites:toggle (Renderer → Main) — toggle a file's favorite state
export const FavoritesToggleSchema = z.object({
  vaultPath: z.string(),
  filePath: z.string()
})

export const FavoritesToggleResultSchema = z.object({
  favorites: z.array(z.string())
})

// favorites:remove (Renderer → Main) — remove a file from favorites
export const FavoritesRemoveSchema = z.object({
  vaultPath: z.string(),
  filePath: z.string()
})

export const FavoritesRemoveResultSchema = z.object({
  favorites: z.array(z.string())
})

export type FavoritesGetPayload = z.infer<typeof FavoritesGetSchema>
export type FavoritesGetResult = z.infer<typeof FavoritesGetResultSchema>
export type FavoritesTogglePayload = z.infer<typeof FavoritesToggleSchema>
export type FavoritesToggleResult = z.infer<typeof FavoritesToggleResultSchema>
export type FavoritesRemovePayload = z.infer<typeof FavoritesRemoveSchema>
export type FavoritesRemoveResult = z.infer<typeof FavoritesRemoveResultSchema>

// note:random (Renderer → Main) — open a random note
export const NoteRandomSchema = z.object({
  vaultPath: z.string(),
  tagFilter: z.string().optional()
})

export const NoteRandomResultSchema = z.object({
  path: z.string().optional(),
  ast: z.any().optional(),
  error: z.string().optional()
})

export type NoteRandomPayload = z.infer<typeof NoteRandomSchema>
export type NoteRandomResult = z.infer<typeof NoteRandomResultSchema>

// note:compose (Renderer → Main) — merge multiple notes into one
export const NoteComposeSchema = z.object({
  vaultPath: z.string(),
  sourcePaths: z.array(z.string()),
  targetName: z.string(),
  headingLevel: z.number().int().min(1).max(6).optional(),
  deleteAfterMerge: z.boolean().optional()
})

export const NoteComposeResultSchema = z.object({
  path: z.string().optional(),
  ast: z.any().optional(),
  error: z.string().optional(),
  previewMarkdown: z.string().optional(),
  conflicts: z.array(z.string()).optional()
})

export type NoteComposePayload = z.infer<typeof NoteComposeSchema>
export type NoteComposeResult = z.infer<typeof NoteComposeResultSchema>

// note:unique (Renderer → Main) — create a note with unique timestamp name
export const NoteUniqueSchema = z.object({
  vaultPath: z.string()
})

export const NoteUniqueResultSchema = z.object({
  path: z.string().optional(),
  ast: z.any().optional(),
  error: z.string().optional()
})

export type NoteUniquePayload = z.infer<typeof NoteUniqueSchema>
export type NoteUniqueResult = z.infer<typeof NoteUniqueResultSchema>

export type PropertiesReadPayload = z.infer<typeof PropertiesReadSchema>
export type PropertiesReadResult = z.infer<typeof PropertiesReadResultSchema>
export type PropertiesWritePayload = z.infer<typeof PropertiesWriteSchema>
export type PropertiesWriteResult = z.infer<typeof PropertiesWriteResultSchema>

// ---------------------------------------------------------------------------
// Feature toggle schemas
// ---------------------------------------------------------------------------

/** Feature toggle entry for IPC transport */
export const FeatureToggleSchema = z.object({
  id: z.string(),
  label: z.string(),
  description: z.string(),
  enabled: z.boolean()
})

export const FeatureTogglesResultSchema = z.object({
  toggles: z.array(FeatureToggleSchema)
})

export const SetFeatureToggleSchema = z.object({
  id: z.string(),
  enabled: z.boolean()
})

export const SetFeatureToggleResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
})

export type FeatureToggle = z.infer<typeof FeatureToggleSchema>
export type FeatureTogglesResult = z.infer<typeof FeatureTogglesResultSchema>
export type SetFeatureTogglePayload = z.infer<typeof SetFeatureToggleSchema>
export type SetFeatureToggleResult = z.infer<typeof SetFeatureToggleResultSchema>
