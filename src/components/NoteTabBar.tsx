import { memo, useCallback, useRef, useState } from 'react'
import { X } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export interface NoteTab {
  path: string
  title: string
  modified?: boolean
}

export interface NoteTabBarProps {
  tabs: NoteTab[]
  activeTabPath: string | null
  modifiedPaths?: Set<string>
  onSelectTab: (path: string) => void
  onCloseTab: (path: string) => void
  onReorderTabs: (fromIndex: number, toIndex: number) => void
}

/**
 * Phase 4 note tab bar: click-to-switch, close (× button, middle-click or
 * Cmd+Click), and HTML5 drag-and-drop reorder for open note tabs.
 */
export const NoteTabBar = memo(function NoteTabBar({
  tabs,
  activeTabPath,
  modifiedPaths,
  onSelectTab,
  onCloseTab,
  onReorderTabs,
}: NoteTabBarProps) {
  const dragIndexRef = useRef<number | null>(null)
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null)
  const [dragging, setDragging] = useState(false)

  const handleCloseClick = useCallback(
    (event: React.MouseEvent, path: string) => {
      event.stopPropagation()
      event.preventDefault()
      onCloseTab(path)
    },
    [onCloseTab],
  )

  const handleDrop = useCallback(
    (event: React.DragEvent, toIndex: number) => {
      event.preventDefault()
      setDragOverIndex(null)
      const fromIndex = dragIndexRef.current
      dragIndexRef.current = null
      if (fromIndex === null || fromIndex === toIndex) return
      onReorderTabs(fromIndex, toIndex)
    },
    [onReorderTabs],
  )

  return (
    <div
      role="tablist"
      aria-label="Open notes"
      className="relative flex shrink-0 items-center overflow-x-auto border-b border-border bg-sidebar"
      data-testid="note-tab-bar"
    >
      {tabs.map((tab, index) => {
        const active = tab.path === activeTabPath
        const modified = tab.modified || modifiedPaths?.has(tab.path) || false
        return (
          <div
            key={tab.path}
            role="tab"
            aria-selected={active}
            tabIndex={0}
            draggable
            onDragStart={(event) => {
              dragIndexRef.current = index
              setDragging(true)
              event.dataTransfer.effectAllowed = 'move'
              event.dataTransfer.setData('text/plain', tab.path)
            }}
            onDragOver={(event) => {
              event.preventDefault()
              if (dragOverIndex !== index) setDragOverIndex(index)
            }}
            onDrop={(event) => handleDrop(event, index)}
            onDragEnd={() => {
              dragIndexRef.current = null
              setDragging(false)
              setDragOverIndex(null)
            }}
            onClick={() => onSelectTab(tab.path)}
            onAuxClick={(event) => {
              if (event.button === 1) handleCloseClick(event, tab.path)
            }}
            onClickCapture={(event) => {
              if (event.metaKey || event.ctrlKey) handleCloseClick(event, tab.path)
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                onSelectTab(tab.path)
              }
            }}
            className={cn(
              'group relative flex min-w-0 max-w-[180px] shrink-0 cursor-pointer items-center gap-1.5 border-r border-border px-3 py-1.5 text-[12px] transition-colors',
              active
                ? 'bg-background font-semibold text-foreground'
                : 'text-muted-foreground hover:bg-[var(--hover)] hover:text-foreground',
              dragOverIndex === index && dragging && 'border-l-2 border-l-[var(--accent-blue)]',
            )}
            style={active ? { boxShadow: 'inset 0 -2px 0 0 var(--accent-blue)' } : undefined}
            data-testid={`note-tab-${index}`}
            data-active={active || undefined}
            title={tab.title}
          >
            <span className="min-w-0 truncate">{tab.title}</span>
            {modified && (
              <span
                className="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--accent-orange)]"
                data-testid={`note-tab-modified-${index}`}
                aria-label="Unsaved changes"
              />
            )}
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              className="ml-0.5 h-4 w-4 shrink-0 p-0 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100"
              aria-label={`Close ${tab.title}`}
              onClick={(event) => handleCloseClick(event, tab.path)}
              data-testid={`note-tab-close-${index}`}
            >
              <X size={11} />
            </Button>
          </div>
        )
      })}
    </div>
  )
})
