import React, { useEffect } from 'react'
import { Node, Root, Text } from 'mdast'
import { useAppContext } from '../App'

// ---------------------------------------------------------------------------
// Plain-text extraction from mdast
// ---------------------------------------------------------------------------

/** Recursively extract plain text from an mdast node tree. */
function extractPlainText(node: Node): string {
  if (node.type === 'text') {
    return (node as Text).value
  }

  const withChildren = node as unknown as { children?: Node[] }
  if (Array.isArray(withChildren.children)) {
    return withChildren.children.map(extractPlainText).join(' ')
  }

  return ''
}

function astToPlainText(ast: Root): string {
  return ast.children
    .map(extractPlainText)
    .join('\n')
    .trim()
}

// ---------------------------------------------------------------------------
// Chevron icon
// ---------------------------------------------------------------------------

interface ChevronProps {
  direction: 'up' | 'down'
}

function ChevronIcon({ direction }: ChevronProps): React.JSX.Element {
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
      style={{
        transform: direction === 'down' ? 'rotate(180deg)' : undefined,
        transition: 'transform 150ms ease-out'
      }}
    >
      <polyline points="18 15 12 9 6 15" />
    </svg>
  )
}

// ---------------------------------------------------------------------------
// ContextPane
// ---------------------------------------------------------------------------

export function ContextPane(): React.JSX.Element {
  const { state, dispatch } = useAppContext()
  const { contextPaneOpen, currentFile, currentAST, contextResults, vectorDisabled, vectorDisabledReason } = state

  // ---- Trigger context query when the current file changes ----
  useEffect(() => {
    if (!currentFile) return

    const text = currentAST ? astToPlainText(currentAST) : currentFile
    if (!text) return

    window.electron.context.query(text).catch((err: unknown) => {
      console.error('[ContextPane] context.query failed:', err)
    })
  }, [currentFile, currentAST])

  // ---- Toggle handler ----
  function handleHeaderClick(): void {
    dispatch({ type: 'CONTEXT_PANE_TOGGLE' })
  }

  // ---- Derive file basename from path ----
  function basename(filePath: string): string {
    return filePath.split(/[\\/]/).pop() ?? filePath
  }

  return (
    <div
      className={`context-pane${contextPaneOpen ? ' context-pane--open' : ''}`}
      aria-label="Context pane"
    >
      {/* Header — always visible, outside the collapsible inner */}
      <header
        className="flex items-center justify-between px-4 py-2 cursor-pointer select-none border-b border-white/10 hover:bg-white/[0.03] transition-colors"
        onClick={handleHeaderClick}
        aria-expanded={contextPaneOpen}
        aria-controls="context-pane-results"
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            handleHeaderClick()
          }
        }}
      >
        <span className="text-xs font-semibold text-white/60 uppercase tracking-wide">
          Related Notes
        </span>
        <ChevronIcon direction={contextPaneOpen ? 'down' : 'up'} />
      </header>

      {/* Collapsible results — inside the grid-row animated inner */}
      <div className="context-pane__inner">
        {contextPaneOpen && (
          <div
            id="context-pane-results"
            className="overflow-y-auto px-4 py-2 space-y-1"
            style={{ minHeight: '80px' }}
            aria-label="Context search results"
          >
            {/* Non-blocking notice when the vector model failed to load (Req 1.4) */}
            {vectorDisabled && (
              <p
                className="text-xs text-amber-400/80 py-2 select-none leading-relaxed"
                role="status"
              >
                Semantic search unavailable: {vectorDisabledReason ?? 'Embedding model not loaded'}
              </p>
            )}

            {!vectorDisabled && contextResults.length === 0 ? (
              <p className="text-xs text-white/30 py-2 select-none">No related notes found</p>
            ) : (
              contextResults.map((result) => (
                <div
                  key={result.path}
                  className="flex items-center justify-between gap-3 px-2 py-1.5 rounded hover:bg-white/[0.05] transition-colors"
                  role="listitem"
                >
                  <span
                    className="text-xs text-white/80 font-medium truncate flex-1"
                    title={result.path}
                  >
                    {basename(result.path)}
                  </span>
                  <span className="text-xs text-nabu-accent font-mono shrink-0">
                    {result.score.toFixed(2)}
                  </span>
                  <span className="text-xs text-white/40 shrink-0">
                    {result.tokenCount} tokens
                  </span>
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  )
}
