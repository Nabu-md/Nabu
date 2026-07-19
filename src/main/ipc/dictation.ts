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

import type { IPCContext } from './context'

/**
 * Register all dictation-feature IPC handlers.
 */
export function registerDictationIPC(ctx: IPCContext): void {
  const { dictationService } = ctx

  // -------------------------------------------------------------------------
  // dictation:start — start audio capture and whisper transcription
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_START, async (_event, rawPayload) => {
    return dictationService.start(_event, rawPayload)
  })

  // -------------------------------------------------------------------------
  // dictation:stop — stop dictation and return transcription
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_STOP, async (_event, rawPayload) => {
    return dictationService.stop(rawPayload)
  })

  // -------------------------------------------------------------------------
  // dictation:status — get dictation model status
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_STATUS, async (_event, rawPayload) => {
    return dictationService.status(rawPayload)
  })

  // -------------------------------------------------------------------------
  // dictation:download-model — download a dictation model
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_DOWNLOAD_MODEL, async (_event, rawPayload) => {
    return dictationService.downloadModel(_event, rawPayload)
  })
}
