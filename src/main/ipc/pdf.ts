/**
 * pdf.ts — PDF feature IPC module.
 *
 * Owns the pdf:* channels, delegating to PdfService.
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. Handler behavior is unchanged.
 */

import { ipcMain } from 'electron'

import { IPCChannel } from '@shared/channels'
import {
  PDFOpenResultSchema,
  PDFRenderPageResultSchema,
  PDFLoadAnnotationsResultSchema,
  PDFSaveAnnotationsResultSchema
} from '@shared/schemas'

import type { IPCContext } from './context'
import { emitActivityLog, normalizeError, errorToString } from './shared'

/**
 * Register all PDF-feature IPC handlers.
 *
 * Each handler delegates to PdfService, which already returns the contract's
 * structured error shape on expected failures. The try/catch here guarantees
 * that any *unexpected* exception thrown by the service is also normalized
 * into the channel's contract error shape instead of rejecting the IPC
 * invocation with a raw Error (Phase 2.4 — Error Normalization).
 */
export function registerPdfIPC(ctx: IPCContext): void {
  const { pdfService } = ctx

  // -------------------------------------------------------------------------
  // pdf:open — open a PDF and return metadata + page count
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_OPEN, async (_event, rawPayload) => {
    try {
      return await pdfService.open(rawPayload)
    } catch (err) {
      const msg = `[IPC] pdf:open unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFOpenResultSchema.parse({
        totalPages: 0,
        metadata: {},
        error: errorToString(normalizeError(err))
      })
    }
  })

  // -------------------------------------------------------------------------
  // pdf:render-page — render a single PDF page to a base64 PNG
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_RENDER_PAGE, async (_event, rawPayload) => {
    try {
      return await pdfService.renderPage(rawPayload)
    } catch (err) {
      const msg = `[IPC] pdf:render-page unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFRenderPageResultSchema.parse({
        pageNumber: 0,
        dataUri: '',
        width: 0,
        height: 0,
        error: errorToString(normalizeError(err))
      })
    }
  })

  // -------------------------------------------------------------------------
  // pdf:load-annotations — load annotations for a PDF
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_LOAD_ANNOTATIONS, async (_event, rawPayload) => {
    try {
      return await pdfService.loadAnnotations(rawPayload)
    } catch (err) {
      const msg = `[IPC] pdf:load-annotations unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFLoadAnnotationsResultSchema.parse({
        annotations: [],
        error: errorToString(normalizeError(err))
      })
    }
  })

  // -------------------------------------------------------------------------
  // pdf:save-annotations — save annotations for a PDF
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_SAVE_ANNOTATIONS, async (_event, rawPayload) => {
    try {
      return await pdfService.saveAnnotations(rawPayload)
    } catch (err) {
      const msg = `[IPC] pdf:save-annotations unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PDFSaveAnnotationsResultSchema.parse({
        success: false,
        error: errorToString(normalizeError(err))
      })
    }
  })
}
