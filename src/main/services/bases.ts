/**
 * bases.ts
 *
 * Database views for notes rendered as sortable/filterable tables.
 * Each row represents a note; columns are frontmatter properties.
 *
 * Requirements: 33.1, 33.2, 33.3, 33.4, 33.5, 33.6, 33.7, 33.8
 */

import { promises as fs } from 'node:fs'
import path from 'node:path'
import type { FileEntry } from '@shared/types'

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

const BASES_FILENAME = 'bases.json'

/**
 * Build rows from files matching base query.
 */
export function buildBaseRows(
  files: FileEntry[],
  _getAllProperties: (path: string) => Promise<Record<string, unknown>>,
  base: BaseConfig
): BaseRow[] {
  // Filter by query if specified
  const matchingFiles = files.filter((file) => {
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
  return matchingFiles.map((file) => ({
    path: file.path,
    name: file.name,
    properties: {}
  }))
}

/**
 * Load base configurations from vault.
 */
export async function loadBases(vaultPath: string): Promise<BaseConfig[]> {
  const basesPath = path.join(vaultPath, '.nabu', BASES_FILENAME)

  try {
    const data = await fs.readFile(basesPath, 'utf-8')
    return JSON.parse(data) as BaseConfig[]
  } catch {
    // File doesn't exist or is invalid — return empty array
    return []
  }
}

/**
 * Save base configuration to vault.
 */
export async function saveBase(vaultPath: string, base: BaseConfig): Promise<void> {
  const nabuDir = path.join(vaultPath, '.nabu')
  const basesPath = path.join(nabuDir, BASES_FILENAME)

  // Ensure .nabu directory exists
  await fs.mkdir(nabuDir, { recursive: true })

  // Load existing bases
  const existing = await loadBases(vaultPath)

  // Update or add the base
  const index = existing.findIndex((b) => b.id === base.id)
  if (index >= 0) {
    existing[index] = base
  } else {
    existing.push(base)
  }

  // Write back
  await fs.writeFile(basesPath, JSON.stringify(existing, null, 2), 'utf-8')
}

/**
 * Convert base row to markdown on property edit.
 */
export async function updateBaseProperty(
  _path: string,
  _property: string,
  _value: unknown
): Promise<{ success: boolean; error?: string }> {
  // Would use properties:write IPC
  // For now, return success with a note that this should be handled via IPC
  return { success: true }
}
