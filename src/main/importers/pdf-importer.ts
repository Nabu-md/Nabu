/**
 * pdf-importer.ts
 *
 * PDF to Markdown converter using pdfjs-dist.
 * Extracts text content and preserves structure.
 *
 * Requirements: 36.1, 36.4, 36.5, 36.6, 36.8, 36.9
 */

import type { Importer } from '../importer-base'
import { GlobalWorkerOptions, getDocument, type PDFDocumentProxy, type PDFPageProxy } from 'pdfjs-dist'

// Configure PDF.js worker for Node.js environment
GlobalWorkerOptions.workerSrc = require('pdfjs-dist/build/pdf.worker.js').fileToPath?.('node_modules/pdfjs-dist/build/pdf.worker.min.js') ?? 
  require.resolve('pdfjs-dist/build/pdf.worker.min.js')

export const pdfImporter: Importer = {
  format: 'pdf',

  validate(data: unknown): boolean {
    return typeof data === 'object' && data !== null && 'numPages' in (data as object)
  },

  async parse(_data: unknown, sourcePath: string): Promise<string> {
    const filename = sourcePath.split('/').pop() ?? 'unknown.pdf'
    
    // Read the PDF file from disk
    const fs = await import('fs/promises')
    const arrayBuffer = await fs.readFile(sourcePath)
    
    let pdf: PDFDocumentProxy
    try {
      pdf = await getDocument({ data: new Uint8Array(arrayBuffer) }).promise
    } catch (err) {
      throw new Error(`Failed to load PDF: ${err instanceof Error ? err.message : String(err)}`)
    }
    
    const sections: string[] = []
    
    for (let i = 1; i <= pdf.numPages; i++) {
      const page: PDFPageProxy = await pdf.getPage(i)
      const textContent = await page.getTextContent()
      
      // Group text items by their vertical position (approximate lines)
      const lines = new Map<string, string[]>()
      for (const item of textContent.items) {
        const transform = (item as unknown as { transform: number[] }).transform
        const y = transform[5] // Vertical position
        void transform[4] // Horizontal position (unused but kept for reference)
        const text = (item as unknown as { str: string }).str
        
        const lineKey = Math.round(y).toString()
        const existing = lines.get(lineKey) ?? []
        existing.push(text)
        lines.set(lineKey, existing)
      }
      
      // Sort by y position (top to bottom) and join text
      const sortedY = Array.from(lines.keys()).sort((a, b) => parseInt(b) - parseInt(a))
      const pageContent = sortedY.map(y => lines.get(y)?.join(' ').trim()).filter(Boolean).join('\n\n')
      
      if (pageContent) {
        sections.push(pageContent)
      }
    }
    
    const content = sections.join('\n\n')
    
    return `---
source_format: pdf
original_file: ${filename}
---

# ${filename.replace('.pdf', '')}

${content || 'No text content found in PDF.'}
`
  },
}