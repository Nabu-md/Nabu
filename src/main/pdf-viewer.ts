/**
 * pdf-viewer.ts
 *
 * Manages PDF loading, metadata extraction, and page rendering using pdfjs-dist.
 * Provides IPC handlers for opening PDFs and rendering pages to base64 PNG.
 * Rendering uses the `canvas` package for Node.js canvas operations.
 *
 * Requirements: 40.1, 40.2, 40.3
 */

import {
  GlobalWorkerOptions,
  getDocument,
  type PDFDocumentProxy,
  type PDFPageProxy
} from 'pdfjs-dist'
import { createCanvas } from 'canvas'
import fs from 'fs/promises'

// Configure PDF.js worker for Node.js environment (same pattern as pdf-importer)
GlobalWorkerOptions.workerSrc =
  require('pdfjs-dist/build/pdf.worker.js').fileToPath?.(
    'node_modules/pdfjs-dist/build/pdf.worker.min.js'
  ) ?? require.resolve('pdfjs-dist/build/pdf.worker.min.js')

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PDFOpenResult {
  totalPages: number
  metadata: {
    title?: string
    author?: string
    subject?: string
    keywords?: string
  }
}

export interface PDFTextResult {
  text: string
  pageTexts: string[]
}

export interface PDFRenderPageResult {
  pageNumber: number
  dataUri: string
  width: number
  height: number
}

// ---------------------------------------------------------------------------
// PDF Opening
// ---------------------------------------------------------------------------

/**
 * Load a PDF document from a file path and return metadata.
 */
export async function getPDFInfo(filePath: string): Promise<PDFOpenResult> {
  const arrayBuffer = await fs.readFile(filePath)

  let pdf: PDFDocumentProxy
  try {
    pdf = await getDocument({ data: new Uint8Array(arrayBuffer) }).promise
  } catch (err) {
    throw new Error(`Failed to load PDF: ${err instanceof Error ? err.message : String(err)}`)
  }

  const metadata = await pdf.getMetadata().catch(() => null)

  return {
    totalPages: pdf.numPages,
    metadata: {
      title: (metadata as { info?: { Title?: string } })?.info?.Title,
      author: (metadata as { info?: { Author?: string } })?.info?.Author,
      subject: (metadata as { info?: { Subject?: string } })?.info?.Subject,
      keywords: (metadata as { info?: { Keywords?: string } })?.info?.Keywords
    }
  }
}

/**
 * Extract text from a PDF document.
 */
export async function extractPDFText(filePath: string): Promise<PDFTextResult> {
  const arrayBuffer = await fs.readFile(filePath)

  const pdf = await getDocument({ data: new Uint8Array(arrayBuffer) }).promise

  const pageTexts: string[] = []

  for (let i = 1; i <= pdf.numPages; i++) {
    const page = await pdf.getPage(i)
    const textContent = await page.getTextContent()
    const pageText = textContent.items
      .map((item) => (item as { str?: string }).str ?? '')
      .join(' ')
    pageTexts.push(pageText)
  }

  return {
    text: pageTexts.join('\n\n'),
    pageTexts
  }
}

// ---------------------------------------------------------------------------
// PDF Page Rendering
// ---------------------------------------------------------------------------

/**
 * Render a single PDF page to a base64 PNG data URI.
 * Uses the `canvas` package for Node.js canvas operations.
 *
 * @param filePath - Path to the PDF file
 * @param pageNumber - 1-indexed page number
 * @param scale - Zoom scale (default 1.0)
 * @returns Base64 PNG data URI with page dimensions
 */
export async function renderPDFPage(
  filePath: string,
  pageNumber: number,
  scale: number = 1.0
): Promise<PDFRenderPageResult> {
  const arrayBuffer = await fs.readFile(filePath)

  const pdf = await getDocument({ data: new Uint8Array(arrayBuffer) }).promise

  if (pageNumber < 1 || pageNumber > pdf.numPages) {
    throw new Error(`Page ${pageNumber} out of range (1-${pdf.numPages})`)
  }

  const page: PDFPageProxy = await pdf.getPage(pageNumber)
  const viewport = page.getViewport({ scale })

  // Create a canvas using the `canvas` package
  const canvas = createCanvas(viewport.width, viewport.height)
  const context = canvas.getContext('2d')

  if (!context) {
    throw new Error('Failed to get canvas 2D context')
  }

  // Render the PDF page to the canvas
  // `canvas` from the `canvas` package is compatible with pdfjs-dist's
  // expected types at runtime; cast to satisfy the type checker.
  const renderContext = {
    canvasContext: context as unknown as CanvasRenderingContext2D,
    viewport,
    canvas: canvas as unknown as HTMLCanvasElement
  }

  await page.render(renderContext).promise

  // Export to base64 PNG data URI
  const dataUri = canvas.toDataURL('image/png')

  return {
    pageNumber,
    dataUri,
    width: viewport.width,
    height: viewport.height
  }
}

// ---------------------------------------------------------------------------
// Cleanup
// ---------------------------------------------------------------------------

/**
 * Clear PDF cache for a specific file (e.g., when file changes).
 */
export function clearPDFCache(_filePath: string): void {
  // PDF document cleanup is handled by pdfjs internally
}

/**
 * Clear all cached PDFs.
 */
export function clearAllPDFCache(): void {
  // PDF document cleanup is handled by pdfjs internally
}
