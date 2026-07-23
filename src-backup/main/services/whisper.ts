/**
 * whisper.ts
 *
 * Whisper.cpp integration for audio dictation.
 * Spawns whisper binary as child process, handles model management.
 *
 * Requirements: 41.1, 41.2, 41.6, 42.4, 42.5, 42.6, 43.2, 43.3
 */

import { spawn, ChildProcess } from 'child_process'
import path from 'path'
import fs from 'fs/promises'

// Model types
export type WhisperModel = 'base' | 'large-v3-turbo-q5'

// Whisper output format
export interface WhisperSegment {
  start: number
  end: number
  text: string
}

export interface WhisperResult {
  text: string
  segments: WhisperSegment[]
  error?: string
}

// Model configuration
const MODEL_CONFIG = {
  base: {
    filename: 'ggml-base.en.bin',
    size: 140 * 1024 * 1024, // ~140 MB
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin',
    sha256: 'e3f3a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5' // Placeholder
  },
  'large-v3-turbo-q5': {
    filename: 'ggml-large-v3-turbo-q5_0.bin',
    size: 550 * 1024 * 1024, // ~550 MB
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin',
    sha256: 'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2' // Placeholder
  }
}

// Whisper process state
let whisperProcess: ChildProcess | null = null
let currentModel: WhisperModel = 'base'

/**
 * Get the path to the whisper binary.
 * Checks multiple candidate locations for dev and production builds.
 */
export function getWhisperBinaryPath(): string {
  const candidates = [
    // Production: bundled in resources
    path.join(process.resourcesPath, 'whisper'),
    // Development: project root
    path.join(process.cwd(), 'whisper'),
    // Development: build directory
    path.join(process.cwd(), 'build', 'whisper')
  ]

  for (const candidate of candidates) {
    try {
      require('fs').accessSync(candidate)
      return candidate
    } catch {
      // Try next candidate
    }
  }

  // Default to resources path (will be created if missing)
  return candidates[0]
}

/**
 * Get the path to the models directory.
 */
export function getModelsPath(): string {
  const isDev = process.env.NODE_ENV === 'development'

  if (isDev) {
    return path.join(process.cwd(), 'resources', 'whisper-models')
  }

  // Production: bundle models in resources directory
  return path.join(process.resourcesPath, 'whisper-models')
}

/**
 * Ensure the model is available, downloading it on first run if needed.
 * Returns true if the model is available (or was successfully downloaded).
 */
export async function ensureModelAvailable(
  model: WhisperModel
): Promise<{ available: boolean; downloaded: boolean; error?: string }> {
  if (await isModelInstalled(model)) {
    return { available: true, downloaded: false }
  }

  // Attempt to download the model
  const result = await downloadModel(model, () => {
    // Progress callback - could be wired to IPC later
  })

  if (result.success) {
    return { available: true, downloaded: true }
  }

  return { available: false, downloaded: false, error: result.error }
}

/**
 * Get the path to a specific model file.
 */
export function getModelPath(model: WhisperModel): string {
  return path.join(getModelsPath(), MODEL_CONFIG[model].filename)
}

/**
 * Check if a model is installed.
 */
export async function isModelInstalled(model: WhisperModel): Promise<boolean> {
  try {
    const modelPath = getModelPath(model)
    await fs.access(modelPath)
    return true
  } catch {
    return false
  }
}

/**
 * Check if whisper binary exists.
 */
export function isWhisperBinaryAvailable(): boolean {
  try {
    // Synchronous check for binary existence
    require('fs').accessSync(getWhisperBinaryPath())
    return true
  } catch {
    return false
  }
}

/**
 * Get the current model status.
 */
export async function getModelStatus(): Promise<{
  model: WhisperModel
  installed: boolean
  downloading: boolean
  downloadProgress: number
}> {
  const installed = await isModelInstalled(currentModel)
  return {
    model: currentModel,
    installed,
    downloading: false,
    downloadProgress: 0
  }
}

/**
 * Set the current model.
 */
export function setModel(model: WhisperModel): void {
  currentModel = model
}

/**
 * Transcribe audio from stdin using whisper.
 * Returns a promise that resolves with the transcription result.
 */
export function transcribeAudio(
  audioStream: NodeJS.ReadableStream,
  model: WhisperModel = 'base'
): Promise<WhisperResult> {
  return new Promise((resolve, reject) => {
    const binaryPath = getWhisperBinaryPath()
    const modelPath = getModelPath(model)

    if (!isWhisperBinaryAvailable()) {
      reject(new Error('Whisper binary not found'))
      return
    }

    // Spawn whisper process
    whisperProcess = spawn(
      binaryPath,
      [
        '-m',
        modelPath,
        '-t',
        '4', // 4 threads
        '--output-format',
        'json',
        '--no-timestamps' // We'll get segments with timestamps
      ],
      {
        stdio: ['pipe', 'pipe', 'pipe']
      }
    )

    let stdout = ''
    let stderr = ''

    whisperProcess.stdout?.on('data', (data) => {
      stdout += data.toString()
    })

    whisperProcess.stderr?.on('data', (data) => {
      stderr += data.toString()
    })

    whisperProcess.on('error', (err) => {
      console.error('Whisper process error:', err)
      reject(err)
    })

    whisperProcess.on('close', (code) => {
      whisperProcess = null

      if (code !== 0) {
        console.error('Whisper process exited with code:', code, stderr)
        reject(new Error(`Whisper failed with code ${code}`))
        return
      }

      try {
        // Parse JSON output
        const result = JSON.parse(stdout) as WhisperResult
        resolve(result)
      } catch (e) {
        console.error('Failed to parse whisper output:', stdout)
        reject(new Error('Invalid whisper output'))
      }
    })

    // Pipe audio to whisper's stdin
    audioStream.pipe(whisperProcess.stdin!)
  })
}

/**
 * Stop the current whisper process.
 */
export function stopWhisper(): void {
  if (whisperProcess) {
    whisperProcess.kill()
    whisperProcess = null
  }
}

/**
 * Compute SHA256 of a file.
 */
async function computeSha256(filePath: string): Promise<string> {
  const crypto = require('crypto')
  const hash = crypto.createHash('sha256')
  const stream = require('fs').createReadStream(filePath)

  return new Promise((resolve, reject) => {
    stream.on('data', (chunk: Buffer) => hash.update(chunk))
    stream.on('end', () => resolve(hash.digest('hex')))
    stream.on('error', (err: Error) => reject(err))
  })
}

/**
 * Download a model file.
 * Returns progress updates via callback.
 * Verifies SHA256 against expected hash if available.
 */
export async function downloadModel(
  model: WhisperModel,
  onProgress: (progress: number) => void
): Promise<{ success: boolean; error?: string }> {
  const modelConfig = MODEL_CONFIG[model]
  const modelPath = getModelPath(model)

  // Ensure models directory exists
  const modelsDir = getModelsPath()
  await fs.mkdir(modelsDir, { recursive: true })

  try {
    // Use node's https module to download
    const https = require('https')
    const { createWriteStream } = require('fs')

    return new Promise((resolve, _reject) => {
      https
        .get(modelConfig.url, (response: any) => {
          if (response.statusCode !== 200) {
            resolve({ success: false, error: `HTTP ${response.statusCode}` })
            return
          }

          const fileStream = createWriteStream(modelPath)
          const totalBytes = parseInt(response.headers['content-length'] || '0', 10)
          let downloadedBytes = 0

          response.on('data', (chunk: Buffer) => {
            downloadedBytes += chunk.length
            if (totalBytes > 0) {
              onProgress(Math.round((downloadedBytes / totalBytes) * 100))
            }
          })

          response.pipe(fileStream)

          fileStream.on('finish', async () => {
            fileStream.close()

            // Verify SHA256 if we have an expected hash
            const expectedHash = modelConfig.sha256
            if (expectedHash && expectedHash.length === 64) {
              try {
                const actualHash = await computeSha256(modelPath)
                if (actualHash !== expectedHash) {
                  // Corrupted download - remove and report
                  await fs.unlink(modelPath).catch(() => {})
                  resolve({ success: false, error: 'SHA256 mismatch - download corrupted' })
                  return
                }
              } catch (hashErr) {
                resolve({ success: false, error: `Hash verification failed: ${String(hashErr)}` })
                return
              }
            }

            onProgress(100)
            resolve({ success: true })
          })

          fileStream.on('error', (err: Error) => {
            resolve({ success: false, error: err.message })
          })
        })
        .on('error', (err: Error) => {
          resolve({ success: false, error: err.message })
        })
    })
  } catch (err) {
    return { success: false, error: String(err) }
  }
}

/**
 * Get the download URL for a model.
 */
export function getModelDownloadUrl(model: WhisperModel): string {
  return MODEL_CONFIG[model].url
}

/**
 * Get the expected file size for a model.
 */
export function getModelSize(model: WhisperModel): number {
  return MODEL_CONFIG[model].size
}

// Dictation state
let micProcess: ChildProcess | null = null
let dictationActive = false
let dictationResolve: ((result: WhisperResult) => void) | null = null
let dictationReject: ((err: Error) => void) | null = null

/**
 * Get the path to the mic-capture.swift helper.
 */
export function getMicCapturePath(): string {
  const candidates = [
    path.join(process.resourcesPath, 'mic-capture.swift'),
    path.join(process.cwd(), 'scripts', 'mic-capture.swift'),
    path.join(process.cwd(), 'mic-capture.swift')
  ]

  for (const candidate of candidates) {
    try {
      require('fs').accessSync(candidate)
      return candidate
    } catch {
      // Try next candidate
    }
  }

  return candidates[0]
}

/**
 * Start dictation: spawn mic-capture.swift and whisper, pipe mic → whisper stdin.
 * Returns a promise that resolves with the transcription when fn is released.
 */
export function startDictation(model: WhisperModel = 'base'): Promise<WhisperResult> {
  return new Promise((resolve, reject) => {
    if (dictationActive) {
      reject(new Error('Dictation already active'))
      return
    }

    const binaryPath = getWhisperBinaryPath()
    const modelPath = getModelPath(model)
    const micPath = getMicCapturePath()

    if (!isWhisperBinaryAvailable()) {
      reject(new Error('Whisper binary not found'))
      return
    }

    dictationActive = true
    dictationResolve = resolve
    dictationReject = reject

    // Spawn whisper process
    whisperProcess = spawn(
      binaryPath,
      ['-m', modelPath, '-t', '4', '--output-format', 'json', '--no-timestamps'],
      { stdio: ['pipe', 'pipe', 'pipe'] }
    )

    // Spawn mic-capture.swift
    // Use `swift` to run the script (available on macOS with Xcode command line tools)
    micProcess = spawn('swift', [micPath], {
      stdio: ['pipe', 'pipe', 'pipe']
    })

    let stdout = ''
    let stderr = ''

    whisperProcess.stdout?.on('data', (data) => {
      stdout += data.toString()
    })

    whisperProcess.stderr?.on('data', (data) => {
      stderr += data.toString()
    })

    micProcess.stderr?.on('data', (data) => {
      const msg = data.toString()
      // Check for permission denied
      if (msg.includes('Microphone access denied') || msg.includes('permission')) {
        console.error('[Whisper] Microphone permission denied')
        stopDictation()
        dictationReject?.(new Error('Microphone permission denied'))
      }
    })

    // Pipe mic output → whisper stdin
    micProcess.stdout?.pipe(whisperProcess.stdin!)

    whisperProcess.on('error', (err) => {
      console.error('[Whisper] Process error:', err)
      stopDictation()
      dictationReject?.(err)
    })

    micProcess.on('error', (err) => {
      console.error('[Whisper] Mic capture error:', err)
      stopDictation()
      dictationReject?.(err)
    })

    whisperProcess.on('close', (code) => {
      whisperProcess = null

      if (code !== 0) {
        console.error('[Whisper] Exited with code:', code, stderr)
        stopDictation()
        dictationReject?.(new Error(`Whisper failed with code ${code}`))
        return
      }

      try {
        const result = JSON.parse(stdout) as WhisperResult
        stopDictation()
        dictationResolve?.(result)
      } catch (e) {
        console.error('[Whisper] Failed to parse output:', stdout)
        stopDictation()
        dictationReject?.(new Error('Invalid whisper output'))
      }
    })
  })
}

/**
 * Stop dictation: send SIGTERM to mic-capture, which flushes and exits.
 * Whisper will then finish transcription and resolve the promise.
 */
export function stopDictation(): void {
  if (micProcess) {
    micProcess.kill('SIGTERM')
    micProcess = null
  }

  dictationActive = false
  dictationResolve = null
  dictationReject = null
}

/**
 * Check if dictation is currently active.
 */
export function isDictationActive(): boolean {
  return dictationActive
}
