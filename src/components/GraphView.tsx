import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { MagnifyingGlass, X, Crosshair } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { translate, type AppLocale } from '../lib/i18n'
import type { VaultEntry } from '../types'
import { buildGraphData, useGraphData, type GraphNode } from '../hooks/useGraphData'

export interface GraphViewProps {
  entries: VaultEntry[]
  onSelectNote: (entry: VaultEntry) => void
  onClose: () => void
  locale?: AppLocale
}

type VisNetwork = {
  destroy: () => void
  fit: (options?: { animation?: boolean }) => void
  on: (event: string, handler: (params: unknown) => void) => void
}

type VisNetworkModule = {
  Network: new (
    container: HTMLElement,
    data: {
      nodes: Array<{ id: string; label: string; value?: number; group?: string; title?: string }>
      edges: Array<{ from: string; to: string }>
    },
    options: Record<string, unknown>,
  ) => VisNetwork
}

function readCssColor(name: string, fallback: string): string {
  if (typeof window === 'undefined' || typeof document === 'undefined') return fallback
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

function nodeSize(degree: number): number {
  return 8 + Math.min(degree, 20) * 2
}

function filterNodes(nodes: GraphNode[], query: string): GraphNode[] {
  if (!query.trim()) return nodes
  const lower = query.trim().toLowerCase()
  return nodes.filter((node) => node.type === 'folder' || node.label.toLowerCase().includes(lower))
}

/**
 * Phase 4 graph view: an Obsidian-style force-directed graph of notes and
 * wikilinks, opened from the sidebar. Clicking a node opens the note.
 */
export function GraphView({ entries, onSelectNote, onClose, locale = 'en' }: GraphViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const networkRef = useRef<VisNetwork | null>(null)
  const entriesRef = useRef(entries)
  const [query, setQuery] = useState('')
  const [showFolders, setShowFolders] = useState(true)
  const graph = useGraphData(entries, { includeFolders: showFolders })
  entriesRef.current = entries

  const graphData = useMemo(
    () => (graph.nodes.length > 0 ? graph : buildGraphData(entries, { includeFolders: showFolders })),
    [graph, entries, showFolders],
  )

  const filteredNodes = useMemo(
    () => filterNodes(graphData.nodes, query),
    [graphData.nodes, query],
  )
  const visibleNodeIds = useMemo(() => new Set(filteredNodes.map((node) => node.id)), [filteredNodes])

  const handleNodeClick = useCallback(
    (params: unknown) => {
      const { nodes } = params as { nodes?: string[] }
      const nodeId = nodes?.[0]
      if (!nodeId) return
      const node = graphData.nodes.find((candidate) => candidate.id === nodeId)
      if (!node || node.type !== 'note') return
      const entry = entriesRef.current.find((candidate) => candidate.path === node.path)
      if (entry) onSelectNote(entry)
    },
    [graphData.nodes, onSelectNote],
  )

  useEffect(() => {
    let cancelled = false
    let disposeClick: (() => void) | null = null

    void (async () => {
      const container = containerRef.current
      if (!container || graphData.nodes.length === 0) return
      const mod = (await import('vis-network/standalone')) as unknown as VisNetworkModule
      if (cancelled || !containerRef.current) return

      const accentBlue = readCssColor('--accent-blue', '#3b82f6')
      const accentPurple = readCssColor('--accent-purple', '#a855f7')
      const foreground = readCssColor('--foreground', '#1f2937')
      const border = readCssColor('--border', '#e5e7eb')
      const muted = readCssColor('--muted-foreground', '#6b7280')

      const nodes = filteredNodes.map((node) => ({
        id: node.id,
        label: node.label,
        value: nodeSize(node.degree),
        group: node.type,
        title: node.path,
      }))
      const edges = graphData.edges
        .filter((edge) => visibleNodeIds.has(edge.from) && visibleNodeIds.has(edge.to))
        .map((edge) => ({ from: edge.from, to: edge.to }))

      const network = new mod.Network(
        container,
        { nodes, edges },
        {
          autoResize: true,
          physics: {
            solver: 'forceAtlas2Based',
            forceAtlas2Based: { gravitationalConstant: -60, centralGravity: 0.01, springLength: 110, springConstant: 0.08, damping: 0.5 },
            stabilization: { fit: true },
          },
          interaction: { hover: true, tooltipDelay: 150 },
          nodes: {
            shape: 'dot',
            scaling: { min: 8, max: 48 },
            font: { color: foreground, size: 12, face: 'inherit' },
            borderWidth: 1,
            color: {
              border,
              background: accentBlue,
              highlight: { border: accentBlue, background: accentPurple },
              hover: { border: accentBlue, background: accentPurple },
            },
          },
          edges: {
            color: { color: muted, highlight: accentBlue, hover: accentBlue },
            width: 0.6,
            selectionWidth: 1.4,
          },
        },
      )
      networkRef.current = network
      network.on('click', handleNodeClick)
      disposeClick = () => {
        // vis-network has no per-event off; destroying the network detaches all.
      }
    })()

    return () => {
      cancelled = true
      networkRef.current?.destroy()
      networkRef.current = null
      disposeClick?.()
    }
  }, [graphData, filteredNodes, visibleNodeIds, handleNodeClick])

  const handleFit = useCallback(() => {
    networkRef.current?.fit({ animation: true })
  }, [])

  return (
    <div
      className="flex h-full min-h-0 flex-col overflow-hidden bg-background"
      data-testid="graph-view"
    >
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
        <div className="relative flex-1 max-w-72">
          <MagnifyingGlass
            size={13}
            className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={translate(locale, 'graph.searchPlaceholder')}
            className="h-7 pl-7 text-[12px]"
            aria-label={translate(locale, 'graph.searchPlaceholder')}
            data-testid="graph-search"
          />
        </div>
        <Button
          type="button"
          variant={showFolders ? 'secondary' : 'ghost'}
          size="sm"
          className="h-7 text-[12px]"
          onClick={() => setShowFolders((current) => !current)}
          aria-pressed={showFolders}
          data-testid="graph-toggle-folders"
        >
          {translate(locale, 'graph.showFolders')}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="h-7 w-7"
          onClick={handleFit}
          aria-label={translate(locale, 'graph.fitView')}
          title={translate(locale, 'graph.fitView')}
          data-testid="graph-fit"
        >
          <Crosshair size={15} />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="h-7 w-7"
          onClick={onClose}
          aria-label={translate(locale, 'graph.close')}
          title={translate(locale, 'graph.close')}
          data-testid="graph-close"
        >
          <X size={15} />
        </Button>
      </div>
      {graphData.nodes.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-[13px] text-muted-foreground">
          {translate(locale, 'graph.empty')}
        </div>
      ) : (
        <div ref={containerRef} className={cn('min-h-0 flex-1')} data-testid="graph-canvas" />
      )}
    </div>
  )
}
