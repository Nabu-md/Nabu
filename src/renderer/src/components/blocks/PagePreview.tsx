/**
 * PagePreview.tsx
 *
 * Hover preview popover for wiki links and embeds.
 * Shows rendered excerpt of target note without navigating.
 *
 * Requirements: 28.1, 28.2, 28.3, 28.4, 28.5, 28.6, 28.7, 28.8
 */

import React, { useEffect, useState } from 'react'

interface PagePreviewProps {
  filePath: string
  isActive: boolean
  onOpen: () => void
}

export function PagePreview({
  filePath,
  isActive,
  onOpen
}: PagePreviewProps): React.JSX.Element | null {
  const [content, setContent] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    if (!isActive) {
      setVisible(false)
      return
    }

    // Show preview after delay
    const timer = window.setTimeout(() => {
      if (!filePath) return
      setLoading(true)
      window.electron.file
        .get(filePath)
        .then(() => {
          // For now, just store that we loaded - real implementation would
          // render via renderNode pipeline
          setContent('Preview...')
          setLoading(false)
          setVisible(true)
        })
        .catch(() => {
          setLoading(false)
          setVisible(false)
        })
    }, 300) // default hover delay

    return () => {
      clearTimeout(timer)
    }
  }, [isActive, filePath])

  if (!visible && !loading) return null

  return (
    <div
      className="page-preview fixed z-50 max-w-sm max-h-64 overflow-y-auto p-3 rounded-lg shadow-xl border border-nabu-border bg-nabu-bg"
      style={{
        // Position near cursor - simplified positioning
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)'
      }}
      onMouseLeave={() => setVisible(false)}
    >
      {loading ? (
        <div className="text-xs text-nabu-text-muted">Loading preview...</div>
      ) : (
        <>
          <div className="text-xs text-nabu-text mb-2">{content ?? 'No preview available'}</div>
          <button
            type="button"
            onClick={onOpen}
            className="text-xs px-2 py-1 rounded bg-nabu-accent text-white hover:opacity-90"
          >
            Open
          </button>
        </>
      )}
    </div>
  )
}
