import type { VaultEntry } from '../types'
import { resolveEntry } from './wikilink'

/**
 * Convention: a note whose frontmatter type is `Folder` acts as a folder
 * note. In the tree it renders with its own row showing the linked children;
 * opening it shows the note content (markdown with wikilinks).
 */
export const FOLDER_NOTE_TYPE = 'folder'

export interface FolderNoteDescription {
  entry: VaultEntry
  /** Linked child entries in wikilink order, deduplicated, folder note excluded. */
  children: VaultEntry[]
}

/** True when the entry uses the `Folder` note type (case-insensitive). */
export function isFolderNote(entry: VaultEntry): boolean {
  return entry.isA?.trim().toLowerCase() === FOLDER_NOTE_TYPE
}


/**
 * Describe a folder note: resolve its outgoing wikilinks to entries and
 * return them in link order (deduplicated, excluding the folder note itself).
 * Returns null when the entry is not a folder note.
 */
export function describeFolderNote({ entry, entries }: { entry: VaultEntry; entries: VaultEntry[] }): FolderNoteDescription | null {
  if (!isFolderNote(entry)) return null

  const seen = new Set<string>([entry.path.toLowerCase()])
  const children: VaultEntry[] = []
  for (const link of entry.outgoingLinks) {
    const resolved = resolveEntry(entries, link, entry)
    if (!resolved || seen.has(resolved.path.toLowerCase())) continue
    seen.add(resolved.path.toLowerCase())
    children.push(resolved)
  }

  return { entry, children }
}
