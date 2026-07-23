/**
 * InlineTagChip.tsx
 *
 * Renders clickable inline #tag chips inside note text.  Tags are detected
 * using the same regex as the indexing layer so that rendered tags match
 * the tag index exactly.
 *
 * Requirements: 14.1, 14.4
 */

import React, { useCallback } from 'react'
import type { Text } from 'mdast'
import { INLINE_TAG_RE } from '@shared/extended-indexing'
import { useAppContext } from '../../../shared/store'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface InlineTagChipProps {
  tag: string
}

/**
 * A single clickable #tag chip rendered inline in note text.
 */
function InlineTagChip({ tag }: InlineTagChipProps): React.JSX.Element {
  const { dispatch } = useAppContext()

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      dispatch({ type: 'TAG_FILTER_TOGGLE', payload: tag })
    },
    [dispatch, tag]
  )

  return (
    <span
      role="button"
      tabIndex={0}
      onClick={handleClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          dispatch({ type: 'TAG_FILTER_TOGGLE', payload: tag })
        }
      }}
      className="inline-tag-chip cursor-pointer text-blue-400/80 hover:text-blue-300 hover:underline transition-colors"
      title={`Filter by #${tag}`}
      aria-label={`Tag: ${tag}`}
    >
      #{tag}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Render helper
// ---------------------------------------------------------------------------

/**
 * Render a text node, splitting it into plain segments and clickable #tag chips.
 *
 * Only text nodes that are NOT inside inline code or code blocks are processed;
 * the parent renderer already handles `inlineCode` nodes separately (Req 14.4).
 */
export function renderInlineTagText(node: Text): React.ReactNode {
  const text = node.value
  if (!text) return text

  // Fast path: no # in text → skip regex entirely
  if (!text.includes('#')) return text

  const parts: React.ReactNode[] = []
  let lastIndex = 0
  let match: RegExpExecArray | null

  // Reset regex state (global regex)
  INLINE_TAG_RE.lastIndex = 0

  while ((match = INLINE_TAG_RE.exec(text)) !== null) {
    // Plain text before the tag
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index))
    }

    const rawTag = match[1] // includes the leading #
    const tag = rawTag.slice(1) // strip #

    parts.push(<InlineTagChip key={`tag-${match.index}`} tag={tag} />)

    lastIndex = INLINE_TAG_RE.lastIndex
  }

  // Remaining plain text after last tag
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex))
  }

  // If no tags were found (shouldn't happen since we checked text.includes('#')), return original
  if (parts.length === 0) return text

  return parts.length === 1 ? parts[0] : <>{parts}</>
}
