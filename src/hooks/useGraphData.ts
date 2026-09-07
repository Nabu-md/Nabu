import { useEffect, useMemo, useRef, useState } from 'react'
import type { VaultEntry } from '../types'

export interface GraphNode {
  id: string
  label: string
  path: string
  type: 'note' | 'folder'
  degree: number
  folder: string
}

export interface GraphEdge {
  from: string
  to: string
}

export interface GraphData {
  nodes: GraphNode[]
  edges: GraphEdge[]
}

const WIKILINK_PATTERN = /\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|[^\]]*)?\]\]/g

function folderOf(path: string): string {
  const parts = path.split('/')
  return parts.length <= 1 ? '' : parts.slice(0, -1).join('/')
}

function noteIdFor(path: string): string {
  return `note:${path}`
}

function folderIdFor(path: string): string {
  return `folder:${path}`
}

function outgoingLinksFor(entry: VaultEntry): string[] {
  if (Array.isArray(entry.outgoingLinks)) return entry.outgoingLinks
  const links: string[] = []
  const content = `${entry.title}\n${entry.snippet ?? ''}`
  for (const match of content.matchAll(WIKILINK_PATTERN)) {
    if (match[1]) links.push(match[1].trim())
  }
  return links
}

function resolveLinkTarget(target: string, entriesByTitle: Map<string, VaultEntry>, entriesByPath: Map<string, VaultEntry>): VaultEntry | null {
  const direct = entriesByPath.get(target) ?? entriesByPath.get(`${target}.md`)
  if (direct) return direct
  const lower = target.toLowerCase()
  const byTitle = entriesByTitle.get(lower)
  if (byTitle) return byTitle
  const byTitleMd = entriesByTitle.get(`${lower}.md`)
  return byTitleMd ?? null
}

export interface BuildGraphDataOptions {
  /** Include folder nodes grouping their notes. */
  includeFolders?: boolean
}

/**
 * Builds a wikilink graph over the given vault entries: notes as nodes,
 * resolved outgoing links as edges, degree centrality for sizing, and
 * (optionally) folder group nodes.
 */
export function buildGraphData(entries: VaultEntry[], options: BuildGraphDataOptions = {}): GraphData {
  const { includeFolders = true } = options
  const markdownEntries = entries.filter((entry) => entry.fileKind !== 'binary')
  const entriesByTitle = new Map<string, VaultEntry>()
  const entriesByPath = new Map<string, VaultEntry>()
  for (const entry of markdownEntries) {
    if (entry.title) entriesByTitle.set(entry.title.toLowerCase(), entry)
    entriesByPath.set(entry.path, entry)
  }

  const nodes = new Map<string, GraphNode>()
  const edges = new Map<string, GraphEdge>()
  const degree = new Map<string, number>()

  const bumpDegree = (id: string) => degree.set(id, (degree.get(id) ?? 0) + 1)

  const ensureNoteNode = (entry: VaultEntry): GraphNode => {
    const id = noteIdFor(entry.path)
    let node = nodes.get(id)
    if (!node) {
      node = {
        id,
        label: entry.title || entry.filename,
        path: entry.path,
        type: 'note',
        degree: 0,
        folder: folderOf(entry.path),
      }
      nodes.set(id, node)
      if (includeFolders) {
        const folder = folderOf(entry.path)
        if (folder) {
          const folderId = folderIdFor(folder)
          if (!nodes.has(folderId)) {
            nodes.set(folderId, {
              id: folderId,
              label: folder.split('/').pop() ?? folder,
              path: folder,
              type: 'folder',
              degree: 0,
              folder,
            })
          }
        }
      }
    }
    return node
  }

  for (const entry of markdownEntries) {
    ensureNoteNode(entry)
    for (const target of outgoingLinksFor(entry)) {
      const resolved = resolveLinkTarget(target, entriesByTitle, entriesByPath)
      if (!resolved || resolved.path === entry.path) continue
      const fromId = noteIdFor(entry.path)
      const toId = noteIdFor(resolved.path)
      ensureNoteNode(resolved)
      const edgeKey = fromId < toId ? `${fromId}->${toId}` : `${toId}->${fromId}`
      if (!edges.has(edgeKey)) edges.set(edgeKey, { from: fromId, to: toId })
      bumpDegree(fromId)
      bumpDegree(toId)
    }
  }

  const finalNodes = [...nodes.values()].map((node) => ({
    ...node,
    degree: degree.get(node.id) ?? 0,
  }))
  const finalEdges = [...edges.values()]

  // Folder nodes connect to their notes so the force layout clusters them.
  if (includeFolders) {
    for (const node of finalNodes) {
      if (node.type !== 'note' || !node.folder) continue
      const folderEdgeKey = `${folderIdFor(node.folder)}-${node.id}`
      if (!edges.has(folderEdgeKey)) finalEdges.push({ from: folderIdFor(node.folder), to: node.id })
    }
  }

  return { nodes: finalNodes, edges: finalEdges }
}

/**
 * Memoized graph data for a set of vault entries, rebuilt when the entry
 * signature changes (path + outgoingLinks summary).
 */
export function useGraphData(entries: VaultEntry[], options: BuildGraphDataOptions = {}): GraphData {
  const { includeFolders = true } = options
  const signature = useMemo(
    () => entries
      .map((entry) => `${entry.path}:${(entry.outgoingLinks ?? []).length}`)
      .join('|'),
    [entries],
  )
  const signatureRef = useRef(signature)
  const [graphCache, setGraphCache] = useState<GraphData>({ nodes: [], edges: [] })

  useEffect(() => {
    signatureRef.current = signature
    const timer = window.setTimeout(() => {
      setGraphCache(buildGraphData(entries, { includeFolders }))
    }, 250)
    return () => window.clearTimeout(timer)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- rebuild only when the entry signature changes
  }, [signature, includeFolders])

  return graphCache
}
