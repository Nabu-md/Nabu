/**
 * KanbanBlock.tsx
 *
 * Kanban board block type for Nabu (Phase 6).
 * Renders as a horizontal row of columns based on frontmatter status.
 *
 * Requirements: Phase 6 (Kanban Board)
 */

import React, { useCallback, useState } from 'react'
import type { Node } from 'mdast'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface KanbanBlockProps {
  node: Node & { statuses: string[]; folderPath: string }
  onStatusChange?: (filePath: string, newStatus: string) => void
}

interface KanbanCardProps {
  filePath: string
  title: string
  content: string
  tags: string[]
  onDragStart: (filePath: string, status: string) => void
  onDragEnd: () => void
  onDrop: (filePath: string, newStatus: string) => void
}

// ---------------------------------------------------------------------------
// Card Component
// ---------------------------------------------------------------------------

function KanbanCard({ filePath, title, content, tags, onDragStart, onDragEnd, onDrop }: KanbanCardProps): React.JSX.Element {
  const handleDragStart = useCallback((e: React.DragEvent) => {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', filePath)
    onDragStart(filePath, '')
  }, [filePath, onDragStart])

  return (
    <div
      draggable
      onDragStart={handleDragStart}
      className="bg-nabu-bg border border-nabu-border rounded p-2 mb-2 cursor-grab hover:shadow-md transition-shadow"
    >
      <div className="font-semibold text-sm text-white/90">{title}</div>
      {content && (
        <div className="text-xs text-white/50 mt-1 line-clamp-2">{content}</div>
      )}
      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-2">
          {tags.map((tag) => (
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

interface KanbanColumnProps {
  status: string
  cards: Array<{ filePath: string; title: string; content: string; tags: string[] }>
  onDragOver: (status: string) => void
  onDrop: (newStatus: string) => void
  onDragStart: (filePath: string, status: string) => void
  onDragEnd: () => void
}

function KanbanColumn({ status, cards, onDragOver, onDrop, onDragStart, onDragEnd }: KanbanColumnProps): React.JSX.Element {
  const [isDragOver, setIsDragOver] = useState(false)

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
    setIsDragOver(true)
    onDragOver(status)
  }, [status, onDragOver])

  const handleDragLeave = useCallback(() => {
    setIsDragOver(false)
  }, [])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragOver(false)
    onDrop(status)
  }, [status, onDrop])

  return (
    <div
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      className={`flex-1 min-w-48 max-w-64 mx-1 ${isDragOver ? 'bg-nabu-accent/10 rounded' : ''}`}
    >
      <div className="font-semibold text-sm text-white/80 mb-2 px-1">{status} ({cards.length})</div>
      <div className="space-y-2">
        {cards.map((card) => (
          <KanbanCard
            key={card.filePath}
            {...card}
            onDragStart={onDragStart}
            onDragEnd={onDragEnd}
            onDrop={() => {}}
          />
        ))}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function KanbanBlock({ node, onStatusChange }: KanbanBlockProps): React.JSX.Element {
  const { statuses, folderPath } = node
  const [draggedFile, setDraggedFile] = useState<string | null>(null)

  const defaultStatuses = ['Backlog', 'In Progress', 'Done']
  const columnStatuses = statuses && statuses.length > 0 ? statuses : defaultStatuses

  // Mock data - in real implementation this would be fetched from vault
  const mockCards: Record<string, Array<{ filePath: string; title: string; content: string; tags: string[] }>> = {
    'Backlog': [
      { filePath: '/note1.md', title: 'Research options', content: 'Look into different solutions...', tags: ['research'] }
    ],
    'In Progress': [
      { filePath: '/note2.md', title: 'Implement UI', content: 'Build the interface...', tags: ['dev', 'urgent'] }
    ],
    'Done': [
      { filePath: '/note3.md', title: 'Setup project', content: 'Initialize repo...', tags: ['setup'] }
    ]
  }

  const handleDragStart = useCallback((filePath: string, _status: string) => {
    setDraggedFile(filePath)
  }, [])

  const handleDragEnd = useCallback(() => {
    setDraggedFile(null)
  }, [])

  const handleDragOver = useCallback((_status: string) => {
    // Visual feedback
  }, [])

  const handleDrop = useCallback((newStatus: string) => {
    if (draggedFile && onStatusChange) {
      onStatusChange(draggedFile, newStatus)
    }
    setDraggedFile(null)
  }, [draggedFile, onStatusChange])

  return (
    <div className="kanban-block flex gap-4 p-4 overflow-x-auto">
      {columnStatuses.map((status) => (
        <KanbanColumn
          key={status}
          status={status}
          cards={mockCards[status] ?? []}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
          onDragStart={handleDragStart}
          onDragEnd={handleDragEnd}
        />
      ))}
    </div>
  )
}