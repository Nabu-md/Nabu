/**
 * SearchPanel.tsx
 *
 * Dedicated advanced-search panel for Nabu v2 (Requirement 3).
 * Provides operator-based search (`path:`, `tag:`, `line:`, `content:`,
 * `file:`, `property:`, `regex:`) with result snippets, keyboard
 * navigation, and last-query preservation across open/close.
 *
 * Requirements: 3.1, 3.7, 3.9, 3.10
 */

import React, { useEffect, useRef, useCallback } from 'react'
import type { SearchQueryResult, SearchQueryMatch } from '@shared/search-query'
import { useAppContext } from '../App'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Render a snippet line with the matching portion wrapped in `<mark>`.
 * Splits the snippet at [startCol, endCol) and wraps the match segment.
 */
function HighlightedSnippet({ match }: { match: SearchQueryMatch }): React.JSX.Element {
  const { snippet, startCol, endCol } = match

  if (startCol >= snippet.length || endCol <= 0 || startCol >= endCol) {
    // No valid highlight range — render the snippet as-is
    return <>{snippet}</>
  }

  const before = snippet.slice(0, Math.max(0, startCol))
  const highlighted = snippet.slice(Math.max(0, startCol), Math.min(endCol, snippet.length))
  const after = snippet.slice(Math.min(endCol, snippet.length))

  return (
    <>
      {before}
      <mark className="search-panel__mark">{highlighted}</mark>
      {after}
    </>
  )
}

// ---------------------------------------------------------------------------
// Chevron icon (shared with ContextPane)
// ---------------------------------------------------------------------------

function ChevronIcon(): React.JSX.Element {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <polyline points="9 18 15 12 9 6" />
    </svg>
  )
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface SearchPanelProps {
  query: string
  results: SearchQueryResult[]
  onQueryChange: (query: string) => void
  onResultsChange: (results: SearchQueryResult[]) => void
  onClose: () => void
}

// ---------------------------------------------------------------------------
// SearchPanel
// ---------------------------------------------------------------------------

export function SearchPanel({
  query,
  results,
  onQueryChange,
  onResultsChange,
  onClose
}: SearchPanelProps): React.JSX.Element {
  const { dispatch } = useAppContext()

  // --- Local state ---
  const [selectedIndex, setSelectedIndex] = React.useState(0)
  const [loading, setLoading] = React.useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const activeRef = useRef<HTMLDivElement>(null)

  // Auto-focus the input when the panel opens
  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  // If a query was pre-set (e.g. from property search), trigger search immediately
  useEffect(() => {
    if (query.trim()) {
      performSearch(query)
    }
    // Only run on mount — query changes via input are handled by the debounce
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Scroll the selected result into view
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  // --- Debounced search ---
  const performSearch = useCallback(
    async (q: string) => {
      if (!q.trim()) {
        onResultsChange([])
        setLoading(false)
        return
      }

      setLoading(true)
      try {
        const response = (await window.electron.search.query(q)) as { results: SearchQueryResult[] }
        onResultsChange(response.results ?? [])
      } catch (err) {
        console.error('[SearchPanel] search query failed:', err)
        onResultsChange([])
      } finally {
        setLoading(false)
      }
    },
    [onResultsChange]
  )

  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value
      onQueryChange(value)
      setSelectedIndex(0)

      if (debounceRef.current) clearTimeout(debounceRef.current)
      debounceRef.current = setTimeout(() => {
        performSearch(value)
      }, 200)
    },
    [onQueryChange, performSearch]
  )

  // --- Open a result ---
  const openResult = useCallback(
    (filePath: string) => {
      window.electron.file
        .get(filePath)
        .then((fileAST) => {
          dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
        })
        .catch((err) => {
          console.error('[SearchPanel] failed to open result:', err)
        })
      onClose()
    },
    [dispatch, onClose]
  )

  // --- Keyboard navigation ---
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault()
          setSelectedIndex((prev) => Math.min(prev + 1, results.length - 1))
          break
        case 'ArrowUp':
          e.preventDefault()
          setSelectedIndex((prev) => Math.max(prev - 1, 0))
          break
        case 'Enter':
          e.preventDefault()
          if (results[selectedIndex]) {
            openResult(results[selectedIndex].filePath)
          }
          break
        case 'Escape':
          e.preventDefault()
          onClose()
          break
      }
    },
    [results, selectedIndex, onClose, openResult]
  )

  // Cleanup debounce on unmount
  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [])

  // --- Render ---
  const hasQuery = query.trim().length > 0
  const hasResults = results.length > 0

  return (
    <div className="search-panel" role="dialog" aria-label="Search notes">
      {/* Header with close button */}
      <div className="search-panel__header">
        <h2 className="search-panel__title">Search</h2>
        <button
          className="search-panel__close"
          onClick={onClose}
          aria-label="Close search panel"
          title="Close (Esc)"
        >
          <ChevronIcon />
        </button>
      </div>

      {/* Search input */}
      <div className="search-panel__input-wrapper">
        <input
          ref={inputRef}
          type="text"
          className="search-panel__input"
          placeholder="Search…  e.g. tag:dev content:api"
          value={query}
          onChange={handleInputChange}
          onKeyDown={handleKeyDown}
          aria-label="Search query"
          autoComplete="off"
          spellCheck={false}
        />
        {loading && <span className="search-panel__spinner" aria-hidden="true" />}
      </div>

      {/* Results */}
      <div className="search-panel__results" role="listbox" aria-label="Search results">
        {!hasQuery && (
          <div className="search-panel__empty">
            <p>Type a query to search notes</p>
            <p className="search-panel__hint">
              Operators: <code>path:</code> <code>tag:</code> <code>line:</code>{' '}
              <code>content:</code> <code>file:</code> <code>property:</code> <code>regex:</code>
            </p>
          </div>
        )}

        {hasQuery && !hasResults && !loading && (
          <div className="search-panel__empty">
            <p>
              No results found for <strong>{query}</strong>
            </p>
          </div>
        )}

        {hasResults &&
          results.map((result, idx) => {
            const firstMatch = result.matches[0]
            const isSelected = idx === selectedIndex

            return (
              <div
                key={result.filePath}
                ref={isSelected ? activeRef : undefined}
                className={`search-panel__result${isSelected ? ' search-panel__result--selected' : ''}`}
                role="option"
                aria-selected={isSelected}
                onClick={() => openResult(result.filePath)}
                onMouseEnter={() => setSelectedIndex(idx)}
              >
                <div className="search-panel__result-header">
                  <span className="search-panel__result-name">{result.name}</span>
                  <span className="search-panel__result-path">{result.relativePath}</span>
                </div>
                {firstMatch && (
                  <div className="search-panel__result-snippet">
                    <HighlightedSnippet match={firstMatch} />
                  </div>
                )}
                {result.matches.length > 1 && (
                  <span className="search-panel__result-count">
                    {result.matches.length} matches
                  </span>
                )}
              </div>
            )
          })}
      </div>
    </div>
  )
}

export default SearchPanel
