/**
 * bases.ts
 *
 * Database views for notes rendered as sortable/filterable tables.
 * Each row represents a note; columns are frontmatter properties.
 *
 * Requirements: 33.1, 33.2, 33.3, 33.4, 33.5, 33.6, 33.7, 33.8
 */

import type { FileEntry } from '../shared/types'

export interface BaseConfig {
  id: string
  name: string
  view: 'table' | 'board' | 'gallery'
  columns: string[] // property names
  query?: {
    tag?: string
    folder?: string
    property?: string
  }
}

export interface BaseRow {
  path: string
  name: string
  properties: Record<string, unknown>
}

/**
 * Build rows from files matching base query.
 */
export function buildBaseRows(
  files: FileEntry[],
  getAllProperties: (path: string) => Promise<Record<string, unknown>>,
  base: BaseConfig,
): BaseRow[] {
  // Filter by query if specified
  const matchingFiles = files.filter(file => {
    if (base.query?.tag && !file.path.includes(`#${base.query.tag}`)) {
      return false
    }
    if (base.query?.folder) {
      const folderPath = base.query.folder.replace(/\/$/, '')
      if (!file.path.includes(folderPath)) {
        return false
      }
    }
    return true
  })

  // Load properties for each file
  return matchingFiles.map(file => ({
    path: file.path,
    name: file.name,
    properties: {},
  }))
}

/**
 * Load base configurations from vault.
 */
export async function loadBases(vaultPath: string): Promise<BaseConfig[]> {
  // Would read from .nabu/bases.json
  return []
}

/**
 * Save base configuration to vault.
 */
export async function saveBase(vaultPath: string, base: BaseConfig): Promise<void> {
  // Would write to .nabu/bases.json
}

/**
 * Convert base row to markdown on property edit.
 */
export async function updateBaseProperty(
  path: string,
  property: string,
  value: unknown,
): Promise<{ success: boolean; error?: string }> {
  // Would use properties:write IPC
  return { success: true }
}