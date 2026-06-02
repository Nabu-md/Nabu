import { z } from 'zod';

// vault:open (Renderer → Main)
export const VaultOpenSchema = z.object({
  path: z.string().optional() // if omitted, show native picker
});

export const VaultScanResultSchema = z.object({
  path: z.string(),
  files: z.array(
    z.object({
      path: z.string(),
      name: z.string(),
      mtime: z.number()
    })
  )
});

// vault:close (Renderer → Main)
export const VaultCloseSchema = z.object({});

// file:get (Renderer → Main)
export const FileGetSchema = z.object({
  path: z.string()
});

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
});

// note:loaded (Main → Renderer)
export const NoteLoadedSchema = z.object({
  path: z.string(),
  ast: z.any()
});

// note:updated (Main → Renderer)
export const NoteUpdatedSchema = z.object({
  path: z.string(),
  ast: z.any(),
  isExternal: z.boolean()
});

// note:deleted (Main → Renderer)
export const NoteDeletedSchema = z.object({
  path: z.string()
});

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
});

// task:toggle (Renderer → Main)
export const TaskToggleSchema = z.object({
  path: z.string(),
  lineIndex: z.number().int().nonnegative()
});

export const TaskToggleResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
});

// context:query (Renderer → Main)
export const ContextQuerySchema = z.object({
  text: z.string(),
  excludePath: z.string().optional()
});

export const ContextSearchResultSchema = z.object({
  results: z.array(
    z.object({
      path: z.string(),
      score: z.number().min(0).max(1),
      tokenCount: z.number().int().nonnegative()
    })
  )
});

// activity:log (bidirectional)
export const ActivityLogSchema = z.object({
  level: z.enum(['info', 'warn', 'error']),
  message: z.string(),
  timestamp: z.number()
});

// vault:create (Renderer → Main)
export const VaultCreateSchema = z.object({
  parentPath: z.string(),
  name: z.string()
});

export const VaultCreateResultSchema = VaultScanResultSchema;

// folder:create (Renderer → Main)
export const FolderCreateSchema = z.object({
  path: z.string()
});

export const FolderCreateResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
});

// note:create (Renderer → Main)
export const NoteCreateSchema = z.object({
  vaultPath: z.string(),
  name: z.string(),
  templateContent: z.string().optional()
});

export const NoteCreateResultSchema = FileGetResultSchema;

// note:save (Renderer → Main)
export const NoteSaveSchema = z.object({
  path: z.string(),
  content: z.string()
});

export const NoteSaveResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
});

// note:rename (Renderer → Main)
export const NoteRenameSchema = z.object({
  oldPath: z.string(),
  newPath: z.string()
});

export const NoteRenameResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
});

// note:delete (Renderer → Main)
export const NoteDeleteSchema = z.object({
  path: z.string()
});

export const NoteDeleteResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
});

// note:get-raw (Renderer → Main)
export const NoteGetRawSchema = z.object({
  path: z.string()
});

export const NoteGetRawResultSchema = z.object({
  path: z.string(),
  content: z.string().optional(),
  error: z.string().optional()
});

// templates:list (Renderer → Main)
export const TemplatesListSchema = z.object({
  vaultPath: z.string()
});

export const TemplatesListResultSchema = z.object({
  templates: z.array(
    z.object({
      name: z.string(),
      path: z.string(),
      content: z.string()
    })
  )
});

// note:export-html (Renderer → Main)
export const NoteExportHtmlSchema = z.object({
  path: z.string(),
  html: z.string()
});

export const NoteExportHtmlResultSchema = z.object({
  success: z.boolean(),
  savedPath: z.string().optional(),
  error: z.string().optional()
});

// settings:get (Renderer → Main)
export const SettingsGetSchema = z.object({
  key: z.string()
});

export const SettingsGetResultSchema = z.object({
  value: z.unknown().optional()
});

// settings:set (Renderer → Main)
export const SettingsSetSchema = z.object({
  key: z.string(),
  value: z.unknown()
});

export const SettingsSetResultSchema = z.object({
  success: z.boolean(),
  error: z.string().optional()
});

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
  )
});

// TypeScript type inference
export type VaultOpenPayload = z.infer<typeof VaultOpenSchema>;
export type VaultScanResult = z.infer<typeof VaultScanResultSchema>;
export type VaultClosePayload = z.infer<typeof VaultCloseSchema>;
export type FileGetPayload = z.infer<typeof FileGetSchema>;
export type FileGetResult = z.infer<typeof FileGetResultSchema>;
export type NoteLoaded = z.infer<typeof NoteLoadedSchema>;
export type NoteUpdated = z.infer<typeof NoteUpdatedSchema>;
export type NoteDeleted = z.infer<typeof NoteDeletedSchema>;
export type NotesLoaded = z.infer<typeof NotesLoadedSchema>;
export type TaskTogglePayload = z.infer<typeof TaskToggleSchema>;
export type TaskToggleResult = z.infer<typeof TaskToggleResultSchema>;
export type ContextQueryPayload = z.infer<typeof ContextQuerySchema>;
export type ContextSearchResult = z.infer<typeof ContextSearchResultSchema>;
export type ActivityLog = z.infer<typeof ActivityLogSchema>;
// v1 types
export type VaultCreatePayload = z.infer<typeof VaultCreateSchema>;
export type VaultCreateResult = z.infer<typeof VaultCreateResultSchema>;
export type FolderCreatePayload = z.infer<typeof FolderCreateSchema>;
export type FolderCreateResult = z.infer<typeof FolderCreateResultSchema>;
export type NoteCreatePayload = z.infer<typeof NoteCreateSchema>;
export type NoteCreateResult = z.infer<typeof NoteCreateResultSchema>;
export type NoteSavePayload = z.infer<typeof NoteSaveSchema>;
export type NoteSaveResult = z.infer<typeof NoteSaveResultSchema>;
export type NoteRenamePayload = z.infer<typeof NoteRenameSchema>;
export type NoteRenameResult = z.infer<typeof NoteRenameResultSchema>;
export type NoteDeletePayload = z.infer<typeof NoteDeleteSchema>;
export type NoteDeleteResult = z.infer<typeof NoteDeleteResultSchema>;
export type NoteGetRawPayload = z.infer<typeof NoteGetRawSchema>;
export type NoteGetRawResult = z.infer<typeof NoteGetRawResultSchema>;
export type TemplatesListPayload = z.infer<typeof TemplatesListSchema>;
export type TemplatesListResult = z.infer<typeof TemplatesListResultSchema>;
export type NoteExportHtmlPayload = z.infer<typeof NoteExportHtmlSchema>;
export type NoteExportHtmlResult = z.infer<typeof NoteExportHtmlResultSchema>;
export type SettingsGetPayload = z.infer<typeof SettingsGetSchema>;
export type SettingsGetResult = z.infer<typeof SettingsGetResultSchema>;
export type SettingsSetPayload = z.infer<typeof SettingsSetSchema>;
export type SettingsSetResult = z.infer<typeof SettingsSetResultSchema>;
export type IndexBuild = z.infer<typeof IndexBuildSchema>;
