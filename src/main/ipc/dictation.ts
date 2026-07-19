/**
 * dictation.ts — Dictation feature IPC module.
 *
 * Owns the dictation:* channels, delegating to DictationService.
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. Handler behavior is unchanged.
 */

import { ipcMain } from 'electron'

import { IPCChannel } from '@shared/channels'
import {
  DictationStartResultSchema,
  DictationStopResultSchema,
  DictationStatusResultSchema,
  DictationDownloadModelResultSchema
} from '@shared/schemas'

import type { IPCContext } from './context'
import { emitActivityLog, normalizeError, errorToString } from './shared'

/**
 * Register all dictation-feature IPC handlers.
 *
 * Each handler delegates to DictationService, which already returns the
 * contract's structured error shape on expected failures. The try/catch here
 * guarantees that any *unexpected* exception thrown by the service is also
 * normalized into the channel's contract error shape instead of rejecting the
 * IPC invocation with a raw Error (Phase 2.4 — Error Normalization).
 */
export function registerDictationIPC(ctx: IPCContext): void {
  const { dictationService } = ctx

  // -------------------------------------------------------------------------
  // dictation:start — start audio capture and whisper transcription
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_START, async (_event, rawPayload) => {
    try {
      return await dictationService.start(_event, rawPayload)
    } catch (err) {
      const msg = `[IPC] dictation:start unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationStartResultSchema.parse({
        success: false,
        error: errorToString(normalizeError(err))
      })
    }
  })

  // -------------------------------------------------------------------------
  // dictation:stop — stop dictation and return transcription
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_STOP, async (_event, rawPayload) => {
    try {
      return await dictationService.stop(rawPayload)
    } catch (err) {
      const msg = `[IPC] dictation:stop unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationStopResultSchema.parse({
        success: false,
        error: errorToString(normalizeError(err))
      })
    }
  })

  // -------------------------------------------------------------------------
  // dictation:status — get dictation model status
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_STATUS, async (_event, rawPayload) => {
    try {
      return await dictationService.status(rawPayload)
    } catch (err) {
      const msg = `[IPC] dictation:status unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationStatusResultSchema.parse({
        available: false,
        error: errorToString(normalizeError(err))
      })
    }
  })

  // -------------------------------------------------------------------------
  // dictation:download-model — download a dictation model
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_DOWNLOAD_MODEL, async (_event, rawPayload) => {
    try {
      return await dictationService.downloadModel(_event, rawPayload)
    } catch (err) {
      const msg = `[IPC] dictation:download-model unexpected error: ${errorToString(normalizeError(err))}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationDownloadModelResultSchema.parse({
        success: false,
        error: errorToString(normalizeError(err))
      })
    }
  })
}
