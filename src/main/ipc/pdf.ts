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

import type { IPCContext } from './context'

/**
 * Register all PDF-feature IPC handlers.
 */
export function registerPdfIPC(ctx: IPCContext): void {
  const { pdfService } = ctx

  // -------------------------------------------------------------------------
  // pdf:open — open a PDF and return metadata + page count
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_OPEN, async (_event, rawPayload) => {
    return pdfService.open(rawPayload)
  })

  // -------------------------------------------------------------------------
  // pdf:render-page — render a single PDF page to a base64 PNG
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_RENDER_PAGE, async (_event, rawPayload) => {
    return pdfService.renderPage(rawPayload)
  })

  // -------------------------------------------------------------------------
  // pdf:load-annotations — load annotations for a PDF
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_LOAD_ANNOTATIONS, async (_event, rawPayload) => {
    return pdfService.loadAnnotations(rawPayload)
  })

  // -------------------------------------------------------------------------
  // pdf:save-annotations — save annotations for a PDF
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_SAVE_ANNOTATIONS, async (_event, rawPayload) => {
    return pdfService.saveAnnotations(rawPayload)
  })
}
