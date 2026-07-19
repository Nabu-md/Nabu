/**
 * FindReplaceBar.tsx
 *
 * Collapsible find/replace bar for the Markdown editor.
 * Supports Cmd+H / Ctrl+H keyboard shortcut.
 *
 * Requirements: Phase 0b
 */

import React, { useState, useCallback, useEffect, useRef } from 'react'

interface FindReplaceBarProps {
  value: string
  onReplace: (find: string, replace: string, replaceAll?: boolean) => void
  onClose: () => void
  onHighlightMatches?: (matches: number, currentIndex: number) => void
}

export function FindReplaceBar({ value, onReplace, onClose, onHighlightMatches }: FindReplaceBarProps): React.JSX.Element {
  const [findText, setFindText] = useState('')
  const [replaceText, setReplaceText] = useState('')
  const [matchCount, setMatchCount] = useState(0)
  const [currentIndex, setCurrentIndex] = useState(0)
  const findInputRef = useRef<HTMLInputElement>(null)

  // Count matches and notify parent
  useEffect(() => {
    if (findText.trim() === '') {
      setMatchCount(0)
      setCurrentIndex(0)
      onHighlightMatches?.(0, 0)
      return
    }

    const regex = new RegExp(escapeRegExp(findText), 'gi')
    const matches = value.match(regex)
    setMatchCount(matches?.length ?? 0)
    setCurrentIndex(0)
    onHighlightMatches?.(matches?.length ?? 0, 0)
  }, [findText, value, onHighlightMatches])

  // Auto-focus find input on mount
  useEffect(() => {
    findInputRef.current?.focus()
  }, [])

  const escapeRegExp = (string: string): string => {
    return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  }

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (e.shiftKey) {
        // Previous match
        setCurrentIndex((i) => {
          const newIndex = i <= 1 ? matchCount : i - 1
          onHighlightMatches?.(matchCount, newIndex)
          return newIndex
        })
      } else {
        // Next match
        setCurrentIndex((i) => {
          const newIndex = i >= matchCount ? 1 : i + 1
          onHighlightMatches?.(matchCount, newIndex)
          return newIndex
        })
      }
    }
  }, [findText, matchCount, onHighlightMatches, onClose])

  const handleReplace = useCallback(() => {
    if (findText.trim() === '') return
    onReplace(findText, replaceText, false)
  }, [findText, replaceText, onReplace])

  const handleReplaceAll = useCallback(() => {
    if (findText.trim() === '') return
    onReplace(findText, replaceText, true)
    setMatchCount(0)
    setCurrentIndex(0)
  }, [findText, replaceText, onReplace])

  return (
    <div className="find-replace-bar flex items-center gap-2 px-4 py-2 bg-nabu-bg-mute border-b border-nabu-border" role="toolbar">
      <div className="flex items-center gap-1">
        <input
          ref={findInputRef}
          type="text"
          placeholder="Find…"
          value={findText}
          onChange={(e) => setFindText(e.target.value)}
          onKeyDown={handleKeyDown}
          className="px-2 py-1 text-sm rounded bg-nabu-bg border border-nabu-border text-nabu-text outline-none focus:border-nabu-accent transition-colors"
          aria-label="Find text"
        />
        <input
          type="text"
          placeholder="Replace…"
          value={replaceText}
          onChange={(e) => setReplaceText(e.target.value)}
          onKeyDown={handleKeyDown}
          className="px-2 py-1 text-sm rounded bg-nabu-bg border border-nabu-border text-nabu-text outline-none focus:border-nabu-accent transition-colors"
          aria-label="Replace text"
        />
      </div>

      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={handleReplace}
          disabled={matchCount === 0}
          className="px-2 py-1 text-xs rounded bg-nabu-bg border border-nabu-border text-nabu-text hover:bg-nabu-bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          aria-label="Replace current match"
        >
          Replace
        </button>
        <button
          type="button"
          onClick={handleReplaceAll}
          disabled={matchCount === 0}
          className="px-2 py-1 text-xs rounded bg-nabu-bg border border-nabu-border text-nabu-text hover:bg-nabu-bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          aria-label="Replace all matches"
        >
          Replace All
        </button>
        <div className="text-xs text-nabu-text-muted px-1">
          {matchCount > 0 ? `${currentIndex} of ${matchCount}` : 'No matches'}
        </div>
      </div>

      <button
        type="button"
        onClick={onClose}
        className="ml-auto px-2 py-1 text-xs rounded hover:bg-nabu-bg-muted text-nabu-text-muted hover:text-nabu-text transition-colors"
        aria-label="Close find/replace"
      >
        ✕
      </button>
    </div>
  )
}