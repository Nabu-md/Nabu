/**
 * PdfViewer.tsx
 *
 * React component for rendering PDF documents in the renderer process.
 * Delegates PDF loading and page rasterisation to the main process via the
 * `pdf:open` and `pdf:render-page` IPC channels (which use pdfjs-dist + the
 * `canvas` package to produce base64 PNG data URIs). The renderer only
 * displays the returned images, keeping the PDF parsing off the UI thread.
 *
 * Requirements: 40.1, 40.2, 40.3
 */

import React, { useEffect, useState, useRef, useCallback } from 'react'
import { PDFAnnotation } from '../../../shared/types'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface PdfViewerProps {
  /** The PDF file path */
  filePath: string
  /** Initial scale (0.5 to 2.0) */
  initialScale?: number
  /** Called when the user closes the PDF viewer */
  onClose?: () => void
}

// ---------------------------------------------------------------------------
// PDF Viewer Component
// ---------------------------------------------------------------------------

/**
 * PDF Viewer component. Renders pages lazily (current page + 1 buffer each
 * direction) by requesting rasterised PNGs from the main process.
 */
export function PdfViewer({
  filePath,
  initialScale = 1.0,
  onClose
}: PdfViewerProps): React.JSX.Element {
  const [totalPages, setTotalPages] = useState(0)
  const [currentPage, setCurrentPage] = useState(1)
  const [scale, setScale] = useState(initialScale)
  const [pageImages, setPageImages] = useState<Record<number, string>>({})
  const [metadata, setMetadata] = useState<{ title?: string; author?: string }>({})
  const [annotations, setAnnotations] = useState<PDFAnnotation[]>([])
  const [isAnnotating, setIsAnnotating] = useState(false)
  const [selectedColor, setSelectedColor] = useState<PDFAnnotation['color']>('yellow')
  const [error, setError] = useState<string | null>(null)
  const pageRefs = useRef<Map<number, HTMLImageElement>>(new Map())

  // Open PDF (metadata + page count) via IPC (Req 40.2)
  useEffect(() => {
    let cancelled = false

    async function openPdf(): Promise<void> {
      try {
        setTotalPages(0)
        setCurrentPage(1)
        setPageImages({})
        setError(null)

        const result = await window.electron.pdf.open(filePath)
        if (cancelled) return

        if (result.error) {
          setError(result.error)
          return
        }

        setTotalPages(result.totalPages)
        setMetadata({
          title: result.metadata.title,
          author: result.metadata.author
        })
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err))
        }
      }
    }

    openPdf()

    return () => {
      cancelled = true
    }
  }, [filePath])

  // Render a single page via IPC (Req 40.2)
  const renderPage = useCallback(
    async (pageNumber: number): Promise<void> => {
      if (pageNumber < 1 || pageNumber > totalPages) return
      if (pageImages[pageNumber]) return

      try {
        const result = await window.electron.pdf.renderPage(filePath, pageNumber, scale)
        if (result.error) {
          console.error(`Failed to render page ${pageNumber}:`, result.error)
          return
        }
        setPageImages((prev) => ({ ...prev, [pageNumber]: result.dataUri }))
      } catch (err) {
        console.error(`Failed to render page ${pageNumber}:`, err)
      }
    },
    [filePath, scale, totalPages, pageImages]
  )

  // Lazy render: current page + 1 buffer each direction (Req 40.3)
  useEffect(() => {
    const pagesToRender = [currentPage, currentPage - 1, currentPage + 1].filter(
      (p) => p >= 1 && p <= totalPages
    )
    pagesToRender.forEach((pageNum) => {
      void renderPage(pageNum)
    })
  }, [currentPage, totalPages, renderPage])

  // Changing zoom invalidates cached rasterisations — re-render at new scale
  useEffect(() => {
    setPageImages({})
  }, [scale])

  // Scroll to a specific page
  const scrollToPage = useCallback((pageNumber: number) => {
    const element = pageRefs.current.get(pageNumber)
    element?.scrollIntoView({ behavior: 'smooth' })
  }, [])

  // Create an annotation from the current text selection on a page image
  const handlePageMouseUp = useCallback(
    (pageNumber: number, img: HTMLImageElement) => {
      if (!isAnnotating) return
      const selection = window.getSelection()
      const selectedText = selection?.toString().trim()
      if (!selectedText || !selection || selection.rangeCount === 0) return

      const range = selection.getRangeAt(0)
      const selRect = range.getBoundingClientRect()
      const imgRect = img.getBoundingClientRect()
      if (selRect.width === 0 || selRect.height === 0) return

      const rect = {
        x: selRect.left - imgRect.left,
        y: selRect.top - imgRect.top,
        w: selRect.width,
        h: selRect.height
      }

      const newAnnotation: PDFAnnotation = {
        id: crypto.randomUUID(),
        page: pageNumber,
        rect,
        text: selectedText,
        color: selectedColor,
        timestamp: Date.now()
      }
      setAnnotations((prev) => [...prev, newAnnotation])
      selection.removeAllRanges()
      setIsAnnotating(false)
    },
    [isAnnotating, selectedColor]
  )

  // Navigation handlers
  const goToPreviousPage = useCallback(() => {
    setCurrentPage((prev) => Math.max(1, prev - 1))
  }, [])

  const goToNextPage = useCallback(() => {
    setCurrentPage((prev) => Math.min(totalPages, prev + 1))
  }, [totalPages])

  const zoomIn = useCallback(() => {
    setScale((prev) => Math.min(2.0, Math.round((prev + 0.1) * 10) / 10))
  }, [])

  const zoomOut = useCallback(() => {
    setScale((prev) => Math.max(0.5, Math.round((prev - 0.1) * 10) / 10))
  }, [])

  const resetZoom = useCallback(() => {
    setScale(1.0)
  }, [])

  if (error) {
    return (
      <div className="pdf-viewer pdf-viewer--error" role="alert">
        <p className="pdf-viewer__error">Failed to load PDF: {error}</p>
        {onClose && (
          <button
            onClick={onClose}
            className="px-3 py-1 mt-2 rounded bg-nabu-surface hover:bg-nabu-border"
          >
            Close
          </button>
        )}
      </div>
    )
  }

  if (totalPages === 0) {
    return (
      <div className="pdf-viewer pdf-viewer--loading">
        <p>Loading PDF…</p>
      </div>
    )
  }

  return (
    <div className="pdf-viewer">
      {/* Toolbar */}
      <div className="pdf-viewer__toolbar flex items-center gap-2 p-2 border-b border-nabu-border">
        {onClose && (
          <button
            onClick={onClose}
            className="px-2 py-1 text-sm rounded hover:bg-nabu-surface"
            title="Close PDF viewer"
          >
            ✕
          </button>
        )}

        {/* Navigation */}
        <div className="flex items-center gap-1">
          <button
            onClick={goToPreviousPage}
            disabled={currentPage <= 1}
            className="px-2 py-1 text-sm rounded hover:bg-nabu-surface"
            title="Previous page"
          >
            ‹
          </button>
          <span className="text-sm text-nabu-text-muted">
            {currentPage} / {totalPages}
          </span>
          <button
            onClick={goToNextPage}
            disabled={currentPage >= totalPages}
            className="px-2 py-1 text-sm rounded hover:bg-nabu-surface"
            title="Next page"
          >
            ›
          </button>
        </div>

        {/* Zoom controls */}
        <div className="flex items-center gap-1 border-l border-nabu-border pl-2 ml-2">
          <button
            onClick={zoomOut}
            className="px-2 py-1 text-sm rounded hover:bg-nabu-surface"
            title="Zoom out"
          >
            −
          </button>
          <button
            onClick={resetZoom}
            className="px-2 py-1 text-sm rounded hover:bg-nabu-surface"
            title="Reset zoom"
          >
            {Math.round(scale * 100)}%
          </button>
          <button
            onClick={zoomIn}
            className="px-2 py-1 text-sm rounded hover:bg-nabu-surface"
            title="Zoom in"
          >
            +
          </button>
        </div>

        {/* Annotation mode */}
        <div className="flex items-center gap-1 border-l border-nabu-border pl-2 ml-2">
          <button
            onClick={() => setIsAnnotating(!isAnnotating)}
            className={`px-2 py-1 text-sm rounded ${
              isAnnotating ? 'bg-nabu-accent text-nabu-bg' : 'hover:bg-nabu-surface'
            }`}
            title="Annotate mode"
          >
            📝
          </button>
        </div>

        {/* Color selector */}
        {isAnnotating && (
          <div className="flex items-center gap-1 border-l border-nabu-border pl-2 ml-2">
            {(['yellow', 'green', 'blue', 'pink', 'orange'] as const).map((color) => (
              <button
                key={color}
                onClick={() => setSelectedColor(color)}
                className={`w-5 h-5 rounded-sm border-2 ${
                  selectedColor === color ? 'border-nabu-accent' : 'border-transparent'
                }`}
                style={{ backgroundColor: color }}
                title={`${color} highlighter`}
              />
            ))}
          </div>
        )}

        {/* Metadata */}
        {(metadata.title || metadata.author) && (
          <div className="ml-auto text-xs text-nabu-text-muted truncate max-w-[40%]">
            {metadata.title ?? metadata.author}
          </div>
        )}
      </div>

      {/* PDF Pages */}
      <div className="pdf-viewer__pages flex flex-col items-center gap-4 p-4 overflow-auto">
        {Array.from({ length: totalPages }, (_, i) => i + 1).map((pageNumber) => (
          <div
            key={pageNumber}
            id={`pdf-page-${pageNumber}`}
            className="pdf-viewer__page relative bg-nabu-bg-paper rounded shadow-lg"
            onMouseUp={(e) => {
              const img = e.currentTarget.querySelector('img')
              if (img) handlePageMouseUp(pageNumber, img)
            }}
          >
            {pageImages[pageNumber] ? (
              <img
                ref={(el) => {
                  if (el) pageRefs.current.set(pageNumber, el)
                }}
                src={pageImages[pageNumber]}
                alt={`Page ${pageNumber}`}
                className="pdf-viewer__canvas block"
                style={{ width: `${Math.round(scale * 100)}%` }}
              />
            ) : (
              <div className="pdf-viewer__page-placeholder w-[600px] h-[800px] flex items-center justify-center text-nabu-text-muted">
                Rendering page {pageNumber}…
              </div>
            )}

            {/* Render annotations for this page */}
            {annotations
              .filter((a) => a.page === pageNumber)
              .map((annotation) => (
                <div
                  key={annotation.id}
                  className="pdf-viewer__annotation absolute pointer-events-none"
                  style={{
                    left: annotation.rect.x,
                    top: annotation.rect.y,
                    width: annotation.rect.w,
                    height: annotation.rect.h,
                    backgroundColor: annotation.color,
                    opacity: 0.3
                  }}
                  title={annotation.text}
                />
              ))}
          </div>
        ))}
      </div>
    </div>
  )
}

export default PdfViewer
