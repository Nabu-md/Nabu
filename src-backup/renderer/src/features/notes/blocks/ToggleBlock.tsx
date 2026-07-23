import React, { useContext } from 'react'
import { Node } from 'mdast'
import { ToggleBlock as ToggleBlockNode } from '@shared/types'
import { AppContext } from '../../../shared/store'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface ToggleBlockProps {
  node: ToggleBlockNode
  filePath: string
  renderNodes?: (nodes: Node[], filePath: string) => React.ReactNode
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Extracts plain text from a heading's children (phrasing content).
 * Recursively walks children to collect `text` node values.
 */
function extractHeadingText(node: ToggleBlockNode['heading']): string {
  function walk(children: unknown[]): string {
    return children
      .map((child) => {
        if (typeof child !== 'object' || child === null) return ''
        const c = child as Record<string, unknown>
        if (typeof c['value'] === 'string') return c['value']
        if (Array.isArray(c['children'])) return walk(c['children'] as unknown[])
        return ''
      })
      .join('')
  }
  return walk(node.children as unknown[])
}

/**
 * Converts a heading text string into a URL-safe slug for use as a headingId.
 * Falls back to a truncated hex hash of the string for unusual characters.
 *
 * Example: "## [toggle] My Section" → "toggle-my-section"
 */
function slugify(text: string): string {
  const cleaned = text
    // Remove [toggle] prefix (case-insensitive) from heading text
    .replace(/^\s*\[toggle\]\s*/i, '')
    .toLowerCase()
    .trim()
    // Replace non-alphanumeric characters with hyphens
    .replace(/[^a-z0-9]+/g, '-')
    // Strip leading/trailing hyphens
    .replace(/^-+|-+$/g, '')

  // If the result is empty (e.g. purely non-ASCII heading), use a fallback
  if (!cleaned) {
    // Simple position-based fallback using char codes
    let hash = 0
    for (let i = 0; i < text.length; i++) {
      hash = (hash * 31 + text.charCodeAt(i)) >>> 0
    }
    return `heading-${hash.toString(16)}`
  }

  return cleaned
}

// ---------------------------------------------------------------------------
// CSS
// ---------------------------------------------------------------------------
// The animation is handled by inline style toggling grid-template-rows.
// We inject minimal CSS once via a <style> tag (idempotent via data attribute).

const STYLE_ID = 'toggle-block-styles'

function ensureStyles(): void {
  if (typeof document !== 'undefined' && !document.getElementById(STYLE_ID)) {
    const style = document.createElement('style')
    style.id = STYLE_ID
    style.textContent = `
.toggle-content {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 150ms ease-out;
}
.toggle-content.open {
  grid-template-rows: 1fr;
}
.toggle-content > .toggle-content-inner {
  overflow: hidden;
  min-height: 0;
}
`
    document.head.appendChild(style)
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ToggleBlock({ node, filePath, renderNodes }: ToggleBlockProps): React.JSX.Element {
  // Inject CSS once on first render
  React.useLayoutEffect(() => {
    ensureStyles()
  }, [])

  const { state, dispatch } = useContext(AppContext)

  // Derive a stable headingId from the heading's text content
  const headingText = extractHeadingText(node.heading)
  const headingId = slugify(headingText) || `heading-${node.heading.depth}`

  // Read current open/closed state from AppContext (defaults to false = collapsed)
  const isOpen = state.toggleStates.get(filePath)?.get(headingId) ?? false

  const handleToggle = (): void => {
    dispatch({
      type: 'TOGGLE_BLOCK',
      payload: { filePath, headingId, isOpen: !isOpen }
    })
  }

  // Render the heading text without the [toggle] prefix
  const displayText = headingText.replace(/^\s*\[toggle\]\s*/i, '').trim() || 'Toggle'

  // Map heading depth to appropriate heading element classes
  const depthClassMap: Record<number, string> = {
    1: 'text-2xl font-bold text-white/90',
    2: 'text-xl font-semibold text-white/85',
    3: 'text-lg font-semibold text-white/80',
    4: 'text-base font-semibold text-white/75',
    5: 'text-sm font-semibold text-white/70',
    6: 'text-xs font-semibold text-white/65'
  }
  const headingClass = depthClassMap[node.heading.depth] ?? 'font-semibold text-white/80'

  return (
    <div className="toggle-block my-2" data-heading-id={headingId}>
      {/* Toggle button — acts as the heading */}
      <button
        type="button"
        onClick={handleToggle}
        className={`flex items-center gap-2 text-left w-full rounded px-1 py-0.5
                    hover:bg-white/5 transition-colors cursor-pointer select-none
                    focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/30
                    ${headingClass}`}
        aria-expanded={isOpen}
        aria-controls={`toggle-content-${headingId}`}
      >
        {/* Chevron indicator */}
        <span
          className="text-white/50 text-xs flex-shrink-0 transition-transform duration-150"
          style={{ transform: isOpen ? 'rotate(90deg)' : 'rotate(0deg)' }}
          aria-hidden="true"
        >
          ▶
        </span>
        <span>{displayText}</span>
      </button>

      {/* Animated collapsible content */}
      <div
        id={`toggle-content-${headingId}`}
        className={`toggle-content${isOpen ? ' open' : ''}`}
        aria-hidden={!isOpen}
      >
        <div className="toggle-content-inner pl-4">
          {node.children.length > 0 && renderNodes ? renderNodes(node.children, filePath) : null}
        </div>
      </div>
    </div>
  )
}
