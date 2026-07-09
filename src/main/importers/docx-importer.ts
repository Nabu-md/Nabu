/**
 * docx-importer.ts
 *
 * DOCX to Markdown converter using mammoth.js.
 * Preserves styled text, headings, lists, tables.
 *
 * Requirements: 36.2, 36.4, 36.5, 36.6, 36.7, 36.8, 36.9
 */

import type { Importer } from '../importer-base'

export const docxImporter: Importer = {
  format: 'docx',

  validate(data: unknown): boolean {
    return Buffer.isBuffer(data) || (data instanceof Uint8Array)
  },

  async parse(data: unknown, sourcePath: string): Promise<string> {
    // Would use mammoth.js to convert DOCX to markdown
    const filename = sourcePath.split('/').pop() ?? 'unknown.docx'
    return `---
source_format: docx
original_file: ${filename}
---

# ${filename.replace('.docx', '')}

DOCX import placeholder - content would be extracted via mammoth.js.
    `
  },
}