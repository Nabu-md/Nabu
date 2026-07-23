/**
 * OCRTextPanel.tsx
 *
 * Displays extracted OCR text for an image embed, with a collapsible panel.
 *
 * Requirements: 39.7
 */

import React, { useEffect, useState } from 'react'
import path from 'path'

interface OCRTextPanelProps {
  /** The image path from an embed node */
  imagePath: string
}

/**
 * Check if an OCR companion note exists for the given image.
 */
async function checkOCRCompanion(
  imagePath: string
): Promise<{ exists: boolean; ocrText?: string }> {
  // Derive companion note path: image.png -> image.ocr.md
  const dir = path.dirname(imagePath)
  const ext = path.extname(imagePath)
  const baseName = path.basename(imagePath, ext)
  const companionPath = path.join(dir, `${baseName}.ocr.md`)

  try {
    const result = await window.electron.file.get(companionPath)
    if (result.ast) {
      // Extract text content from the AST (blockquote with ocr text)
      let ocrText = ''
      const extractText = (nodes: unknown[]): void => {
        for (const node of nodes) {
          const n = node as { type?: string; value?: string; children?: unknown[] }
          if (n.type === 'text' && n.value) {
            ocrText += n.value + ' '
          }
          if (n.children && Array.isArray(n.children)) {
            extractText(n.children)
          }
        }
      }
      extractText(result.ast.children as unknown[])
      return { exists: true, ocrText: ocrText.trim() }
    }
  } catch {
    // No companion note exists
  }

  return { exists: false }
}

/**
 * OCR Text Panel component.
 * Renders a collapsible panel below an image embed showing extracted OCR text.
 */
export function OCRTextPanel({ imagePath }: OCRTextPanelProps): React.JSX.Element | null {
  const [isExpanded, setIsExpanded] = useState(true)
  const [ocrText, setOcrText] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [hasCompanion, setHasCompanion] = useState(false)

  useEffect(() => {
    let cancelled = false
    setIsLoading(true)

    checkOCRCompanion(imagePath)
      .then((result) => {
        if (!cancelled) {
          setHasCompanion(result.exists)
          setOcrText(result.ocrText ?? null)
          setIsLoading(false)
        }
      })
      .catch(() => {
        if (!cancelled) {
          setHasCompanion(false)
          setIsLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [imagePath])

  // No companion note — don't render anything (Req 39.7)
  if (isLoading || !hasCompanion) {
    return null
  }

  return (
    <div
      className="ocr-text-panel mt-2 border-t border-white/10 pt-2"
      aria-label="Extracted text from image"
    >
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded((prev) => !prev)}
        className="flex items-center gap-2 text-xs font-semibold text-white/60 hover:text-white/80 transition-colors w-full text-left mb-1"
      >
        <span>Extracted text</span>
        <span className="text-[10px] opacity-60" aria-hidden="true">
          {isExpanded ? '▲' : '▼'}
        </span>
        <span className="ml-auto text-[10px] text-white/30 font-normal">(OCR - macOS Vision)</span>
      </button>
      {isExpanded && ocrText && (
        <blockquote className="border-l-2 border-white/25 pl-3 text-white/55 italic text-xs my-1">
          {ocrText}
        </blockquote>
      )}
    </div>
  )
}
