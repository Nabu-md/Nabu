/**
 * MermaidBlock.tsx
 *
 * Lazy-loads the mermaid library and renders a diagram SVG inline.
 * Provides loading, error, and ready states. Re-renders when the
 * theme or source changes.
 *
 * Requirements: 10.1 – 10.6
 */

import React, { useEffect, useState } from 'react'
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
  if (mermaidInstance && mermaidTheme === theme) return mermaidInstance

  const mermaid = (await import('mermaid')).default
  mermaid.initialize({
    startOnLoad: false,
    theme: theme === 'dark' ? 'dark' : 'default',
    fontFamily: 'inherit'
  })
  mermaidInstance = mermaid
  mermaidTheme = theme
  return mermaid
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MermaidBlock({ value }: MermaidBlockProps): React.JSX.Element {
  const { state } = useAppContext()
  const [stateType, setStateType] = useState<DiagramState>('loading')
  const [svg, setSvg] = useState<string>('')
  
  // Stable ID for the mermaid diagram
  const [diagramId] = useState(() => `mermaid-${crypto.randomUUID().slice(0, 8)}`)

  const theme = state.theme === 'dark' ? 'dark' : 'default'

  useEffect(() => {
    let cancelled = false
    setStateType('loading')

    async function render(): Promise<void> {
      try {
        const mermaid = await ensureMermaid(theme)
        const { svg: renderedSvg } = await mermaid.render(diagramId, value)
        if (!cancelled) {
          setSvg(renderedSvg)
          setStateType('ready')
        }
      } catch (err) {
        console.error('[MermaidBlock] render failed:', err)
        if (!cancelled) setStateType('error')
      }
    }

    void render()
    return () => {
      cancelled = true
    }
  }, [value, theme, diagramId])

  if (stateType === 'loading') {
    return <div className="text-sm text-nabu-text-muted animate-pulse">Rendering diagram…</div>
  }

  if (stateType === 'error') {
    return <div className="text-sm text-red-400">Failed to render diagram</div>
  }

  return <div className="mermaid-container" dangerouslySetInnerHTML={{ __html: svg }} />
}

export default MermaidBlock
