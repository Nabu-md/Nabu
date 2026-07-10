/**
 * TagsPanel.tsx
 *
 * Hierarchical tag pane that displays tags extracted from the vault in a
 * nested tree structure, with per-tag counts, multi-select filtering,
 * and parent-tag namespace filtering.
 *
 * Requirements: 14.2, 14.3, 14.5
 */

import React, { useState, useMemo, useCallback } from 'react'
import { useAppContext } from '../App'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface TagNode {
  /** The full tag path (e.g. "parent/child"). */
  tag: string
  /** Label to display (the last segment). */
  label: string
  /** Number of files carrying this tag (union of all descendants). */
  count: number
  /** Child tags in the hierarchy. */
  children: TagNode[]
  /** True if this is a "folder" (has children). */
  isFolder: boolean
  /** True if this node or any child is currently selected. */
  isActive: boolean
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build a tree from a flat tag→count map.
 *
 * Namespaced tags like `parent/child/grandchild` are split into segments
 * and nested.  Each node accumulates the counts of all its descendants.
 */
function buildTagTree(tagCounts: Map<string, number>): TagNode[] {
  const root: TagNode[] = []

  for (const [tag, count] of tagCounts) {
    const segments = tag.split('/')
    let currentLevel = root

    for (let i = 0; i < segments.length; i++) {
      const isLast = i === segments.length - 1
      const fullTag = segments.slice(0, i + 1).join('/')
      const existing = currentLevel.find((n) => n.tag === fullTag)

      if (existing) {
        // Accumulate descendant count into parent nodes
        if (isLast) {
          existing.count += count
        }
        currentLevel = existing.children
      } else {
        const node: TagNode = {
          tag: fullTag,
          label: segments[i],
          count: isLast ? count : 0,
          children: [],
          isFolder: !isLast,
          isActive: false
        }
        currentLevel.push(node)
        currentLevel = node.children
      }
    }

    // Propagate leaf counts to parents
    propagateCounts(root)
  }

  return root
}

/**
 * Propagate child counts upward so parent nodes show the union.
 */
function propagateCounts(nodes: TagNode[]): number {
  let total = 0
  for (const node of nodes) {
    if (node.children.length > 0) {
      const childSum = propagateCounts(node.children)
      node.count = Math.max(node.count, childSum)
      total += node.count
    } else {
      total += node.count
    }
  }
  return total
}

// ---------------------------------------------------------------------------
// TagTreeItem
// ---------------------------------------------------------------------------

interface TagTreeItemProps {
  node: TagNode
  selectedTags: Set<string>
  onToggle: (tag: string) => void
  depth: number
}

function TagTreeItem({ node, selectedTags, onToggle, depth }: TagTreeItemProps): React.JSX.Element {
  const [isExpanded, setIsExpanded] = useState(true)
  const hasChildren = node.children.length > 0
  const isPressed = selectedTags.has(node.tag)

  return (
    <div className="tag-tree-item">
      <div className="flex items-center gap-0.5" style={{ paddingLeft: `${depth * 12}px` }}>
        {/* Expand/collapse for folder tags */}
        {hasChildren ? (
          <button
            type="button"
            onClick={() => setIsExpanded((prev) => !prev)}
            className="shrink-0 w-3.5 h-3.5 flex items-center justify-center text-nabu-text-faint hover:text-nabu-text transition-colors"
            aria-label={isExpanded ? 'Collapse' : 'Expand'}
          >
            <span className="text-[8px] leading-none">{isExpanded ? '▼' : '▶'}</span>
          </button>
        ) : (
          <span className="shrink-0 w-3.5" />
        )}

        {/* Tag button */}
        <button
          type="button"
          role="button"
          aria-pressed={isPressed}
          onClick={() => onToggle(node.tag)}
          className={[
            'flex items-center justify-between flex-1 min-w-0 px-1 py-0.5 text-xs rounded',
            'transition-colors text-left select-none',
            isPressed
              ? 'bg-nabu-accent/20 text-nabu-accent'
              : 'text-nabu-text-muted hover:text-nabu-text hover:bg-nabu-bg-mute'
          ].join(' ')}
        >
          <span className="truncate">
            {node.isFolder && !depth && '#'}
            {node.label}
          </span>
          <span
            className={[
              'ml-1 shrink-0 tabular-nums',
              isPressed ? 'text-nabu-accent/70' : 'text-nabu-text-faint'
            ].join(' ')}
          >
            {node.count}
          </span>
        </button>
      </div>

      {/* Children */}
      {hasChildren && isExpanded && (
        <div className="tag-tree-children">
          {node.children.map((child) => (
            <TagTreeItem
              key={child.tag}
              node={child}
              selectedTags={selectedTags}
              onToggle={onToggle}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// TagsPanel
// ---------------------------------------------------------------------------

export function TagsPanel(): React.JSX.Element {
  const { state, dispatch } = useAppContext()

  // Derive sorted tag tree from tagIndex — memoised on tagIndex changes
  const tree = useMemo(() => {
    // Flatten tagIndex to a tag→count map
    const tagCounts = new Map<string, number>()
    for (const [tag, paths] of state.tagIndex) {
      tagCounts.set(tag, paths.size)
    }
    return buildTagTree(tagCounts)
  }, [state.tagIndex])

  // Sort tree nodes: folders first, then by count descending
  const sortedTree = useMemo(() => {
    const sortNodes = (nodes: TagNode[]): TagNode[] => {
      const sorted = [...nodes].sort((a, b) => {
        if (a.isFolder !== b.isFolder) return a.isFolder ? -1 : 1
        return b.count - a.count
      })
      for (const node of sorted) {
        if (node.children.length > 0) {
          node.children = sortNodes(node.children)
        }
      }
      return sorted
    }
    return sortNodes(tree)
  }, [tree])

  const handleToggle = useCallback(
    (tag: string) => {
      dispatch({ type: 'TAG_FILTER_TOGGLE', payload: tag })
    },
    [dispatch]
  )

  const handleClearFilters = useCallback(() => {
    // Clear all selected tags by toggling each one
    for (const tag of state.selectedTags) {
      dispatch({ type: 'TAG_FILTER_TOGGLE', payload: tag })
    }
  }, [dispatch, state.selectedTags])

  const hasActiveFilters = state.selectedTags.size > 0

  return (
    <div className="tags-panel shrink-0">
      {/* Section header */}
      <button
        className="w-full flex items-center justify-between px-3 py-1.5 text-xs font-semibold
                   text-nabu-text-muted uppercase tracking-wide hover:text-nabu-text
                   transition-colors select-none"
        aria-expanded={true}
        onClick={() => {}}
      >
        <span>Tags</span>
        {hasActiveFilters && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              handleClearFilters()
            }}
            className="text-[10px] text-nabu-accent/70 hover:text-nabu-accent transition-colors"
            aria-label="Clear all tag filters"
          >
            Clear filters
          </button>
        )}
      </button>

      {/* Tag tree */}
      <div className="px-2 pb-2">
        {sortedTree.length === 0 ? (
          <p className="px-1 py-1 text-xs text-nabu-text-faint italic">No tags found</p>
        ) : (
          <div className="flex flex-col gap-0.5">
            {sortedTree.map((node) => (
              <TagTreeItem
                key={node.tag}
                node={node}
                selectedTags={state.selectedTags}
                onToggle={handleToggle}
                depth={0}
              />
            ))}
          </div>
        )}

        {/* Active filter indicator (Req 14.5) */}
        {hasActiveFilters && (
          <div className="mt-2 pt-2 border-t border-nabu-border/50">
            <p className="text-[10px] text-nabu-accent/60 px-1">
              Filtering by {state.selectedTags.size} tag{state.selectedTags.size !== 1 ? 's' : ''}
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
