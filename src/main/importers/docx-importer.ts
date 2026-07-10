/**
 * docx-importer.ts
 *
 * DOCX to Markdown converter using mammoth.js.
 * Preserves styled text, headings, lists, tables.
 *
 * Requirements: 36.2, 36.4, 36.5, 36.6, 36.7, 36.8, 36.9
 */

import type { Importer } from '../importer-base'

const mammoth = require('mammoth')

export const docxImporter: Importer = {
  format: 'docx',

  validate(data: unknown): boolean {
    return Buffer.isBuffer(data) || data instanceof Uint8Array
  },

  async parse(_data: unknown, sourcePath: string): Promise<string> {
    const filename = sourcePath.split('/').pop() ?? 'unknown.docx'

    const fs = await import('fs/promises')
    const buffer = await fs.readFile(sourcePath)

    let result: { value: string; messages: unknown[] }
    try {
      result = await mammoth.convertToMarkdown({ buffer })
    } catch (err) {
      throw new Error(`Failed to convert DOCX: ${err instanceof Error ? err.message : String(err)}`)
    }

    return `---
source_format: docx
original_file: ${filename}
---

# ${filename.replace('.docx', '')}

${result.value || 'No content found in DOCX.'}
`
  }
}
