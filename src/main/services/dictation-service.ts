/**
 * dictation-service.ts
 *
 * DictationService — owns the speech workflow, transcription coordination, and
 * dictation orchestration.
 *
 * This service extracts the dictation business logic that was previously
 * embedded inside `ipc.ts` (dictation:start, dictation:stop, dictation:status,
 * dictation:download-model handlers). The IPC layer now delegates to this
 * service, leaving behind thin wrappers.
 *
 * The underlying whisper.cpp integration lives in `whisper.ts`; this service
 * coordinates it and applies the same validation/error-handling contract the
 * IPC handlers previously performed.
 *
 * This is a pure extraction: no behavior is redesigned, improved, or changed.
 *
 * Requirements: 41.3, 41.4, 42.4, 42.5
 */

import { IPCChannel } from '@shared/channels'
import {
  DictationStartSchema,
  DictationStartResultSchema,
  DictationStopSchema,
  DictationStopResultSchema,
  DictationStatusSchema,
  DictationStatusResultSchema,
  DictationDownloadModelSchema,
  DictationDownloadModelResultSchema
} from '@shared/schemas'
import { emitActivityLog, formatZodError } from '../ipc'
import { appEventBus } from '@shared/events'

import type { IpcMainInvokeEvent } from 'electron'

// ---------------------------------------------------------------------------
// DictationService
// ---------------------------------------------------------------------------

/**
 * Owns all dictation business logic. Coordinates the whisper.cpp integration
 * from `whisper.ts` and applies the validation/error contract.
 */
export class DictationService {
  /**
   * Start audio capture and whisper transcription.
   * Mirrors the previous `dictation:start` IPC handler logic exactly.
   *
   * @param event - the IPC event, used to send the transcription result back
   *   to the requesting renderer window.
   */
  async start(event: IpcMainInvokeEvent, rawPayload: unknown): Promise<unknown> {
    const validation = DictationStartSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[DictationService] dictation:start validation failed: ${reason}`)
      return DictationStartResultSchema.parse({ success: false, error: reason })
    }

    const { model = 'base' } = validation.data

    try {
      // Check if whisper binary is available
      const { isWhisperBinaryAvailable, isModelInstalled, downloadModel, startDictation } =
        await import('./whisper')

      if (!isWhisperBinaryAvailable()) {
        return DictationStartResultSchema.parse({
          success: false,
          error: 'Whisper binary not found. Please reinstall Nabu.'
        })
      }

      // Check if model is installed, download if needed
      if (!(await isModelInstalled(model))) {
        const downloadResult = await downloadModel(model, () => {})
        if (!downloadResult.success) {
          return DictationStartResultSchema.parse({
            success: false,
            error: `Model download failed: ${downloadResult.error}`
          })
        }
      }

      // Start dictation: spawn mic-capture.swift and whisper, pipe mic → whisper
      startDictation(model)
        .then((result) => {
          // Send transcription result to renderer
          event.sender.send(IPCChannel.DICTATION_RESULT, {
            text: result.text,
            segments: result.segments
          })

          // Notify internal subscribers (services only) that dictation finished.
          appEventBus.publish('DictationFinished', {
            widgetId: 'clipboard-dictation-widget',
            result: { text: result.text, segments: result.segments }
          })
        })
        .catch((err) => {
          console.error('[DictationService] Dictation failed:', err)
          event.sender.send(IPCChannel.DICTATION_RESULT, {
            text: '',
            error: String(err)
          })
        })

      return DictationStartResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[DictationService] dictation:start handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationStartResultSchema.parse({ success: false, error: String(err) })
    }
  }

  /**
   * Stop dictation and return transcription.
   * Mirrors the previous `dictation:stop` IPC handler logic exactly.
   */
  async stop(rawPayload: unknown): Promise<unknown> {
    const validation = DictationStopSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[DictationService] dictation:stop validation failed: ${reason}`)
      return DictationStopResultSchema.parse({ success: false, error: reason })
    }

    try {
      const { stopDictation } = await import('./whisper')
      // Stop dictation: send SIGTERM to mic-capture, which flushes and exits
      // Whisper will then finish transcription and resolve the promise
      stopDictation()
      return DictationStopResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[DictationService] dictation:stop handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationStopResultSchema.parse({ success: false, error: String(err) })
    }
  }

  /**
   * Get dictation model status.
   * Mirrors the previous `dictation:status` IPC handler logic exactly.
   */
  async status(rawPayload: unknown): Promise<unknown> {
    const validation = DictationStatusSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[DictationService] dictation:status validation failed: ${reason}`)
      return DictationStatusResultSchema.parse({ available: false, error: reason })
    }

    try {
      const { isWhisperBinaryAvailable, getModelStatus } = await import('./whisper')
      const available = isWhisperBinaryAvailable()

      if (!available) {
        return DictationStatusResultSchema.parse({ available: false })
      }

      const modelStatus = await getModelStatus()
      return DictationStatusResultSchema.parse({ available: true, modelStatus })
    } catch (err) {
      const msg = `[DictationService] dictation:status handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationStatusResultSchema.parse({ available: false, error: String(err) })
    }
  }

  /**
   * Download a dictation model.
   * Mirrors the previous `dictation:download-model` IPC handler logic exactly.
   *
   * @param event - the IPC event, used to send download progress back to the
   *   requesting renderer window.
   */
  async downloadModel(event: IpcMainInvokeEvent, rawPayload: unknown): Promise<unknown> {
    const validation = DictationDownloadModelSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog(
        'warn',
        `[DictationService] dictation:download-model validation failed: ${reason}`
      )
      return DictationDownloadModelResultSchema.parse({ success: false, error: reason })
    }

    const { model } = validation.data

    try {
      const { downloadModel } = await import('./whisper')

      // Send progress updates to renderer
      const progressCallback = (progress: number) => {
        event.sender.send(IPCChannel.DICTATION_DOWNLOAD_PROGRESS, {
          model,
          progress
        })
      }

      const result = await downloadModel(model, progressCallback)
      return DictationDownloadModelResultSchema.parse(result)
    } catch (err) {
      const msg = `[DictationService] dictation:download-model handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return DictationDownloadModelResultSchema.parse({ success: false, error: String(err) })
    }
  }
}
