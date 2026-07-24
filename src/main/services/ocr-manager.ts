/**
 * ocr-manager.ts
 *
 * Manages OCR processing for image files added to vault.
 * Spawns the Swift OCR helper and manages an AsyncQueue for sequential processing.
 *
 * Requirements: 39.1, 39.2, 39.3, 39.4, 39.5, 39.6, 39.7, 39.8
 */

import { spawn } from 'child_process'
import path from 'path'
import fs from 'fs/promises'
import { app } from 'electron'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** OCR result from the Swift helper */
export interface OCRResult {
  text: string
  blocks: OCRBlock[]
}

export interface OCRBlock {
  rect: { x: number; y: number; w: number; h: number }
  text: string
  confidence: number
}

/** Result of processing an image file */
export interface ImageOCROptions {
  imagePath: string
  vaultPath: string
  ocrEnabled: boolean
}

// ---------------------------------------------------------------------------
// OCR Queue - processes files sequentially to avoid saturating CPU
// ---------------------------------------------------------------------------

interface OCRQueueItem {
  imagePath: string
  vaultPath: string
  resolve: (value: OCRResult | null) => void
  reject: (error: Error) => void
}

const ocrQueue: OCRQueueItem[] = []
let isProcessing = false

/**
 * AsyncQueue for OCR jobs - processes one image at a time (Req 39.5)
 */
async function processOCRQueue(): Promise<void> {
  if (isProcessing || ocrQueue.length === 0) return

  isProcessing = true

  while (ocrQueue.length > 0) {
    const item = ocrQueue.shift()!
    try {
      const result = await runOCR(item.imagePath)
      item.resolve(result)
    } catch (err) {
      item.reject(err instanceof Error ? err : new Error(String(err)))
    }
  }

  isProcessing = false
}

/**
 * Queue an OCR job for an image file
 */
export function enqueueOCR(imagePath: string, vaultPath: string): Promise<OCRResult | null> {
  return new Promise((resolve, reject) => {
    ocrQueue.push({ imagePath, vaultPath, resolve, reject })
    processOCRQueue()
  })
}

// ---------------------------------------------------------------------------
// Swift process spawning
// ---------------------------------------------------------------------------

/**
 * Run the Swift OCR helper on an image file
 */
async function runOCR(imagePath: string): Promise<OCRResult | null> {
  // Skip non-macOS platforms (Req 39.6)
  if (process.platform !== 'darwin') {
    console.debug(`[OCR] Skipping OCR on non-macOS platform for ${imagePath}`)
    return null
  }

  return new Promise((resolve, reject) => {
    // Resolve the Swift script path
    const swiftPath = app.isPackaged
      ? path.join(process.resourcesPath, 'ocr.swift')
      : path.join(__dirname, '..', '..', '..', 'scripts', 'ocr.swift')

    const child = spawn('swift', [swiftPath, imagePath], {
      stdio: ['ignore', 'pipe', 'pipe']
    })

    let stdout = ''
    let stderr = ''

    child.stdout?.on('data', (data) => {
      stdout += data.toString()
    })

    child.stderr?.on('data', (data) => {
      stderr += data.toString()
    })

    child.on('close', (code) => {
      // Exit code 0 = success
      if (code === 0) {
        try {
          const result = JSON.parse(stdout) as OCRResult
          // Empty text skip (Req 39.4)
          if (!result.text || result.text.trim().length === 0) {
            resolve(null)
          } else {
            resolve(result)
          }
        } catch (parseErr) {
          reject(new Error(`[OCR] Failed to parse Swift output: ${String(parseErr)}`))
        }
      }
      // Exit code 1 = permission denied (Req 39.6)
      else if (code === 1) {
        console.warn(`[OCR] Permission denied for image ${imagePath}`)
        resolve(null)
      }
      // Exit code 2 = corrupt image (Req 39.8)
      else if (code === 2) {
        console.warn(`[OCR] Corrupt image ${imagePath}`)
        resolve(null)
      } else {
        reject(new Error(`[OCR] Swift process exited with code ${code}: ${stderr}`))
      }
    })

    child.on('error', (err) => {
      reject(err)
    })
  })
}

// ---------------------------------------------------------------------------
// Companion note creation
// ---------------------------------------------------------------------------

/**
 * Create a companion .ocr.md file for an image
 */
export async function createOCRCompanionNote(
  imagePath: string,
  ocrResult: OCRResult,
  _vaultPath: string
): Promise<string | null> {
  // Derive companion note path: chart.png -> chart.ocr.md
  const dir = path.dirname(imagePath)
  const ext = path.extname(imagePath)
  const baseName = path.basename(imagePath, ext)
  const companionPath = path.join(dir, `${baseName}.ocr.md`)

  // Build frontmatter
  const frontmatter = `---
source: [[${path.basename(imagePath)}]]
ocr_date: ${new Date().toISOString()}
ocr_model: macOS_Vision
---

`

  // Build body with block quote and block reference
  const body = `> ${ocrResult.text.replace(/\n/g, '\n> ')}\n\n^ocr\n`

  const content = frontmatter + body

  try {
    await fs.writeFile(companionPath, content, 'utf-8')
    return companionPath
  } catch (err) {
    console.error(`[OCR] Failed to create companion note: ${String(err)}`)
    return null
  }
}

// ---------------------------------------------------------------------------
// Image file detection
// ---------------------------------------------------------------------------

const IMAGE_EXTENSIONS = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.tiff'])

/**
 * Check if a file is an image that should be processed for OCR
 */
export function isImageFile(filePath: string): boolean {
  const ext = path.extname(filePath).toLowerCase()
  return IMAGE_EXTENSIONS.has(ext)
}
