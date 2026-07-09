/**
 * pdf-importer.ts
 *
 * PDF to Markdown converter using pdfjs-dist.
 * Extracts text content and preserves structure.
 *
 * Requirements: 36.1, 36.4, 36.5, 36.6, 36.8, 36.9
 */

import type { Importer } from '../importer-base'

export const pdfImporter: Importer = {
  format: 'pdf',

  validate(data: unknown): boolean {
    return typeof data === 'object' && data !== null && 'numPages' in (data as object)
  },

  async parse(data: unknown, sourcePath: string): Promise<string> {
    // Would use pdfjs-dist to extract text
    // For now, return placeholder
    const filename = sourcePath.split('/').pop() ?? 'unknown.pdf'
    return `---
source_format: pdf
original_file: ${filename}
---

# ${filename.replace('.pdf', '')}

PDF import placeholder - content would be extracted via pdfjs-dist.
    `
  },
}