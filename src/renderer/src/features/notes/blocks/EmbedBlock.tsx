/**
 * EmbedBlock.tsx
 *
 * Renders `![[target]]` embeds. Image embeds are resolved via the
 * `asset:read` IPC bridge into a data-URI `<img>`. Note embeds transclude
 * the note's parsed AST inline, capped at a configurable depth to prevent
 * infinite recursion.
 *
 * Requirements: 11.1 – 11.7
 */

import React, { useEffect, useMemo, useRef, useState } from 'react'
import { Node, Parent } from 'mdast'
import { FileEntry } from '@shared/types'
import { useAppContext } from '../../../shared/store'
import { OCRTextPanel } from './OCRTextPanel'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const IPC_TIMEOUT_MS = 5000
const MAX_EMBED_DEPTH = 5

/** File extensions that should be rendered as images. */
const IMAGE_EXT_RE = /\.(png|jpg|jpeg|gif|svg|webp|bmp|ico)$/i

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Check whether a target looks like an image path. */
function isImageTarget(target: string): boolean {
  return IMAGE_EXT_RE.test(target)
}

/**
 * Build a lookup index from lowercase basename (without `.md`) → list of
 * absolute paths for markdown notes.
 */
function buildNoteIndex(vaultFiles: FileEntry[]): Map<string, string[]> {
  const index = new Map<string, string[]>()
  for (const entry of vaultFiles) {
    const key = entry.name.replace(/\.md$/i, '').toLowerCase()
    const existing = index.get(key)
    if (existing) {
      existing.push(entry.path)
    } else {
      index.set(key, [entry.path])
    }
  }
  return index
}

/**
 * Resolve a wiki-link-style target to a markdown note path, or null.
 */
function resolveNoteTarget(target: string, index: Map<string, string[]>): string | null {
  const key = target.replace(/\.md$/i, '').toLowerCase()
  const matches = index.get(key)
  if (!matches || matches.length === 0) return null
  // Shortest path = closest to vault root
  return matches.sort((a, b) => a.length - b.length)[0]
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface EmbedBlockProps {
  target: string
  embedDepth: number
  renderNodes: (nodes: Node[], filePath: string) => React.ReactNode
}

type EmbedState = 'loading' | 'ready' | 'error'

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function EmbedBlock({
  target,
  embedDepth,
  renderNodes
}: EmbedBlockProps): React.JSX.Element {
  const { state } = useAppContext()
  const vaultFiles = state.vault?.files ?? []
  const vaultPath = state.vault?.path ?? ''

  const [embedState, setEmbedState] = useState<EmbedState>('loading')
  const [data, setData] = useState<React.ReactNode | null>(null)
  const [error, setError] = useState<string>('')

  const cancelledRef = useRef(false)

  const noteIndex = useMemo(() => buildNoteIndex(vaultFiles), [vaultFiles])

  useEffect(() => {
    cancelledRef.current = false
    setEmbedState('loading')
    setError('')

    const loadEmbed = async (): Promise<void> => {
      // ── Depth cap ──────────────────────────────────────────────────────
      if (embedDepth >= MAX_EMBED_DEPTH) {
        setData(
          <div className="text-xs text-white/30 italic px-2 py-1">
            ⤷ Max embed depth reached for <code className="font-mono">![[{target}]]</code>
          </div>
        )
        setEmbedState('ready')
        return
      }

      try {
        // ── Image embed ──────────────────────────────────────────────────
        if (isImageTarget(target)) {
          const resolvedPath = vaultPath ? vaultPath + '/' + target : target
          const result = await window.ipc.file
            .readAsset(resolvedPath)
            .then((r: unknown) => r as { dataUri?: string; error?: string })

          if (cancelledRef.current) return

          if (result.error) {
            setError(result.error)
            setEmbedState('error')
            return
          }

          // Store the resolved path for OCR check
          const resolvedImagePath = resolvedPath
          setData(
            <>
              <img
                src={result.dataUri}
                alt={target}
                className="max-w-full h-auto rounded my-2"
                loading="lazy"
              />
              <OCRTextPanel imagePath={resolvedImagePath} />
            </>
          )
          setEmbedState('ready')
          return
        }

        // ── Note embed (transclusion) ────────────────────────────────────
        const resolvedPath = resolveNoteTarget(target, noteIndex)
        if (!resolvedPath) {
          setError(`Not found: "${target}"`)
          setEmbedState('error')
          return
        }

        const fileResult = await withTimeout(
          window.ipc.file
            .get(resolvedPath)
            .then((r: unknown) => r as { path: string; ast: Parent }),
          IPC_TIMEOUT_MS
        )

        if (cancelledRef.current) return

        // Render the embedded note's AST recursively
        setData(
          <div className="embedded-note">
            {renderNodes(fileResult.ast.children as Node[], fileResult.path)}
          </div>
        )
        setEmbedState('ready')
      } catch (err) {
        if (cancelledRef.current) return
        setError(err instanceof Error ? err.message : 'Failed to load embed')
        setEmbedState('error')
      }
    }

    loadEmbed()

    return () => {
      cancelledRef.current = true
    }
  }, [target, embedDepth, vaultPath, vaultFiles, noteIndex, renderNodes])

  // ---- Loading state ----
  if (embedState === 'loading') {
    return (
      <div
        className="my-2 rounded bg-white/[0.03] p-3 animate-pulse"
        aria-busy="true"
        aria-label="Loading embed…"
      >
        <div className="h-4 w-3/4 rounded bg-white/10" />
      </div>
    )
  }

  // ---- Error / broken embed ----
  if (embedState === 'error') {
    return (
      <span
        className="inline-flex items-center gap-1 text-xs text-white/40"
        title={error}
        aria-label={`Broken embed: ${target}`}
      >
        <span aria-hidden="true" className="text-red-400/60">
          ⚠
        </span>
        <code className="font-mono">![[{target}]]</code>
        <span className="text-white/20">— {error}</span>
      </span>
    )
  }

  // ---- Ready ----
  return (
    <div
      className="embed-block my-2 rounded border-l-2 border-white/10 pl-3"
      data-embed-target={target}
    >
      {data}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Timeout utility (shared, duplicated here to avoid cross-boundary import)
// ---------------------------------------------------------------------------

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Request timed out')), ms)
    promise.then(
      (val) => {
        clearTimeout(timer)
        resolve(val)
      },
      (err) => {
        clearTimeout(timer)
        reject(err)
      }
    )
  })
}

export default EmbedBlock
