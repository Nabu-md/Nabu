/**
 * KanbanBlock.tsx
 *
 * Kanban board block type for Nabu (Phase 6).
 * Renders as a horizontal row of columns based on frontmatter status.
 * Fetches real data from vault via IPC and writes status changes back.
 *
 * Requirements: Phase 6 (Kanban Board)
 */

import React, { useCallback, useEffect, useRef, useState } from 'react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface KanbanCardData {
  filePath: string
  title: string
  content: string
  tags: string[]
  status: string
}

interface KanbanBlockProps {
  folderPath: string
  vaultPath?: string
}

// ---------------------------------------------------------------------------
// Card Component
// ---------------------------------------------------------------------------

function KanbanCard({ card }: { card: KanbanCardData }): React.JSX.Element {
  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.effectAllowed = 'move'
      e.dataTransfer.setData('text/plain', card.filePath)
      e.dataTransfer.setData('application/nabu-file', card.filePath)
      const el = e.currentTarget as HTMLElement
      el.classList.add('opacity-50')
    },
    [card.filePath]
  )

  const handleDragEnd = useCallback((e: React.DragEvent) => {
    const el = e.currentTarget as HTMLElement
    el.classList.remove('opacity-50')
  }, [])

  return (
    <div
      draggable
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      className="bg-nabu-bg border border-nabu-border rounded p-2 mb-2 cursor-grab hover:shadow-md transition-shadow"
    >
      <div className="font-semibold text-sm text-white/90">{card.title}</div>
      {card.content && (
        <div className="text-xs text-white/50 mt-1 line-clamp-2">{card.content}</div>
      )}
      {card.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-2">
          {card.tags.map((tag) => (
            <span key={tag} className="text-xs text-nabu-accent/70">
              #{tag}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Column Component
// ---------------------------------------------------------------------------

function KanbanColumn({
  status,
  cards,
  onDrop
}: {
  status: string
  cards: KanbanCardData[]
  onDrop: (newStatus: string) => void
}): React.JSX.Element {
  const [isDragOver, setIsDragOver] = useState(false)

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
    setIsDragOver(true)
  }, [])

  const handleDragLeave = useCallback(() => {
    setIsDragOver(false)
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setIsDragOver(false)
      onDrop(status)
    },
    [status, onDrop]
  )

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      className={`flex-1 min-w-56 max-w-72 mx-1 rounded-lg transition-colors ${
        isDragOver ? 'bg-nabu-accent/10 ring-1 ring-nabu-accent/30' : ''
      }`}
    >
      <div className="font-semibold text-sm text-white/80 mb-2 px-2 py-1 border-b border-nabu-border/50">
        {status} <span className="text-white/40 font-normal">({cards.length})</span>
      </div>
      <div className="px-2 pb-2 min-h-[60px]">
        {cards.length === 0 && (
          <div className="text-xs text-white/30 text-center py-4 italic">Drop cards here</div>
        )}
        {cards.map((card) => (
          <KanbanCard key={card.filePath} card={card} />
        ))}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function KanbanBlock({ folderPath, vaultPath: _vaultPath }: KanbanBlockProps): React.JSX.Element {
  const [data, setData] = useState<{ statuses: string[]; cards: KanbanCardData[] } | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const draggedFileRef = useRef<string | null>(null)

  // Fetch kanban data from vault
  useEffect(() => {
    if (!folderPath || typeof window.electron.kanban === 'undefined') return

    setLoading(true)
    setError(null)

    window.electron.kanban
      .getData(folderPath, folderPath)
      .then((result) => {
        setData(result as { statuses: string[]; cards: KanbanCardData[] })
        setLoading(false)
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : 'Failed to load kanban data')
        setLoading(false)
      })
  }, [folderPath])

  // Track dragged file via a global ref listener
  useEffect(() => {
    const handler = (e: DragEvent): void => {
      const filePath = e.dataTransfer?.getData('application/nabu-file')
      if (filePath) {
        draggedFileRef.current = filePath
      }
    }
    document.addEventListener('dragstart', handler)
    return () => document.removeEventListener('dragstart', handler)
  }, [])

  const handleDrop = useCallback(
    async (newStatus: string) => {
      if (!data) return

      const filePath = draggedFileRef.current
      if (!filePath) return

      const movedCard = data.cards.find((c) => c.filePath === filePath)
      if (!movedCard || movedCard.status === newStatus) return

      // Optimistic update — move card locally
      draggedFileRef.current = null
      setData((prev) => {
        if (!prev) return prev
        return {
          ...prev,
          cards: prev.cards.map((c) =>
            c.filePath === filePath ? { ...c, status: newStatus } : c
          )
        }
      })

      // Write to file via IPC
      try {
        const result = await window.electron.kanban.setStatus(folderPath, filePath, newStatus)
        if (!result.success) {
          throw new Error(result.error ?? 'Unknown error')
        }
      } catch (err) {
        console.error('[KanbanBlock] Failed to update status:', err)
        // Rollback on failure
        setData((prev) => {
          if (!prev) return prev
          return {
            ...prev,
            cards: prev.cards.map((c) =>
              c.filePath === filePath ? { ...c, status: movedCard.status } : c
            )
          }
        })
      }
    },
    [data, folderPath]
  )

  // Loading state
  if (loading) {
    return (
      <div className="kanban-block flex items-center justify-center h-32 text-sm text-white/40">
        Loading kanban board…
      </div>
    )
  }

  // Error state
  if (error) {
    return (
      <div className="kanban-block flex items-center justify-center h-32 text-sm text-red-400/80">
        Failed to load kanban: {error}
      </div>
    )
  }

  // Empty state
  if (!data || data.cards.length === 0) {
    return (
      <div className="kanban-block flex items-center justify-center h-32 text-sm text-white/40">
        No notes with status frontmatter found — add `status:` to your notes
      </div>
    )
  }

  // Group cards by status
  const grouped: Record<string, KanbanCardData[]> = {}
  for (const status of data.statuses) {
    grouped[status] = []
  }
  for (const card of data.cards) {
    if (!grouped[card.status]) grouped[card.status] = []
    grouped[card.status].push(card)
  }

  return (
    <div
      className="kanban-block flex gap-3 p-4 overflow-x-auto rounded-lg border border-nabu-border/30 bg-nabu-bg/50"
      data-component="kanban-block"
    >
      {data.statuses.map((status) => (
        <KanbanColumn
          key={status}
          status={status}
          cards={grouped[status] ?? []}
          onDrop={handleDrop}
        />
      ))}
    </div>
  )
}
