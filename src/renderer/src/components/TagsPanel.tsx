import React, { useState, useMemo } from 'react'
import { useAppContext } from '../App'

// ---------------------------------------------------------------------------
// TagsPanel
//
// Renders a collapsible list of all tags extracted from the vault, sorted by
// frequency (most-used first). Clicking a tag dispatches TAG_FILTER_TOGGLE to
// filter the file tree to only files that contain that tag.
// ---------------------------------------------------------------------------

export function TagsPanel(): React.JSX.Element {
  const { state, dispatch } = useAppContext()
  const [isExpanded, setIsExpanded] = useState(true)

  // Derive sorted tag list from tagIndex — memoised on tagIndex changes
  const tags = useMemo(
    () =>
      Array.from(state.tagIndex.entries())
        .map(([tag, paths]) => ({ tag, count: paths.size }))
        .sort((a, b) => b.count - a.count),
    [state.tagIndex]
  )

  return (
    <div className="tags-panel shrink-0">
      {/* Section header */}
      <button
        className="w-full flex items-center justify-between px-3 py-1.5 text-xs font-semibold
                   text-nabu-text-muted uppercase tracking-wide hover:text-nabu-text
                   transition-colors select-none"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded((prev) => !prev)}
      >
        <span>Tags</span>
        <span aria-hidden="true" className="text-nabu-text-faint">
          {isExpanded ? '▼' : '▶'}
        </span>
      </button>

      {/* Tag list */}
      {isExpanded && (
        <div className="px-2 pb-2">
          {state.tagIndex.size === 0 ? (
            <p className="px-1 py-1 text-xs text-nabu-text-faint italic">No tags found</p>
          ) : (
            <div className="flex flex-col gap-0.5">
              {tags.map(({ tag, count }) => {
                const isPressed = state.selectedTags.has(tag)
                return (
                  <button
                    key={tag}
                    role="button"
                    aria-pressed={isPressed}
                    onClick={() => dispatch({ type: 'TAG_FILTER_TOGGLE', payload: tag })}
                    className={[
                      'flex items-center justify-between w-full px-2 py-0.5 text-xs rounded',
                      'transition-colors text-left select-none',
                      isPressed
                        ? 'bg-nabu-accent/20 text-nabu-accent'
                        : 'text-nabu-text-muted hover:text-nabu-text hover:bg-nabu-bg-mute'
                    ].join(' ')}
                  >
                    <span className="truncate">#{tag}</span>
                    <span
                      className={[
                        'ml-1 shrink-0 tabular-nums',
                        isPressed ? 'text-nabu-accent/70' : 'text-nabu-text-faint'
                      ].join(' ')}
                    >
                      {count}
                    </span>
                  </button>
                )
              })}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
