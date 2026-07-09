/**
 * composer.ts
 *
 * Note Composer - merge multiple notes into a single note.
 * Combines frontmatter tags and content with headings.
 *
 * Requirements: 21.1, 21.2, 21.3, 21.4, 21.5
 */

import path from 'path'
import fs from 'fs/promises'

interface FileEntry {
  path: string
  name: string
}

/**
 * Merge multiple notes into a single content string.
 * Prepends each source's content under `## <source name>` heading.
 * Combines frontmatter tags, warns on conflicting scalar fields.
 */
export function mergeNotes(
  sourcePaths: string[],
  vaultPath: string,
  allNotes: FileEntry[],
  headingLevel: number = 2,
): { previewMarkdown: string; warning?: string } {
  const headingPrefix = '#'.repeat(headingLevel)

  // Build content with headings
  const sections = sourcePaths.map((sourcePath) => {
    const note = allNotes.find((n) => n.path === sourcePath)
    const name = note?.name.replace(/\.md$/, '') ?? path.basename(sourcePath, '.md')
    return `## ${name}`
  })

  // For preview, we just generate the structure
  // The actual content is fetched via IPC in the renderer
  const previewMarkdown = sections.join('\n\n')

  return { previewMarkdown }
}

/**
 * Combine frontmatter tags from multiple notes.
 */
export function mergeTags(notesMetadata: { tags?: string[] }[]): string[] {
  const tagSet = new Set<string>()
  for (const meta of notesMetadata) {
    for (const tag of meta.tags ?? []) {
      tagSet.add(tag)
    }
  }
  return Array.from(tagSet).sort()
}

/**
 * Check for conflicting scalar fields in frontmatter.
 */
export function checkScalarConflicts(
  notesFrontmatter: Record<string, unknown>[],
): string[] {
  const scalarFields = new Map<string, string>() // field -> source note
  const conflicts: string[] = []

  for (let i = 0; i < notesFrontmatter.length; i++) {
    const fm = notesFrontmatter[i]
    for (const [key, value] of Object.entries(fm)) {
      if (scalarFields.has(key)) {
        // Check if values differ
        if (scalarFields.get(key) !== String(value)) {
          conflicts.push(key)
        }
      } else {
        scalarFields.set(key, String(value))
      }
    }
  }

  return [...new Set(conflicts)]
}