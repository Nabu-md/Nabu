/**
 * CytoscapeGraphView.tsx
 *
 * Cytoscape-based graph view for Nabu (Phase 4).
 * Uses cose-bilkent layout for clustered graph visualization.
 *
 * Requirements: Phase 4 (Cytoscape Graph View)
 */

import React, { useEffect, useRef, useState, useCallback } from 'react'
// eslint-disable-next-line @typescript-eslint/no-require-imports
const cytoscape = require('cytoscape')
// eslint-disable-next-line @typescript-eslint/no-require-imports
const layout = require('cose-bilkent')
import type { Edge, FileEntry } from '@shared/types'

// Register the layout extension
cytoscape.use(layout)

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CytoscapeGraphViewProps {
  files: FileEntry[]
  edges: Edge[]
  currentFile?: string | null
  searchQuery?: string
  mode?: 'global' | 'local'
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CytoscapeGraphView({
  files,
  edges,
  currentFile,
  searchQuery = '',
  mode
}: CytoscapeGraphViewProps): React.JSX.Element {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  void mode // For future use with local/global filtering
  const containerRef = useRef<HTMLDivElement>(null)
  const cyRef = useRef<ReturnType<typeof cytoscape> | null>(null)

  const [layoutMode, setLayoutMode] = useState<'cluster' | 'force'>('cluster')

  // Initialize cytoscape on mount
  useEffect(() => {
    if (!containerRef.current) return

    const cy = cytoscape({
      container: containerRef.current,
      elements: [],
      style: [
        {
          selector: 'node',
          style: {
            'background-color': '#60a5fa',
            'label': 'data(label)',
            'color': '#e5e5e5',
            'font-size': '10px',
            'shape': 'ellipse',
            'width': 40,
            'height': 30,
            'text-wrap': 'wrap',
            'text-max-width': 80,
            'text-outline-width': 1,
            'text-outline-color': '#0a0a0a'
          }
        },
        {
          selector: 'edge',
          style: {
            'line-color': '#2a2a2a',
            'curve-style': 'bezier',
            'target-arrow-shape': 'triangle',
            'target-arrow-color': '#2a2a2a',
            'opacity': 0.3
          }
        },
        {
          selector: '.highlighted',
          style: {
            'background-color': '#60a5fa',
            'line-color': '#60a5fa',
            'opacity': 1
          }
        }
      ],
      layout: { name: 'cose-bilkent' }
    })

    cyRef.current = cy

    return () => {
      cy.destroy()
      cyRef.current = null
    }
  }, [])

  // Update graph data when files/edges change
  useEffect(() => {
    const cy = cyRef.current
    if (!cy) return

    // Build elements
    const elements = [
      ...files.map((f) => ({
        data: { id: f.path, label: f.name.replace('.md', '') },
        classes: f.path === currentFile ? 'highlighted' : ''
      })),
      ...edges.map((e, i) => ({
        data: {
          id: `edge-${i}`,
          source: e.source,
          target: e.target,
          snippet: e.snippet
        }
      }))
    ]

    cy.elements().remove()
    cy.add(elements)

    // Apply layout
    cy.layout({
      name: layoutMode === 'cluster' ? 'cose-bilkent' : 'cola',
      animate: true,
      animationDuration: 300,
      padding: 20
    }).run()
  }, [files, edges, layoutMode, currentFile])

  // Handle node clicks
  useEffect(() => {
    const cy = cyRef.current
    if (!cy) return

    const handleClick = (evt: { target: ReturnType<typeof cytoscape> }): void => {
      const node = evt.target
      if (node.isNode()) {
        const filePath = node.id()
        window.electron?.file?.get(filePath).catch(console.error)
      }
    }

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    cy.on('tap', handleClick as any)

    return () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      cy.off('tap', handleClick as any)
    }
  }, [])

  const toggleLayout = useCallback(() => {
    const cy = cyRef.current
    if (!cy) return

    setLayoutMode((prev) => {
      const next = prev === 'cluster' ? 'force' : 'cluster'
      cy.layout({
        name: next === 'cluster' ? 'cose-bilkent' : 'cola',
        animate: true,
        animationDuration: 500
      }).run()
      return next
    })
  }, [])

  return (
    <div className="cytoscape-graph-view flex flex-col h-full" aria-label="Graph view (cytoscape)">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-nabu-border">
        <input
          type="text"
          placeholder="Search nodes…"
          value={searchQuery}
          onChange={() => {}} // Handled by parent
          className="flex-1 px-2 py-1 text-xs rounded bg-nabu-bg border border-nabu-border outline-none"
        />
        <button
          type="button"
          onClick={toggleLayout}
          className="px-2 py-1 text-xs rounded bg-nabu-bg border border-nabu-border hover:bg-nabu-bg-muted transition-colors"
          title="Toggle layout"
        >
          {layoutMode === 'cluster' ? 'Clustered' : 'Force'}
        </button>
      </div>
      <div ref={containerRef} className="flex-1" />
    </div>
  )
}