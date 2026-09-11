import { memo, useCallback, useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import { CaretDown, CaretRight, Folder } from '@phosphor-icons/react'
import { cn } from '@/lib/utils'
import type { VaultEntry } from '../../types'

const TREE_INDENT_PX = 12

interface FolderNoteRowProps {
  entry: VaultEntry
  depth: number
  childrenCount: number
  isExpanded?: boolean
  isSelected?: boolean
  onToggle: () => void
  onOpen?: (entry: VaultEntry) => void
  onContextMenu?: (entry: VaultEntry, event: React.MouseEvent<HTMLElement>) => void
  registerRow?: (key: string, element: HTMLElement | null) => void
  rowKey?: string
  focusSiblingRow?: (key: string, offset: number) => void
}

/**
 * Tree row for a `type: Folder` note. Visually a folder row; toggling shows
 * the linked child notes, opening (double click / context action) opens the
 * note itself in the editor.
 */
export const FolderNoteRow = memo(function FolderNoteRow({
  entry,
  depth,
  childrenCount,
  isExpanded = false,
  isSelected = false,
  onToggle,
  onOpen,
  onContextMenu,
  registerRow,
  rowKey = `folder-note:${entry.path}`,
  focusSiblingRow,
}: FolderNoteRowProps) {
  const rowRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    registerRow?.(rowKey, rowRef.current)
    return () => registerRow?.(rowKey, null)
  }, [registerRow, rowKey])

  const handleKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.key === 'ArrowDown') {
        event.preventDefault()
        focusSiblingRow?.(rowKey, 1)
        return
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault()
        focusSiblingRow?.(rowKey, -1)
        return
      }
      if (event.key === 'Enter') {
        event.preventDefault()
        onToggle()
      }
    },
    [focusSiblingRow, onToggle, rowKey],
  )

  return (
    <div
      ref={rowRef}
      className={cn(
        'group relative flex items-center gap-1 rounded transition-colors',
        isSelected ? 'bg-[var(--accent-blue-light)] text-primary' : 'text-foreground hover:bg-accent',
      )}
      style={{ paddingLeft: 8 + depth * TREE_INDENT_PX, borderRadius: 4, paddingTop: 2, paddingBottom: 2 }}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onClick={onToggle}
      onDoubleClick={() => onOpen?.(entry)}
      onContextMenu={(event) => {
        event.preventDefault()
        onContextMenu?.(entry, event)
      }}
      aria-expanded={childrenCount > 0 ? isExpanded : undefined}
      data-testid={`folder-note-row:${entry.title}`}
    >
      <button
        type="button"
        className="flex h-auto flex-1 items-center justify-start gap-1.5 rounded text-left text-[13px] font-medium hover:bg-transparent"
        style={{ paddingTop: 4, paddingBottom: 4, paddingLeft: 0, paddingRight: 12 }}
        title={entry.path}
        tabIndex={-1}
      >
        {childrenCount > 0 ? (
          isExpanded ? (
            <CaretDown size={12} className="shrink-0 text-muted-foreground" />
          ) : (
            <CaretRight size={12} className="shrink-0 text-muted-foreground" />
          )
        ) : (
          <span className="inline-block w-3 shrink-0" />
        )}
        <Folder size={14} className="shrink-0 text-muted-foreground" aria-hidden />
        <span className="truncate">{entry.title}</span>
        {childrenCount > 0 && (
          <span className="ml-auto text-[11px] text-muted-foreground" data-testid="folder-note-child-count">
            {childrenCount}
          </span>
        )}
      </button>
    </div>
  )
})
