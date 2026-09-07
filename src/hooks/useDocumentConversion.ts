import { useCallback, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

function conversionInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

export interface ConvertCommandError {
  code: 'needsOcr' | 'unsupported'
  message: string
  pages: number[]
  pageCount: number
}

export interface SupportedFormat {
  name: string
  extensions: string[]
}

export interface DocumentConversionState {
  converting: boolean
  convertDocumentToMarkdown: (filePath: string) => Promise<string>
  listSupportedFormats: () => Promise<SupportedFormat[]>
}

/**
 * AnyDoc-backed document conversion (PDF, DOCX, PPTX, XLSX, ODT, RTF, EPUB,
 * CSV → Markdown). When a scanned PDF reports `needsOcr`, callers chain into
 * macOS Vision OCR (useOcr) and merge the extracted text.
 */
export function useDocumentConversion(): DocumentConversionState {
  const [converting, setConverting] = useState(false)

  const convertDocumentToMarkdown = useCallback(async (filePath: string) => {
    setConverting(true)
    try {
      return await conversionInvoke<string>('convert_to_markdown', { filePath })
    } catch (error) {
      if (isConvertCommandError(error)) {
        const detail: ConvertCommandError = error
        const suffix = detail.code === 'needsOcr' ? ' (needs OCR)' : ''
        throw new Error(`${detail.message}${suffix}`)
      }
      throw error
    } finally {
      setConverting(false)
    }
  }, [])

  const listSupportedFormats = useCallback(
    () => conversionInvoke<SupportedFormat[]>('list_supported_formats'),
    [],
  )

  return { converting, convertDocumentToMarkdown, listSupportedFormats }
}

export function isConvertCommandError(error: unknown): error is ConvertCommandError {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    typeof (error as ConvertCommandError).code === 'string'
  )
}
