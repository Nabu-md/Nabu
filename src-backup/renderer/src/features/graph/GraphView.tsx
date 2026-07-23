/**
 * GraphView.tsx — Knowledge graph visualization
 *
 * Displays the vault's graph of note relationships. Supports three modes:
 * - Files mode: traditional node-per-file with wikilink edges
 * - Tags mode: node-per-tag with co-occurrence edges (Req 38.2)
 * - Blocks mode: block reference visualization (Req 38.6)
 *
 * Requirements: 38.1, 38.2, 38.3, 38.4, 38.5, 38.6
 */

import React, { useEffect, useRef, useState, useCallback } from 'react'
import { forceSimulation, forceManyBody, forceLink, forceCenter } from 'd3-force'
import { useAppContext } from '../../shared/store'
import type { Edge, FileEntry } from '@shared/types'
import {
  computeTagGraph,
  getTagNodeColor,
  getTagDisplayLabel,
  getTagRecentNotes,
  computeBlockGraph,
  extractBlockRefLinks,
  type BlockGraphNode,
  type BlockGraphEdge
} from '@shared/graph-utils'

// ---------------------------------------------------------------------------
// Local type definitions for d3-force nodes/links
// (d3-force v3 ships no bundled TS types)
// ---------------------------------------------------------------------------

interface D3Node {
  id: string
  label: string
  // Positions — mutated by the simulation in place
  x: number
  y: number
  vx?: number
  vy?: number
  fx?: number | null
  fy?: number | null
  // For tag nodes
  count?: number
  radius?: number
  color?: string
  // For block nodes (blocks graph mode)
  isBlock?: boolean
  // d3 internal index
  index?: number
}

interface D3Link {
  source: string | D3Node
  target: string | D3Node
}

// Minimal typings for the simulation object returned by d3
interface D3Sim {
  stop(): D3Sim
  restart(): D3Sim
  alpha(a?: number): D3Sim
  alphaDecay(a?: number): D3Sim
  alphaTarget(a?: number): D3Sim
  force(name: string, f?: unknown): D3Sim
  nodes(nodes?: D3Node[]): D3Sim
  _updateTransform?: (t: { x: number; y: number; scale: number }) => void
}

export function GraphView(): React.JSX.Element {
  const { state, dispatch } = useAppContext()
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const nodesRef = useRef<D3Node[]>([])
  const linksRef = useRef<D3Link[]>([])
  const simRef = useRef<D3Sim | null>(null)
  const rafRef = useRef<number | null>(null)
  const dragNodeRef = useRef<D3Node | null>(null)
  const panStartRef = useRef<{ x: number; y: number; tx: number; ty: number } | null>(null)

  const [searchQuery, setSearchQuery] = useState('')
  const [mode, setMode] = useState<'global' | 'local'>('global')
  const [canvasW, setCanvasW] = useState(800)
  const [canvasH, setCanvasH] = useState(600)
  const [transform, setTransform] = useState({ x: 0, y: 0, scale: 1 })
  // Tooltip state for tag hover (Req 38.4)
  const [hoveredTag, setHoveredTag] = useState<{
    label: string
    count: number
    x: number
    y: number
    recentNotes: FileEntry[]
  } | null>(null)

  // Block reference graph state (Req 38.6) — populated asynchronously from
  // the extended index (block definitions) plus a raw-content scan for
  // cross-note block references via the existing `note:get-raw` IPC.
  const [blockNodes, setBlockNodes] = useState<BlockGraphNode[]>([])
  const [blockEdges, setBlockEdges] = useState<BlockGraphEdge[]>([])
  const [blockGraphLoading, setBlockGraphLoading] = useState(false)

  // ---------------------------------------------------------------------------
  // Resize observer
  // ---------------------------------------------------------------------------
  useEffect(() => {
    const container = containerRef.current
    if (!container) return
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (entry) {
        setCanvasW(entry.contentRect.width)
        setCanvasH(entry.contentRect.height)
      }
    })
    ro.observe(container)
    return () => ro.disconnect()
  }, [])

  // ---------------------------------------------------------------------------
  // Build nodes and links based on graphMode
  // ---------------------------------------------------------------------------
  useEffect(() => {
    const files = state.vault?.files ?? []
    const graphMode = state.graphMode

    let nodes: D3Node[]
    let links: D3Link[]

    if (graphMode === 'tags') {
      // Tags mode: use computeTagGraph from graph-utils
      if (state.extendedIndex) {
        const tagGraph = computeTagGraph(state.extendedIndex, files)
        nodes = tagGraph.nodes.map((n) => ({
          id: n.id,
          label: n.label,
          x: canvasW / 2 + (Math.random() - 0.5) * 100,
          y: canvasH / 2 + (Math.random() - 0.5) * 100,
          count: n.count,
          radius: n.radius,
          color: getTagNodeColor(n.label)
        }))
        links = tagGraph.edges.map((e) => ({
          source: e.source,
          target: e.target
        }))
      } else {
        nodes = []
        links = []
      }
    } else if (graphMode === 'blocks') {
      // Blocks mode (Req 38.6): visualise block references.
      // Block *definitions* come from the extended index; cross-note
      // *references* are fetched asynchronously (see the blocks effect below)
      // and merged into `blockNodes`/`blockEdges`.
      nodes = blockNodes.map((n) => ({
        id: n.id,
        label: n.label,
        x: canvasW / 2 + (Math.random() - 0.5) * 100,
        y: canvasH / 2 + (Math.random() - 0.5) * 100,
        // Block nodes are drawn as squares; note nodes as circles.
        isBlock: n.isBlock
      })) as D3Node[]
      links = blockEdges.map((e) => ({ source: e.source, target: e.target }))
    } else {
      // Files mode (default): existing behavior
      const edges: Edge[] = state.graphEdges

      // Build nodes
      nodes = files.map((f) => ({
        id: f.path,
        label: f.name,
        x: canvasW / 2 + (Math.random() - 0.5) * 100,
        y: canvasH / 2 + (Math.random() - 0.5) * 100
      }))
      // Build links (source/target start as string ids; d3 replaces with object refs)
      const nodeIds = new Set(nodes.map((n) => n.id))
      links = edges
        .filter((e) => nodeIds.has(e.source) && nodeIds.has(e.target))
        .map((e) => ({ source: e.source, target: e.target }))
    }

    nodesRef.current = nodes
    linksRef.current = links

    // Stop previous simulation / animation frame
    if (simRef.current) simRef.current.stop()
    if (rafRef.current) cancelAnimationFrame(rafRef.current)

    // Mutable snapshot of transform for the draw closure (avoids stale captures)
    let currentTransform = { x: 0, y: 0, scale: 1 }

    // Create simulation
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const sim: D3Sim = (forceSimulation as any)(nodes)
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      .force('charge', (forceManyBody as any)().strength(-150))
      .force(
        'link',
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (forceLink as any)(links)
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          .id((d: any) => d.id)
          .distance(100)
      )
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      .force('center', (forceCenter as any)(canvasW / 2, canvasH / 2))
      .alpha(0.3)
      .alphaDecay(0.02)

    // Attach helper so the transform-sync effect can push updates into the closure
    sim._updateTransform = (t) => {
      currentTransform = t
    }

    simRef.current = sim

    // Render loop
    const draw = (): void => {
      const canvas = canvasRef.current
      if (!canvas) return
      const ctx = canvas.getContext('2d')
      if (!ctx) return

      ctx.clearRect(0, 0, canvas.width, canvas.height)
      ctx.save()
      ctx.translate(currentTransform.x, currentTransform.y)
      ctx.scale(currentTransform.scale, currentTransform.scale)

      const allNodes = nodesRef.current
      const allLinks = linksRef.current

      // Filter for local mode — only the current file and its direct neighbours
      let visibleNodeIds: Set<string> | null = null
      if (mode === 'local' && state.currentFile) {
        visibleNodeIds = new Set<string>([state.currentFile])
        for (const link of allLinks) {
          const s =
            typeof link.source === 'object' ? (link.source as D3Node).id : (link.source as string)
          const t =
            typeof link.target === 'object' ? (link.target as D3Node).id : (link.target as string)
          if (s === state.currentFile) visibleNodeIds.add(t)
          if (t === state.currentFile) visibleNodeIds.add(s)
        }
      }

      // Apply search filter
      const searchLower = searchQuery.toLowerCase()
      const isVisible = (node: D3Node): boolean => {
        if (visibleNodeIds && !visibleNodeIds.has(node.id)) return false
        if (searchLower && !node.label.toLowerCase().includes(searchLower)) return false
        return true
      }

      const visibleNodes = allNodes.filter(isVisible)
      const visibleIds = new Set(visibleNodes.map((n) => n.id))

      // Draw edges
      ctx.strokeStyle =
        getComputedStyle(document.documentElement).getPropertyValue('--nabu-border') || '#2a2a2a'
      ctx.lineWidth = 1
      for (const link of allLinks) {
        const s =
          typeof link.source === 'object'
            ? (link.source as D3Node)
            : allNodes.find((n) => n.id === (link.source as string))
        const t =
          typeof link.target === 'object'
            ? (link.target as D3Node)
            : allNodes.find((n) => n.id === (link.target as string))
        if (!s || !t || !visibleIds.has(s.id) || !visibleIds.has(t.id)) continue
        ctx.beginPath()
        ctx.moveTo(s.x, s.y)
        ctx.lineTo(t.x, t.y)
        ctx.stroke()
      }

      // Draw nodes
      const accentColor =
        getComputedStyle(document.documentElement).getPropertyValue('--nabu-accent') || '#60a5fa'
      const textColor =
        getComputedStyle(document.documentElement).getPropertyValue('--nabu-text') || '#e5e5e5'

      // Tag node colors using CSS variables (same palette as tab groups)
      const tagColors = {
        blue:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-blue') ||
          '#3b82f6',
        red:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-red') ||
          '#ef4444',
        green:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-green') ||
          '#10b981',
        yellow:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-yellow') ||
          '#f59e0b',
        purple:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-purple') ||
          '#8b5cf6',
        orange:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-orange') ||
          '#f97316',
        cyan:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-cyan') ||
          '#06b6d4',
        pink:
          getComputedStyle(document.documentElement).getPropertyValue('--nabu-tag-pink') ||
          '#ec4899'
      }

      for (const node of visibleNodes) {
        const { x, y } = node

        if (graphMode === 'blocks' && node.isBlock) {
          // Draw block nodes as small squares (Req 38.6)
          const size = 7
          ctx.fillStyle = accentColor
          ctx.beginPath()
          ctx.rect(x - size / 2, y - size / 2, size, size)
          ctx.fill()
          ctx.font = '9px sans-serif'
          ctx.fillStyle = textColor
          ctx.textAlign = 'left'
          ctx.textBaseline = 'middle'
          ctx.fillText(node.label, x + size / 2 + 3, y)
        } else if (graphMode === 'tags' && node.radius !== undefined) {
          // Draw tag nodes as rounded pills
          const radius = node.radius
          const color = node.color ? tagColors[node.color as keyof typeof tagColors] : accentColor

          ctx.fillStyle = color
          ctx.beginPath()
          // Draw rounded rectangle (pill shape)
          const labelWidth = Math.max(radius * 2, node.label.length * 6)
          const height = radius * 1.6
          ctx.roundRect(x - labelWidth / 2, y - height / 2, labelWidth, height, height / 2)
          ctx.fill()

          // Draw label inside pill
          ctx.font = '10px sans-serif'
          ctx.fillStyle = '#fff'
          ctx.textAlign = 'center'
          ctx.textBaseline = 'middle'
          ctx.fillText(getTagDisplayLabel(node.label), x, y)
        } else {
          // Draw file nodes as circles (existing behavior)
          ctx.beginPath()
          ctx.arc(x, y, 6, 0, Math.PI * 2)
          ctx.fillStyle = accentColor
          ctx.fill()
          ctx.font = '10px sans-serif'
          ctx.fillStyle = textColor
          ctx.fillText(node.label, x + 9, y + 4)
        }
      }

      ctx.restore()
      rafRef.current = requestAnimationFrame(draw)
    }

    rafRef.current = requestAnimationFrame(draw)

    return () => {
      sim.stop()
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.vault, state.graphEdges, state.extendedIndex, state.graphMode, canvasW, canvasH])

  // Sync transform into the draw closure whenever it changes
  useEffect(() => {
    simRef.current?._updateTransform?.(transform)
  }, [transform])

  // ---------------------------------------------------------------------------
  // Blocks mode: build the block reference graph (Req 38.6)
  //
  // Block *definitions* are already in `state.extendedIndex.blockRefs`.
  // Cross-note *references* (`[[Note#^id]]`) are derived by scanning each
  // note's raw content through the existing `note:get-raw` IPC. A module
  // level cache avoids re-fetching unchanged files across mode switches.
  // ---------------------------------------------------------------------------
  const rawCache = useRef<Map<string, string>>(new Map())

  useEffect(() => {
    if (state.graphMode !== 'blocks' || !state.extendedIndex) return

    const blockRefs = state.extendedIndex.blockRefs as unknown as Record<
      string,
      Record<string, string>
    >
    const files = state.vault?.files ?? []

    // Build a basename → full-path resolver (mirrors wiki-link resolution).
    const nameToPath = new Map<string, string>()
    for (const f of files) {
      nameToPath.set(f.name.toLowerCase(), f.path)
    }

    // Seed the graph with block definitions immediately (synchronous).
    const seedLinks: Array<{ source: string; targetNote: string; blockId: string }> = []
    setBlockNodes(computeBlockGraph(blockRefs, seedLinks).nodes)
    setBlockEdges(computeBlockGraph(blockRefs, seedLinks).edges)

    let cancelled = false
    setBlockGraphLoading(true)

    const buildFromRefs = async (): Promise<void> => {
      const refs: Array<{ source: string; targetNote: string; blockId: string }> = []
      for (const file of files) {
        if (cancelled) return
        // Skip files that cannot define block references of interest.
        if (file.path.toLowerCase().endsWith('.pdf')) continue
        let raw = rawCache.current.get(file.path)
        if (raw === undefined) {
          try {
            const result = await window.electron.note.getRaw(file.path)
            raw = result.content ?? ''
            rawCache.current.set(file.path, raw)
          } catch {
            continue
          }
        }
        for (const link of extractBlockRefLinks(raw)) {
          // Resolve the target note name to a full path (wiki-link style).
          const targetPath = nameToPath.get(link.targetNote.toLowerCase()) ?? link.targetNote
          refs.push({ source: file.path, targetNote: targetPath, blockId: link.blockId })
        }
      }
      if (cancelled) return
      const graph = computeBlockGraph(blockRefs, refs)
      setBlockNodes(graph.nodes)
      setBlockEdges(graph.edges)
      setBlockGraphLoading(false)
    }

    void buildFromRefs()

    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.graphMode, state.extendedIndex, state.vault?.files])

  // Hit-test: find the node closest to given canvas coords (within 20 px)
  const findNode = useCallback(
    (cx: number, cy: number): D3Node | null => {
      const { x: tx, y: ty, scale } = transform
      const wx = (cx - tx) / scale
      const wy = (cy - ty) / scale
      let closest: D3Node | null = null
      let minDist = 20
      for (const node of nodesRef.current) {
        const dx = node.x - wx
        const dy = node.y - wy
        const dist = Math.sqrt(dx * dx + dy * dy)
        if (dist < minDist) {
          minDist = dist
          closest = node
        }
      }
      return closest
    },
    [transform]
  )

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect()
      if (!rect) return
      const cx = e.clientX - rect.left
      const cy = e.clientY - rect.top
      const node = findNode(cx, cy)
      if (node) {
        dragNodeRef.current = node
        node.fx = node.x
        node.fy = node.y
        // Clear tooltip when dragging starts
        setHoveredTag(null)
      } else {
        panStartRef.current = { x: e.clientX, y: e.clientY, tx: transform.x, ty: transform.y }
      }
    },
    [findNode, transform]
  )

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      // If dragging, don't show tooltip
      if (dragNodeRef.current) {
        const rect = canvasRef.current?.getBoundingClientRect()
        if (!rect) return
        const wx = (e.clientX - rect.left - transform.x) / transform.scale
        const wy = (e.clientY - rect.top - transform.y) / transform.scale
        dragNodeRef.current.fx = wx
        dragNodeRef.current.fy = wy
        simRef.current?.alphaTarget(0.1).restart()
        return
      }

      // If panning, don't show tooltip
      if (panStartRef.current) {
        const dx = e.clientX - panStartRef.current.x
        const dy = e.clientY - panStartRef.current.y
        setTransform((prev) => ({
          ...prev,
          x: panStartRef.current!.tx + dx,
          y: panStartRef.current!.ty + dy
        }))
        return
      }

      // Check for tag hover in tags mode
      const rect = canvasRef.current?.getBoundingClientRect()
      if (!rect) return
      const cx = e.clientX - rect.left
      const cy = e.clientY - rect.top
      const node = findNode(cx, cy)

      if (state.graphMode === 'tags' && node && state.extendedIndex && node.count !== undefined) {
        const recentNotes = getTagRecentNotes(
          node.label,
          state.vault?.files ?? [],
          state.extendedIndex.tagIndex
        )
        setHoveredTag({
          label: node.label,
          count: node.count,
          x: e.clientX,
          y: e.clientY,
          recentNotes
        })
      } else {
        setHoveredTag(null)
      }
    },
    [findNode, transform, state.graphMode, state.extendedIndex, state.vault?.files]
  )

  const handleMouseUp = useCallback(() => {
    if (dragNodeRef.current) {
      dragNodeRef.current.fx = null
      dragNodeRef.current.fy = null
      dragNodeRef.current = null
      simRef.current?.alphaTarget(0)
    }
    panStartRef.current = null
  }, [])

  const handleMouseLeave = useCallback(() => {
    setHoveredTag(null)
  }, [])

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect()
      if (!rect) return
      const cx = e.clientX - rect.left
      const cy = e.clientY - rect.top
      const node = findNode(cx, cy)
      if (node) {
        if (state.graphMode === 'tags') {
          // Tags mode: dispatch to filter file tree (Req 38.5)
          dispatch({ type: 'TAG_FILTER_TOGGLE', payload: node.label })
          setHoveredTag(null)
        } else if (state.graphMode === 'blocks') {
          // Blocks mode: open the owning note (Req 38.6). Block nodes carry
          // their owner path in `node.id` for note nodes, or `path#^blockId`
          // for block nodes — resolve the owning note path either way.
          const ownerPath = node.isBlock ? (node.id.split('#')[0] ?? node.id) : node.id
          if (ownerPath.toLowerCase().endsWith('.pdf')) {
            dispatch({ type: 'PDF_OPENED', payload: { path: ownerPath } })
            return
          }
          window.electron.file
            .get(ownerPath)
            .then((fileAST) => {
              dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
              dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
            })
            .catch(console.error)
        } else {
          // Files mode: open the note (or PDF viewer pane)
          if (node.id.toLowerCase().endsWith('.pdf')) {
            dispatch({ type: 'PDF_OPENED', payload: { path: node.id } })
            return
          }
          window.electron.file
            .get(node.id)
            .then((fileAST) => {
              dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
              dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
            })
            .catch(console.error)
        }
      }
    },
    [findNode, dispatch, state.graphMode]
  )

  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault()
    const factor = e.deltaY < 0 ? 1.1 : 0.9
    setTransform((prev) => ({
      ...prev,
      scale: Math.max(0.1, Math.min(5, prev.scale * factor))
    }))
  }, [])

  // Handle graph mode toggle
  const handleGraphModeChange = (newMode: 'files' | 'tags' | 'blocks'): void => {
    dispatch({ type: 'GRAPH_MODE_CHANGED', payload: newMode })
  }

  // Render tooltip for tag hover (Req 38.4)
  const renderTagTooltip = (): React.JSX.Element | null => {
    if (!hoveredTag) return null

    return (
      <div
        className="absolute pointer-events-none bg-nabu-bg-pop border border-nabu-border rounded px-2 py-1 text-xs shadow-lg z-10"
        style={{
          left: hoveredTag.x + 10,
          top: hoveredTag.y - 10,
          maxWidth: '250px'
        }}
      >
        <div className="font-semibold text-nabu-accent mb-1">{hoveredTag.label}</div>
        <div className="text-nabu-text-muted mb-1">
          {hoveredTag.count} {hoveredTag.count === 1 ? 'note' : 'notes'}
        </div>
        {hoveredTag.recentNotes.length > 0 && (
          <div className="text-nabu-text-faint">
            Recent: {hoveredTag.recentNotes.map((n) => n.name).join(', ')}
          </div>
        )}
      </div>
    )
  }

  // Render blocks mode empty / loading state (Req 38.6)
  const renderBlocksPlaceholder = (): React.JSX.Element => {
    const hasBlockDefs =
      state.extendedIndex && Object.keys(state.extendedIndex.blockRefs).length > 0
    if (blockGraphLoading) {
      return (
        <div className="absolute inset-0 flex items-center justify-center text-nabu-text-faint text-sm">
          <p className="text-center max-w-md">Scanning notes for block references…</p>
        </div>
      )
    }
    if (!hasBlockDefs) {
      return (
        <div className="absolute inset-0 flex items-center justify-center text-nabu-text-faint text-sm">
          <p className="text-center max-w-md">
            Define blocks with a <code>^block-id</code> marker at the end of a line, then link
            to them with <code>[[Note#^block-id]]</code> to populate this view.
          </p>
        </div>
      )
    }
    return (
      <div className="absolute inset-0 flex items-center justify-center text-nabu-text-faint text-sm">
        <p className="text-center max-w-md">
          No block references found yet. Link to a block with{' '}
          <code>[[Note#^block-id]]</code> to connect notes.
        </p>
      </div>
    )
  }

  return (
    <div className="graph-view flex flex-col h-full" aria-label="Graph view">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-2 shrink-0 border-b border-nabu-border">
        <input
          type="text"
          aria-label="Filter graph nodes"
          placeholder="Search nodes…"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="flex-1 px-2 py-1 text-xs rounded bg-nabu-bg-mute border border-nabu-border
                     text-nabu-text placeholder:text-nabu-text-faint
                     focus:outline-none focus:border-nabu-accent transition-colors"
        />

        {/* Graph mode toggle - Req 38.1 */}
        <div role="radiogroup" aria-label="Graph view mode" className="flex gap-1">
          <button
            role="radio"
            aria-checked={state.graphMode === 'files'}
            onClick={() => handleGraphModeChange('files')}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              state.graphMode === 'files'
                ? 'bg-nabu-accent/20 text-nabu-accent'
                : 'text-nabu-text-muted hover:text-nabu-text bg-nabu-bg-mute'
            }`}
            title="Files view - node per file with wikilink edges"
          >
            Files
          </button>
          <button
            role="radio"
            aria-checked={state.graphMode === 'tags'}
            onClick={() => handleGraphModeChange('tags')}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              state.graphMode === 'tags'
                ? 'bg-nabu-accent/20 text-nabu-accent'
                : 'text-nabu-text-muted hover:text-nabu-text bg-nabu-bg-mute'
            }`}
            title="Tags view - node per tag with co-occurrence edges"
          >
            Tags
          </button>
          <button
            role="radio"
            aria-checked={state.graphMode === 'blocks'}
            onClick={() => handleGraphModeChange('blocks')}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              state.graphMode === 'blocks'
                ? 'bg-nabu-accent/20 text-nabu-accent'
                : 'text-nabu-text-muted hover:text-nabu-text bg-nabu-bg-mute'
            }`}
            title="Blocks view - visualize block references as a graph"
           >
             Blocks
          </button>
        </div>

        {/* Graph scope toggle (global/local) */}
        <div role="radiogroup" aria-label="Graph scope" className="flex gap-1">
          <button
            role="radio"
            aria-checked={mode === 'global'}
            onClick={() => setMode('global')}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              mode === 'global'
                ? 'bg-nabu-accent/20 text-nabu-accent'
                : 'text-nabu-text-muted hover:text-nabu-text bg-nabu-bg-mute'
            }`}
          >
            Global
          </button>
          <button
            role="radio"
            aria-checked={mode === 'local'}
            onClick={() => setMode('local')}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              mode === 'local'
                ? 'bg-nabu-accent/20 text-nabu-accent'
                : 'text-nabu-text-muted hover:text-nabu-text bg-nabu-bg-mute'
            }`}
          >
            Local
          </button>
        </div>
      </div>

      {/* Canvas */}
      <div ref={containerRef} className="flex-1 relative overflow-hidden">
        <canvas
          ref={canvasRef}
          width={canvasW}
          height={canvasH}
          role="img"
          aria-label="Note relationship graph"
          className="absolute inset-0 cursor-grab active:cursor-grabbing"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseLeave}
          onClick={handleClick}
          onWheel={handleWheel}
        />
        {renderTagTooltip()}
        {state.vault === null && (
          <div className="absolute inset-0 flex items-center justify-center text-nabu-text-faint text-sm">
            No vault open
          </div>
        )}
        {state.vault !== null && state.vault.files.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center text-nabu-text-faint text-sm">
            No notes in vault
          </div>
        )}
        {state.graphMode === 'blocks' && blockNodes.length === 0 && renderBlocksPlaceholder()}
      </div>
    </div>
  )
}
