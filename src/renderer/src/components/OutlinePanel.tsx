/**
 * OutlinePanel.tsx
 *
 * Sidebar panel showing the heading hierarchy of the current note.
 * Tracks the active heading via IntersectionObserver and scrolls
 * to headings on click.
 *
 * Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6
 */

import React, { useEffect, useMemo, useState, useCallback, useRef } from 'react'
import { useAppContext } from '../App'
import type { Root, Heading } from 'mdast'
// Parent type from unist - used for AST type checking

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface OutlineEntry {
  /** Index of the heading in the AST's children array — used as the element id suffix. */
  childIndex: number
  depth: number
  text: string
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Flatten phrasing children into a plain text string. */
function flattenText(node: unknown): string {
  if (typeof node === 'string') return node
  if (node && typeof node === 'object') {
    const n = node as { value?: string; children?: unknown[] }
    if (n.value) return n.value
    if (n.children) return n.children.map(flattenText).join('')
  }
  return ''
}

/** Extract heading entries from a Root AST. */
function extractOutline(ast: Root | null): OutlineEntry[] {
  if (!ast) return []

  const entries: OutlineEntry[] = []

  for (let i = 0; i < ast.children.length; i++) {
    const child = ast.children[i]
    if (child.type === 'heading') {
      const heading = child as Heading
      entries.push({
        childIndex: i,
        depth: heading.depth,
        text: flattenText(heading)
      })
    }
  }

  return entries
}

// ---------------------------------------------------------------------------
// OutlinePanel
// ---------------------------------------------------------------------------

export function OutlinePanel(): React.JSX.Element | null {
  const { state } = useAppContext()
  const [isExpanded, setIsExpanded] = useState(true)
  const [activeIndex, setActiveIndex] = useState<number | null>(null)
  const listRef = useRef<HTMLUListElement>(null)

  // Build outline from current AST (Req 7.1, 7.2).
  const entries = useMemo(() => extractOutline(state.currentAST), [state.currentAST])

  // IntersectionObserver — track the heading currently in view (Req 7.4).
  useEffect(() => {
    setActiveIndex(null)

    const elements: Element[] = []
    for (const entry of entries) {
      const el = document.getElementById(`outline-heading-${entry.childIndex}`)
      if (el) elements.push(el)
    }

    if (elements.length === 0) return

    const observer = new IntersectionObserver(
      (observed) => {
        // Find the first heading that is intersecting or closest to the top.
        let active: number | null = null
        for (const o of observed) {
          if (o.isIntersecting) {
            const id = o.target.id
            const match = id.match(/outline-heading-(\d+)/)
            if (match) {
              const idx = parseInt(match[1], 10)
              if (active === null || idx < active) {
                active = idx
              }
            }
          }
        }
        // Fall back to the first visible one above the viewport if none is intersecting.
        if (active === null) {
          for (const o of observed) {
            if (o.boundingClientRect.top > 0) {
              const id = o.target.id
              const match = id.match(/outline-heading-(\d+)/)
              if (match) {
                const idx = parseInt(match[1], 10)
                if (active === null || idx < active) {
                  active = idx
                }
              }
            }
          }
        }
        setActiveIndex(active)
      },
      { rootMargin: '-80px 0px -60% 0px' }
    )

    for (const el of elements) observer.observe(el)
    return () => observer.disconnect()
  }, [entries])

  // Scroll to a heading on click (Req 7.3).
  const scrollToHeading = useCallback((childIndex: number) => {
    const el = document.getElementById(`outline-heading-${childIndex}`)
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'start' })
    }
  }, [])

  // Empty state (Req 7.5).
  if (entries.length === 0) return null

  return (
    <section className="outline-panel border-t border-white/10 mt-2 pt-2" aria-label="Outline">
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded((prev) => !prev)}
        className="flex items-center gap-2 text-xs font-semibold text-white/40 hover:text-white/70 transition-colors w-full text-left px-3 py-1.5"
      >
        <span>Outline</span>
        <span aria-hidden="true" className="text-[10px]">
          {isExpanded ? '▲' : '▼'}
        </span>
      </button>
      {isExpanded && (
        <ul ref={listRef} role="list" className="space-y-0.5 px-2 pb-2">
          {entries.map((entry) => {
            const isActive = activeIndex === entry.childIndex
            return (
              <li key={entry.childIndex}>
                <button
                  type="button"
                  onClick={() => scrollToHeading(entry.childIndex)}
                  onMouseEnter={() => setActiveIndex(entry.childIndex)}
                  className={`w-full text-left rounded px-2 py-1 text-xs transition-colors truncate ${
                    isActive
                      ? 'text-nabu-accent bg-white/10'
                      : 'text-white/50 hover:text-white/80 hover:bg-white/8'
                  }`}
                  style={{ paddingLeft: `${8 + (entry.depth - 1) * 12}px` }}
                >
                  {entry.text || '(untitled)'}
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

export default OutlinePanel
