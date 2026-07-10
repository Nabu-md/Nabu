/**
 * audio-recorder.ts
 *
 * Record audio from microphone and save as MP3/OGG.
 * Inserts embed link at cursor position.
 *
 * Requirements: 35.1, 35.2, 35.3, 35.4, 35.5, 35.6, 35.7
 */

import fs from 'fs/promises'
import path from 'path'

/**
 * Audio recording session.
 */
export interface AudioSession {
  id: string
  filePath: string
  startTime: Date
  duration: number
}

let activeSession: AudioSession | null = null

/**
 * Check if audio recording is supported.
 */
export function isAudioRecordingSupported(): boolean {
  // In Electron, we can use navigator.mediaDevices in the renderer
  // or use native Node.js modules
  return typeof navigator !== 'undefined' && !!navigator.mediaDevices
}

/**
 * Start recording audio.
 * Returns the file path where the recording will be saved.
 */
export async function startRecording(
  vaultPath: string,
  filename?: string
): Promise<{ success: boolean; path?: string; error?: string }> {
  try {
    const audioDir = path.join(vaultPath, '.nabu', 'audio')
    await fs.mkdir(audioDir, { recursive: true })

    const timestamp = Date.now()
    const finalFilename = filename ?? `recording-${timestamp}.mp3`
    const filePath = path.join(audioDir, finalFilename)

    activeSession = {
      id: timestamp.toString(),
      filePath,
      startTime: new Date(),
      duration: 0
    }

    return { success: true, path: filePath }
  } catch (err) {
    return { success: false, error: String(err) }
  }
}

/**
 * Stop recording and save the audio file.
 */
export async function stopRecording(
  blob: Blob
): Promise<{ success: boolean; path?: string; error?: string }> {
  if (!activeSession) {
    return { success: false, error: 'No active recording session' }
  }

  try {
    const buffer = await blob.arrayBuffer()
    await fs.writeFile(activeSession.filePath, Buffer.from(buffer))
    const path = activeSession.filePath
    activeSession = null
    return { success: true, path }
  } catch (err) {
    return { success: false, error: String(err) }
  }
}

/**
 * Get embed markdown for an audio file.
 */
export function getAudioEmbedMarkdown(path: string): string {
  const relativePath = path.split('/.nabu/audio/')[1] ?? path
  return `![[${relativePath}]]`
}
