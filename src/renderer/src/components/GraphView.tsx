import React, { useEffect, useRef, useState, useCallback } from 'react'
import { forceSimulation, forceManyBody, forceLink, forceCenter } from 'd3-force'
import { useAppContext } from '../App'
import type { Edge } from '../../../shared/types'

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

  // Resize observer
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

  // Build nodes and links, init simulation
  useEffect(() => {
    const files = state.vault?.files ?? []
    const edges: Edge[] = state.graphEdges

    // Build nodes
    const nodes: D3Node[] = files.map((f) => ({
      id: f.path,
      label: f.name,
      x: canvasW / 2 + (Math.random() - 0.5) * 100,
      y: canvasH / 2 + (Math.random() - 0.5) * 100
    }))
    nodesRef.current = nodes

    // Build links (source/target start as string ids; d3 replaces with object refs)
    const nodeIds = new Set(nodes.map((n) => n.id))
    const links: D3Link[] = edges
      .filter((e) => nodeIds.has(e.source) && nodeIds.has(e.target))
      .map((e) => ({ source: e.source, target: e.target }))
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
      for (const node of visibleNodes) {
        const { x, y } = node
        ctx.beginPath()
        ctx.arc(x, y, 6, 0, Math.PI * 2)
        ctx.fillStyle = accentColor
        ctx.fill()
        ctx.font = '10px sans-serif'
        ctx.fillStyle = textColor
        ctx.fillText(node.label, x + 9, y + 4)
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
  }, [state.vault, state.graphEdges, canvasW, canvasH])

  // Sync transform into the draw closure whenever it changes
  useEffect(() => {
    simRef.current?._updateTransform?.(transform)
  }, [transform])

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
      } else {
        panStartRef.current = { x: e.clientX, y: e.clientY, tx: transform.x, ty: transform.y }
      }
    },
    [findNode, transform]
  )

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (dragNodeRef.current) {
        const rect = canvasRef.current?.getBoundingClientRect()
        if (!rect) return
        const wx = (e.clientX - rect.left - transform.x) / transform.scale
        const wy = (e.clientY - rect.top - transform.y) / transform.scale
        dragNodeRef.current.fx = wx
        dragNodeRef.current.fy = wy
        simRef.current?.alphaTarget(0.1).restart()
      } else if (panStartRef.current) {
        const dx = e.clientX - panStartRef.current.x
        const dy = e.clientY - panStartRef.current.y
        setTransform((prev) => ({
          ...prev,
          x: panStartRef.current!.tx + dx,
          y: panStartRef.current!.ty + dy
        }))
      }
    },
    [transform]
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

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect()
      if (!rect) return
      const cx = e.clientX - rect.left
      const cy = e.clientY - rect.top
      const node = findNode(cx, cy)
      if (node) {
        window.electron.file
          .get(node.id)
          .then((fileAST) => {
            dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
            dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
          })
          .catch(console.error)
      }
    },
    [findNode, dispatch]
  )

  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault()
    const factor = e.deltaY < 0 ? 1.1 : 0.9
    setTransform((prev) => ({
      ...prev,
      scale: Math.max(0.1, Math.min(5, prev.scale * factor))
    }))
  }, [])

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
          onMouseLeave={handleMouseUp}
          onClick={handleClick}
          onWheel={handleWheel}
        />
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
      </div>
    </div>
  )
}
