import { useCallback, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

function ocrInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

export interface OcrState {
  running: boolean
  extractTextFromImage: (file: File | string) => Promise<string>
  extractTextFromPdf: (file: File | string) => Promise<string>
}

function toPath(source: File | string): string {
  return typeof source === 'string' ? source : (source as File & { path?: string }).path ?? ''
}

/**
 * macOS Vision OCR bridge. Fully on-device; returns errors on non-macOS or
 * when the file path is unavailable (browser builds cannot read file paths).
 */
export function useOcr(): OcrState {
  const [running, setRunning] = useState(false)

  const extractTextFromImage = useCallback(async (file: File | string) => {
    const path = toPath(file)
    if (!path) throw new Error('OCR needs a file path (unavailable in browser mode)')
    setRunning(true)
    try {
      return await ocrInvoke<string>('ocr_extract_text', { imagePath: path })
    } finally {
      setRunning(false)
    }
  }, [])

  const extractTextFromPdf = useCallback(async (file: File | string) => {
    const path = toPath(file)
    if (!path) throw new Error('OCR needs a file path (unavailable in browser mode)')
    setRunning(true)
    try {
      return await ocrInvoke<string>('ocr_extract_text_from_pdf', { pdfPath: path })
    } finally {
      setRunning(false)
    }
  }, [])

  return { running, extractTextFromImage, extractTextFromPdf }
}
