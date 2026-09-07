/**
 * Hook for communicating with the Nabu MCP WebSocket bridge.
 *
 * Provides typed tool invocations for vault operations:
 * - readNote, createNote, searchNotes, appendToNote
 *
 * Connection is lazy — only opens when first tool is called.
 */
import { useCallback, useRef, useState } from 'react'

const DEFAULT_WS_URL = 'ws://localhost:9710'

interface PendingRequest {
  resolve: (value: unknown) => void
  reject: (reason: unknown) => void
}

interface SearchResult {
  path: string
  title: string
  snippet: string
}

export function useMcpBridge(wsUrl = DEFAULT_WS_URL) {
  const wsRef = useRef<WebSocket | null>(null)
  const pendingRef = useRef<Map<string, PendingRequest>>(new Map())
  const idCounterRef = useRef(0)
  const [connected, setConnected] = useState(false)

  const ensureConnection = useCallback((): Promise<WebSocket> => {
    const ws = wsRef.current
    if (ws && ws.readyState === WebSocket.OPEN) return Promise.resolve(ws)

    return new Promise((resolve, reject) => {
      const newWs = new WebSocket(wsUrl)

      newWs.onopen = () => {
        wsRef.current = newWs
        setConnected(true)
        resolve(newWs)
      }

      newWs.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data)
          const pending = pendingRef.current.get(msg.id)
          if (pending) {
            pendingRef.current.delete(msg.id)
            if (msg.error) {
              pending.reject(new Error(msg.error))
            } else {
              pending.resolve(msg.result)
            }
          }
        } catch {
          // ignore malformed messages
        }
      }

      newWs.onclose = () => {
        wsRef.current = null
        setConnected(false)
      }

      newWs.onerror = () => {
        reject(new Error('WebSocket connection failed'))
      }
    })
  }, [wsUrl])

  const callTool = useCallback(async <T>(tool: string, args: Record<string, unknown>): Promise<T> => {
    const ws = await ensureConnection()
    const id = `mcp-${++idCounterRef.current}`

    return new Promise((resolve, reject) => {
      pendingRef.current.set(id, { resolve: resolve as (value: unknown) => void, reject })
      ws.send(JSON.stringify({ id, tool, args }))

      // Timeout after 30 seconds
      setTimeout(() => {
        if (pendingRef.current.has(id)) {
          pendingRef.current.delete(id)
          reject(new Error('MCP tool call timed out'))
        }
      }, 30_000)
    })
  }, [ensureConnection])

  const readNote = useCallback(
    (path: string) => callTool<{ content: string }>('read_note', { path }),
    [callTool],
  )

  const createNote = useCallback(
    (path: string, title: string, isA?: string) =>
      callTool<string>('create_note', { path, title, is_a: isA }),
    [callTool],
  )

  const searchNotes = useCallback(
    (query: string, limit?: number) =>
      callTool<SearchResult[]>('search_notes', { query, limit }),
    [callTool],
  )

  const appendToNote = useCallback(
    (path: string, text: string) =>
      callTool<{ ok: boolean }>('append_to_note', { path, text }),
    [callTool],
  )

  return { connected, readNote, createNote, searchNotes, appendToNote, callTool }
}
