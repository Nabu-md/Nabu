/**
 * composer.ts
 *
 * Note Composer - merge multiple notes into a single note.
 * Combines frontmatter tags and content with headings.
 *
 * Requirements: 21.1, 21.2, 21.3, 21.4, 21.5
 */

import path from 'node:path'
import type { FileEntry } from '@shared/types'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SourceNote {
  path: string
  name: string
  frontmatter: Record<string, unknown>
  content: string
}

// ---------------------------------------------------------------------------
// Merge Notes
// ---------------------------------------------------------------------------

/**
 * Merge multiple notes into a single content string.
 * Prepends each source's content under `## <source name>` heading.
 * Combines frontmatter tags, warns on conflicting scalar fields.
 */
export async function mergeNotes(
  sourcePaths: string[],
  vaultPath: string,
  allNotes: FileEntry[],
  _headingLevel: number = 2
): Promise<{ previewMarkdown: string; warning?: string }> {
  const sections: string[] = []
  const mergedFrontmatter: Record<string, unknown> = {}
  const allTags = new Set<string>()
  const warnings: string[] = []

  for (const sourcePath of sourcePaths) {
    // Find the note in the vault
    const noteFile = allNotes.find((n) => n.path === sourcePath)
    const name = noteFile?.name.replace(/\.md$/, '') ?? path.basename(sourcePath, '.md')

    // Try to read the actual file content via IPC
    let content = ''
    let frontmatter: Record<string, unknown> = {}

    try {
      // Use the IPC to get file content - in main process this would be direct
      const { readFileSync, existsSync } = await import('fs')
      const fullPath = path.join(vaultPath, sourcePath)

      if (existsSync(fullPath)) {
        const rawContent = readFileSync(fullPath, 'utf-8')

        // Parse frontmatter
        const yamlMatch = rawContent.match(/^---\n([\s\S]*?)\n---\n/)
        if (yamlMatch) {
          try {
            const yaml = await import('yaml')
            frontmatter = yaml.parse(yamlMatch[1]) ?? {}
            content = rawContent.slice(yamlMatch[0].length)
          } catch {
            // Invalid YAML - treat as all content
            content = rawContent
          }
        } else {
          content = rawContent
        }
      }
    } catch (err) {
      warnings.push(`Failed to read ${name}: ${err instanceof Error ? err.message : String(err)}`)
    }

    // Collect tags
    if (typeof frontmatter.tags === 'string') {
      allTags.add(frontmatter.tags)
    } else if (Array.isArray(frontmatter.tags)) {
      for (const tag of frontmatter.tags) {
        if (typeof tag === 'string') allTags.add(tag)
      }
    }

    // Check for scalar field conflicts
    for (const [key, value] of Object.entries(frontmatter)) {
      if (key === 'tags') continue

      if (mergedFrontmatter[key] !== undefined && mergedFrontmatter[key] !== value) {
        if (!warnings.some((w) => w.includes(`Conflicting value for '${key}'`))) {
          warnings.push(`Conflicting value for '${key}' (keeping first occurrence)`)
        }
      } else if (mergedFrontmatter[key] === undefined) {
        mergedFrontmatter[key] = value
      }
    }

    // Add section with heading
    sections.push(`## ${name}\n\n${content}`)
  }

  // Build merged frontmatter
  if (allTags.size > 0) {
    mergedFrontmatter.tags = Array.from(allTags).sort()
  }

  // Build frontmatter YAML
  let frontmatterYaml = ''
  if (Object.keys(mergedFrontmatter).length > 0) {
    const { stringify } = await import('yaml')
    frontmatterYaml = `---\n${stringify(mergedFrontmatter)}\n---\n\n`
  }

  const previewMarkdown = frontmatterYaml + sections.join('\n\n')

  return {
    previewMarkdown,
    warning: warnings.length > 0 ? warnings.join('; ') : undefined
  }
}

// ---------------------------------------------------------------------------
// Merge Tags
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Scalar Conflict Detection
// ---------------------------------------------------------------------------

/**
 * Check for conflicting scalar fields in frontmatter.
 * Returns list of field names that have different values across notes.
 */
export function checkScalarConflicts(notesFrontmatter: Record<string, unknown>[]): string[] {
  const scalarFields = new Map<string, string>() // field -> source note
  const conflicts: string[] = []

  for (let i = 0; i < notesFrontmatter.length; i++) {
    const fm = notesFrontmatter[i]
    for (const [key, value] of Object.entries(fm)) {
      if (key === 'tags') continue // Tags are merged, not checked for conflicts

      const stringValue = String(value)
      if (scalarFields.has(key)) {
        // Check if values differ
        if (scalarFields.get(key) !== stringValue) {
          conflicts.push(key)
        }
      } else {
        scalarFields.set(key, stringValue)
      }
    }
  }

  return [...new Set(conflicts)]
}
