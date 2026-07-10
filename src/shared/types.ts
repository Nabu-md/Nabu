import { Node, Root, Heading, PhrasingContent } from 'mdast'

// Custom AST node types extending mdast Node
export interface ToggleBlock extends Node {
  type: 'toggleBlock'
  heading: Heading // original heading node
  children: Node[] // immediate child content (one level deep)
}

export interface TaskList extends Node {
  type: 'taskList'
  items: TaskItem[]
}

export interface TaskItem extends Node {
  type: 'taskItem'
  checked: boolean
  lineIndex: number // 0-based line index in source file
  children: PhrasingContent[]
}

export interface WikiLink extends Node {
  type: 'wikiLink'
  target: string // e.g., "Page Name"
  resolved: boolean // set by renderer during resolution
  blockRef?: string // for [[note#^id]] form, e.g. "block-id"
}

export interface Callout extends Node {
  type: 'callout'
  /** The callout type (e.g. "note", "warning", "tip"). Lowercased. */
  calloutType: string
  /** Optional title text after the type declaration. */
  title?: string
  /**
   * Collapsible suffix:
   *   '+' → expanded by default, toggleable
   *   '-' → collapsed by default, toggleable
   *   undefined → not collapsible
   */
  toggle?: '+' | '-'
  /** The callout body (paragraphs, lists, etc.). */
  children: Node[]
}

// Core vault and file types
export interface VaultMetadata {
  path: string
  files: FileEntry[]
}

export interface FileEntry {
  path: string
  name: string
  mtime: number
}

export interface FileAST {
  path: string
  ast: Root
  error?: ParseError
}

export interface ParseError {
  line: number
  column: number
  message: string
}

export interface SearchResult {
  path: string
  score: number
  tokenCount: number
}

export interface ActivityEntry {
  filePath: string
  timestamp: number
  isExternal: boolean
}

// Graph types
export interface Edge {
  source: string
  target: string
  /** First paragraph of the source note truncated to 80 chars, precomputed at graph-build time */
  snippet: string
}

export interface GraphNode {
  id: string
  label: string
  x: number
  y: number
  vx?: number
  vy?: number
}

export interface Embed extends Node {
  type: 'embed'
  target: string
}

// Template type
export interface Template {
  name: string
  path: string
  content: string
}
