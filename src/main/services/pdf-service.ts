/**
 * pdf-service.ts
 *
 * PdfService — owns PDF loading, PDF processing, and PDF coordination.
 *
 * This service extracts the PDF business logic that was previously embedded
 * inside `ipc.ts` (pdf:open, pdf:render-page, pdf:load-annotations,
 * pdf:save-annotations handlers). The IPC layer now delegates to this service,
 * leaving behind thin wrappers.
 *
 * The underlying PDF engine functions live in `pdf-viewer.ts`; this service
 * coordinates them and applies the same validation/error-handling contract the
 * IPC handlers previously performed.
 *
 * This is a pure extraction: no behavior is redesigned, improved, or changed.
 *
 * Requirements: 40.1, 40.2, 40.3, 40.4, 40.5
 */

import {
  PDFOpenSchema,
  PDFOpenResultSchema,
  PDFRenderPageSchema,
  PDFRenderPageResultSchema,
  PDFLoadAnnotationsSchema,
  PDFLoadAnnotationsResultSchema,
  PDFSaveAnnotationsSchema,
  PDFSaveAnnotationsResultSchema
} from '@shared/schemas'
import { emitActivityLog, formatZodError } from '../ipc'
import {
  getPDFInfo,
  renderPDFPage,
  loadPDFAnnotations,
  savePDFAnnotations
} from './pdf-viewer'

// ---------------------------------------------------------------------------
// PdfService
// ---------------------------------------------------------------------------

/**
 * Owns all PDF business logic. Coordinates the PDF engine functions from
 * `pdf-viewer.ts` and applies the validation/error contract.
 */
export class PdfService {
  /**
   * Open a PDF and return its page count and metadata.
   * Mirrors the previous `pdf:open` IPC handler logic exactly.
   */
  async open(rawPayload: unknown): Promise<unknown> {
    const validation = PDFOpenSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[PdfService] pdf:open validation failed: ${reason}`)
      return PDFOpenResultSchema.parse({ totalPages: 0, metadata: {}, error: reason })
    }

    const { path: filePath } = validation.data

    try {
      const info = await getPDFInfo(filePath)
      return PDFOpenResultSchema.parse({
        totalPages: info.totalPages,
        metadata: {
          title: info.metadata.title,
          author: info.metadata.author,
          subject: info.metadata.subject,
          keywords: info.metadata.keywords
        }
      })
    } catch (err) {
      const msg = `[PdfService] pdf:open handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFOpenResultSchema.parse({ totalPages: 0, metadata: {}, error: String(err) })
    }
  }

  /**
   * Render a single PDF page to a base64 PNG.
   * Mirrors the previous `pdf:render-page` IPC handler logic exactly.
   */
  async renderPage(rawPayload: unknown): Promise<unknown> {
    const validation = PDFRenderPageSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[PdfService] pdf:render-page validation failed: ${reason}`)
      return PDFRenderPageResultSchema.parse({
        pageNumber: 0,
        dataUri: '',
        width: 0,
        height: 0,
        error: reason
      })
    }

    const { path: filePath, pageNumber, scale } = validation.data

    try {
      const result = await renderPDFPage(filePath, pageNumber, scale)
      return PDFRenderPageResultSchema.parse({
        pageNumber: result.pageNumber,
        dataUri: result.dataUri,
        width: result.width,
        height: result.height
      })
    } catch (err) {
      const msg = `[PdfService] pdf:render-page handler error for "${filePath}" page ${pageNumber}: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFRenderPageResultSchema.parse({
        pageNumber,
        dataUri: '',
        width: 0,
        height: 0,
        error: String(err)
      })
    }
  }

  /**
   * Load annotations for a PDF.
   * Mirrors the previous `pdf:load-annotations` IPC handler logic exactly.
   */
  async loadAnnotations(rawPayload: unknown): Promise<unknown> {
    const validation = PDFLoadAnnotationsSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[PdfService] pdf:load-annotations validation failed: ${reason}`)
      return PDFLoadAnnotationsResultSchema.parse({ annotations: [], error: reason })
    }

    const { path: filePath } = validation.data

    try {
      const annotations = await loadPDFAnnotations(filePath)
      return PDFLoadAnnotationsResultSchema.parse({ annotations })
    } catch (err) {
      const msg = `[PdfService] pdf:load-annotations handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFLoadAnnotationsResultSchema.parse({ annotations: [], error: String(err) })
    }
  }

  /**
   * Save annotations for a PDF.
   * Mirrors the previous `pdf:save-annotations` IPC handler logic exactly.
   */
  async saveAnnotations(rawPayload: unknown): Promise<unknown> {
    const validation = PDFSaveAnnotationsSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[PdfService] pdf:save-annotations validation failed: ${reason}`)
      return PDFSaveAnnotationsResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, annotations } = validation.data

    try {
      await savePDFAnnotations(filePath, annotations)
      return PDFSaveAnnotationsResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[PdfService] pdf:save-annotations handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFSaveAnnotationsResultSchema.parse({ success: false, error: String(err) })
    }
  }
}
