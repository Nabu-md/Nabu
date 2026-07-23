/**
 * models/index.ts
 *
 * Shared domain models — the canonical, reusable application types.
 *
 * This module is the single source of truth for cross-cutting value types
 * used by both the main process and the renderer. It intentionally contains
 * NO runtime behavior, NO Electron imports, and NO React imports.
 *
 * Phase 1.4 — Shared Contracts & Typed IPC Framework.
 */

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/** A stable vault identifier (UUID or path-derived key). */
export type VaultId = string

/** A filesystem path (absolute on the host OS). */
export type FilePath = string

// ---------------------------------------------------------------------------
// Core vault / file models
// ---------------------------------------------------------------------------

/** Metadata describing a single file in a vault. */
export interface FileEntry {
  path: string
  name: string
  mtime: number
}

/** Metadata describing an opened vault. */
export interface VaultMetadata {
  path: string
  files: FileEntry[]
}

/** A recently opened vault entry. */
export interface RecentVault {
  path: string
  name: string
  lastOpened: number
}

// ---------------------------------------------------------------------------
// AST / parse models
// ---------------------------------------------------------------------------

/** Structured parse error for a markdown document. */
export interface ParseError {
  line: number
  column: number
  message: string
}

/**
 * Result of parsing a file into an mdast AST.
 * `ast` is intentionally typed loosely here; the concrete mdast `Root` type
 * is re-exported from `types.ts` for consumers that need it.
 */
export interface FileAST {
  path: string
  ast: unknown
  error?: ParseError
}

// ---------------------------------------------------------------------------
// Search / context models
// ---------------------------------------------------------------------------

/** A single semantic (vector) search result. */
export interface SearchResult {
  path: string
  score: number
  tokenCount: number
}

/** A single full-text search match within a file. */
export interface SearchMatch {
  line: number
  snippet: string
  startCol: number
  endCol: number
}

/** A single file's full-text search result. */
export interface SearchResultItem {
  filePath: string
  name: string
  relativePath: string
  score: number
  matches: SearchMatch[]
}

// ---------------------------------------------------------------------------
// Activity / logging models
// ---------------------------------------------------------------------------

/** A structured activity log entry. */
export interface ActivityEntry {
  filePath: string
  timestamp: number
  isExternal: boolean
}

/** Severity level for activity log messages. */
export type ActivityLevel = 'info' | 'warn' | 'error'

/** A structured activity log payload (bidirectional). */
export interface ActivityLogPayload {
  level: ActivityLevel
  message: string
  timestamp: number
}

// ---------------------------------------------------------------------------
// Graph models
// ---------------------------------------------------------------------------

/** A directed edge in the note graph. */
export interface Edge {
  source: string
  target: string
  snippet: string
}

/** A node in the note graph. */
export interface GraphNode {
  id: string
  label: string
  x: number
  y: number
  vx?: number
  vy?: number
}

// ---------------------------------------------------------------------------
// Template models
// ---------------------------------------------------------------------------

/** A vault template definition. */
export interface Template {
  name: string
  path: string
  content: string
}

// ---------------------------------------------------------------------------
// Feature toggle models
// ---------------------------------------------------------------------------

/** A feature toggle exposed to the Settings UI. */
export interface FeatureToggle {
  id: string
  label: string
  description: string
  enabled: boolean
}

// ---------------------------------------------------------------------------
// Clipboard history models
// ---------------------------------------------------------------------------

/** A single clipboard history entry. */
export interface ClipboardEntry {
  id: string
  text: string
  timestamp: number
}

// ---------------------------------------------------------------------------
// PDF models
// ---------------------------------------------------------------------------

/** A single PDF annotation (persisted per-PDF). */
export interface PDFAnnotation {
  id: string
  page: number
  rect: { x: number; y: number; w: number; h: number }
  text: string
  color: 'yellow' | 'green' | 'blue' | 'pink' | 'orange'
  comment?: string
  timestamp: number
  linkedNotePath?: string
}

/** A PDF document's metadata. */
export interface PDFMetadata {
  title?: string
  author?: string
  subject?: string
  keywords?: string
}

// ---------------------------------------------------------------------------
// Dictation / whisper models
// ---------------------------------------------------------------------------

/** A single whisper transcription segment. */
export interface WhisperSegment {
  start: number
  end: number
  text: string
}

/** A whisper transcription result. */
export interface WhisperResult {
  text: string
  segments: WhisperSegment[]
  error?: string
}

/** Status of a single dictation model. */
export interface DictationModelStatus {
  model: 'base' | 'large-v3-turbo-q5'
  installed: boolean
  downloading: boolean
  downloadProgress: number
}

// ---------------------------------------------------------------------------
// Kanban models
// ---------------------------------------------------------------------------

/** A single kanban card. */
export interface KanbanCard {
  filePath: string
  title: string
  content: string
  tags: string[]
  status: string
}

// ---------------------------------------------------------------------------
// Index models
// ---------------------------------------------------------------------------

/**
 * Serialised shape of the extended search index for IPC transport.
 * All Maps/Sets are converted to plain objects/arrays.
 */
export interface ExtendedIndexPayload {
  positions: Record<string, Record<string, number[]>>
  lineSnippets: Record<string, string[]>
  tagIndex: Record<string, string[]>
  aliasIndex: Record<string, string[]>
  propertyIndex: Record<string, Record<string, string[]>>
  blockRefs: Record<string, Record<string, string>>
}

/** Payload pushed on the index:build channel. */
export interface IndexBuildPayload {
  ftIndex: Record<string, string[]>
  tagIndex: Record<string, string[]>
  edges: Edge[]
  extendedIndex: ExtendedIndexPayload
}
