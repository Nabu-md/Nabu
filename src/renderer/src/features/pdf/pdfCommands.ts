/**
 * pdfCommands.ts
 *
 * Renderer-side command module for PDF viewer workflow orchestration.
 *
 * Phase 5.4 — Thin UI Enforcement (Architecture Goal 9).
 *
 * This module is the single owner of PDF-related workflow orchestration on the
 * renderer. The `PdfViewer` component must NOT build note content/frontmatter
 * or sequence the create-note-from-annotation flow — it invokes the function
 * here and updates its local annotation state with the returned linked path.
 *
 * The function performs exactly the same IPC call and returns the same linked
 * note path the component previously computed, so runtime behavior is
 * unchanged. The only difference is placement: orchestration (including the
 * domain content generation for the linked note) lives here, not inside the
 * presentation component.
 *
 * Responsibilities (business logic, not presentation):
 *   - build the linked-note markdown body + YAML frontmatter from an annotation
 *   - create the note via IPC
 *   - return the linked note path (or null on failure)
 */

import { PDFAnnotation } from '@shared/types'
import { ipc } from "../../shared/ipc"'

/**
 * Create a backlinked note from a PDF annotation.
 *
 * @param filePath  The source PDF's absolute path.
 * @param annotation  The annotation to capture as a note.
 * @returns The created note's path, or null on failure.
 */
export async function createNoteFromAnnotation(
  filePath: string,
  annotation: PDFAnnotation
): Promise<string | null> {
  const pdfName = filePath.split('/').pop()?.replace('.pdf', '') ?? 'pdf'
  const title = annotation.text.substring(0, 60) || 'PDF Annotation'
  const isoDate = new Date(annotation.timestamp).toISOString()

  const body = [
    `> ${annotation.text}`,
    annotation.comment ? `\n${annotation.comment}` : '',
    '',
    `Source: [[${pdfName}.pdf#page=${annotation.page}]]`
  ].join('\n')

  const frontmatter = `---\nsource: [[${pdfName}.pdf]]\npage: ${annotation.page}\nannotation_date: ${isoDate}\n---\n\n`

  try {
    const result = await ipc.note.create('', title, frontmatter + body)
    if (result && result.path) {
      return result.path
    }
    return null
  } catch (err) {
    console.error('[pdfCommands] Failed to create note from annotation:', err)
    return null
  }
}
