import React from 'react'
import { PhrasingContent } from 'mdast'
import { TaskList as TaskListNode, TaskItem as TaskItemNode } from '@shared/types'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface TaskListProps {
  node: TaskListNode
  /**
   * optimisticToggles is managed by the parent (NoteView) so that optimistic
   * state can be cleared when an external edit arrives for the current file.
   * Key: lineIndex (0-based), Value: overridden checked state.
   */
  optimisticToggles: Record<number, boolean>
  /**
   * Called by a checkbox change. The parent applies the optimistic update and
   * sends the IPC message, reverting on failure.
   */
  onToggle: (lineIndex: number) => void
  /**
   * Optional renderer for PhrasingContent children. When provided, inline
   * nodes (bold, italic, code, links, wiki-links, etc.) inside each task item
   * are rendered via the shared AST visitor rather than falling back to plain
   * text extraction.
   */
  renderChildren?: (nodes: PhrasingContent[]) => React.ReactNode
}

// ---------------------------------------------------------------------------
// TaskItem
// ---------------------------------------------------------------------------

interface TaskItemProps {
  item: TaskItemNode
  checked: boolean
  onToggle: (lineIndex: number) => void
  renderChildren?: (nodes: PhrasingContent[]) => React.ReactNode
}

function TaskItem({ item, checked, onToggle, renderChildren }: TaskItemProps): React.JSX.Element {
  // Compute an accessible label from the item's children
  const plainTextLabel = item.children
    .map((c) => ('value' in c ? (c as { value: string }).value : ''))
    .join('')
    .trim()

  const ariaLabel = plainTextLabel || `Task at line ${item.lineIndex + 1}`

  const handleChange = (): void => {
    onToggle(item.lineIndex)
  }

  return (
    <li className="flex items-start gap-2 py-0.5">
      <input
        type="checkbox"
        id={`task-${item.lineIndex}`}
        checked={checked}
        onChange={handleChange}
        aria-label={ariaLabel}
        className="mt-0.5 cursor-pointer accent-blue-500 flex-shrink-0"
      />
      <label
        htmlFor={`task-${item.lineIndex}`}
        className={[
          'text-sm leading-relaxed cursor-pointer select-text',
          checked ? 'line-through opacity-50 text-white/50' : 'text-white/75'
        ].join(' ')}
      >
        {renderChildren
          ? renderChildren(item.children)
          : plainTextLabel || <span className="opacity-40 italic">Empty task</span>}
      </label>
    </li>
  )
}

// ---------------------------------------------------------------------------
// TaskList
// ---------------------------------------------------------------------------

/**
 * Renders a GFM task list (- [ ] / - [x]) as an interactive checklist.
 *
 * Optimistic UI is managed by the parent (NoteView) and passed in via props so
 * that the parent can clear optimistic state on external edits. Checkbox clicks
 * are handled synchronously (optimistic update ≤ 16 ms) and reverted by the
 * parent if the IPC call fails.
 *
 * Requirements: 5.1, 5.2, 5.7, 11.3
 */
export function TaskList({
  node,
  optimisticToggles,
  onToggle,
  renderChildren
}: TaskListProps): React.JSX.Element {
  return (
    <ul className="task-list list-none pl-0 my-2 space-y-0.5" role="list" aria-label="Task list">
      {node.items.map((item) => {
        // Determine effective checked state: optimistic override takes precedence
        const checked =
          item.lineIndex in optimisticToggles ? optimisticToggles[item.lineIndex] : item.checked

        return (
          <TaskItem
            key={item.lineIndex}
            item={item}
            checked={checked}
            onToggle={onToggle}
            renderChildren={renderChildren}
          />
        )
      })}
    </ul>
  )
}
