/**
 * MermaidBlock.tsx
 *
 * Lazy-loads the mermaid library and renders a diagram SVG inline.
 * Provides loading, error, and ready states. Re-renders when the
 * theme or source changes.
 *
 * Requirements: 10.1 – 10.6
 */

import React, { useEffect, useRef, useState } from 'react'
import type { MermaidConfig } from 'mermaid'
import { useAppContext } from '../../../shared/store'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MermaidBlockProps {
  /** The mermaid diagram source text (code block content). */
  value: string
}

type DiagramState = 'loading' | 'ready' | 'error'

// ---------------------------------------------------------------------------
// Module-level cache for the mermaid module (lazy loaded once)
// ---------------------------------------------------------------------------

type MermaidModule = {
  initialize: (config: MermaidConfig) => void
  parse: (text: string, options?: { suppressErrors?: boolean }) => Promise<unknown>
  render: (id: string, text: string) => Promise<{ svg: string }>
}

let mermaidInstance: MermaidModule | null = null
let mermaidTheme: 'dark' | 'default' | null = null

/** Ensure mermaid is loaded and initialised with the correct theme. */
async function ensureMermaid(theme: 'dark' | 'default'): Promise<MermaidModule> {
  if (!mermaidInstance) {
    const mod = await import('mermaid')
    mermaidInstance = mod.default as unknown as MermaidModule
  }

  // Re-initialise when the theme changes
  if (mermaidTheme !== theme) {
    mermaidInstance.initialize({
      startOnLoad: false,
      theme
    })
    mermaidTheme = theme
  }

  return mermaidInstance
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MermaidBlock({ value }: MermaidBlockProps): React.JSX.Element {
  const { state } = useAppContext()
  const [diagramState, setDiagramState] = useState<DiagramState>('loading')
  const [svgContent, setSvgContent] = useState<string>('')
  const [error, setError] = useState<string>('')

  // A stable ref so effect-cleanup sees the latest value without re-running
  const diagramIdRef = useRef<string>('mermaid-' + Math.random().toString(36).slice(2, 10))

  // Derive the mermaid theme from the app theme
  const mermaidThemeValue: 'dark' | 'default' =
    state.theme === 'dark' ||
    (state.theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
      ? 'dark'
      : 'default'

  useEffect(() => {
    let cancelled = false
    setDiagramState('loading')
    setError('')

    const renderDiagram = async (): Promise<void> => {
      try {
        const mermaid = await ensureMermaid(mermaidThemeValue)

        if (cancelled) return

        // Validate first — throws on invalid syntax
        await mermaid.parse(value, { suppressErrors: false })

        // Render
        const { svg } = await mermaid.render(diagramIdRef.current, value)

        if (cancelled) return
        setSvgContent(svg)
        setDiagramState('ready')
      } catch (err) {
        if (cancelled) return
        setError(err instanceof Error ? err.message : 'Failed to render diagram')
        setDiagramState('error')
      }
    }

    renderDiagram()

    return () => {
      cancelled = true
    }
  }, [value, mermaidThemeValue])

  // ---- Loading state ----
  if (diagramState === 'loading') {
    return (
      <div
        className="my-3 rounded-lg bg-white/5 p-6 animate-pulse"
        aria-busy="true"
        aria-label="Rendering diagram…"
      >
        <div className="h-32 w-full rounded bg-white/10" />
      </div>
    )
  }

  // ---- Error state ----
  if (diagramState === 'error') {
    return (
      <div className="my-3 rounded-lg border-l-4 border-l-red-500 bg-red-950/20 p-4">
        <p className="text-xs font-semibold text-red-400 mb-1">Mermaid diagram error</p>
        <p className="text-xs text-red-400/80 font-mono mb-2 whitespace-pre-wrap break-all">
          {error}
        </p>
        <pre className="text-xs text-white/70 font-mono bg-white/5 p-2 rounded overflow-x-auto">
          {value}
        </pre>
      </div>
    )
  }

  // ---- Ready state ----
  return (
    <div
      className="my-3 flex justify-center rounded-lg bg-white/5 p-4 overflow-x-auto"
      aria-label="Mermaid diagram"
    >
      <div dangerouslySetInnerHTML={{ __html: svgContent }} />
    </div>
  )
}

export default MermaidBlock
