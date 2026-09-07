import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '../mock-tauri'
import type { AiAgentId } from '../lib/aiAgents'
import type { AiAgentPermissionMode } from '../lib/aiAgentPermissionMode'
import { createScopedStreamEventName } from '../utils/aiStreamEvents'
import { cleanupTauriEventListener } from '../utils/tauriEventCleanup'

export type DeepResearchStatus = 'idle' | 'running' | 'done' | 'error'

export interface DeepResearchSource {
  title: string
  url: string
  excerpt: string
}

export interface DeepResearchIteration {
  number: number
  goal: string
}

export interface DeepResearchState {
  status: DeepResearchStatus
  iterations: DeepResearchIteration[]
  sources: DeepResearchSource[]
  interimSummaries: string[]
  report: string | null
  error: string | null
  sessionId: string | null
}

export const INITIAL_DEEP_RESEARCH_STATE: DeepResearchState = {
  status: 'idle',
  iterations: [],
  sources: [],
  interimSummaries: [],
  report: null,
  error: null,
  sessionId: null,
}

export interface StartDeepResearchOptions {
  query: string
  vaultPath: string
  vaultPaths?: string[]
  depth?: number
  model?: string
  agent?: AiAgentId
  permissionMode?: AiAgentPermissionMode
  signal?: AbortSignal
}

export type DeepResearchEvent =
  | { kind: 'IterationStart'; iteration: number; goal: string }
  | { kind: 'ToolStart'; tool_name: string; tool_id: string }
  | { kind: 'ToolDone'; tool_id: string; output?: string }
  | { kind: 'SourceAdded'; title: string; url: string; excerpt: string }
  | { kind: 'InterimSummary'; iteration: number; text: string }
  | { kind: 'Result'; report: string }
  | { kind: 'Error'; message: string }
  | { kind: 'Done' }

export interface DeepResearchCallbacks {
  onIterationStart?: (iteration: number, goal: string) => void
  onSourceAdded?: (source: DeepResearchSource) => void
  onInterimSummary?: (iteration: number, text: string) => void
  onResult?: (report: string) => void
  onError?: (message: string) => void
  onDone?: () => void
}

function mockDeepResearchResponse(query: string, callbacks: DeepResearchCallbacks): void {
  const { onIterationStart, onSourceAdded, onInterimSummary, onResult, onError, onDone } = callbacks
  const deliver = (event: DeepResearchEvent) => {
    switch (event.kind) {
      case 'IterationStart':
        onIterationStart?.(event.iteration, event.goal)
        return
      case 'SourceAdded':
        onSourceAdded?.({ title: event.title, url: event.url, excerpt: event.excerpt })
        return
      case 'InterimSummary':
        onInterimSummary?.(event.iteration, event.text)
        return
      case 'Result':
        onResult?.(event.report)
        return
      case 'Error':
        onError?.(event.message)
        return
      case 'Done':
        onDone?.()
        return
    }
  }

  const current: DeepResearchEvent = {
    kind: 'IterationStart',
    iteration: 1,
    goal: `Gather information about "${query}" from multiple sources.`,
  }
  const sequence: DeepResearchEvent[] = [
    current,
    { kind: 'SourceAdded', title: 'en.wikipedia.org', url: `https://en.wikipedia.org/wiki/${encodeURIComponent(query)}`, excerpt: 'Fetching overview…' },
    { kind: 'SourceAdded', title: 'news.example.com', url: 'https://news.example.com/search?q='.concat(encodeURIComponent(query)), excerpt: 'Fetching recent coverage…' },
    { kind: 'ToolDone', tool_id: 'tool-1', output: 'ok' },
    { kind: 'InterimSummary', iteration: 1, text: `[mock] Gathered ${query} overview from 2 sources. Identifying gaps…` },
    { kind: 'IterationStart', iteration: 2, goal: `Fill gaps for "${query}" with additional sources.` },
    { kind: 'Result', report: `# Research Report: ${query}\n\n[mock deep research] This is a simulated report produced in browser mode. In the desktop app the Claude CLI performs real multi-step research with web scraping.\n\n## Key findings\n- Finding one about ${query}\n- Finding two about ${query}\n\n## Sources\n- https://en.wikipedia.org\n- https://news.example.com` },
    { kind: 'Done' },
  ]

  let index = 0
  const timer = window.setInterval(() => {
    if (index < sequence.length) deliver(sequence[index])
    index += 1
    if (index >= sequence.length) window.clearInterval(timer)
  }, 500)
}

function handleDeepResearchEvent(event: DeepResearchEvent, callbacks: DeepResearchCallbacks): void {
  switch (event.kind) {
    case 'IterationStart':
      callbacks.onIterationStart?.(event.iteration, event.goal)
      return
    case 'SourceAdded':
      callbacks.onSourceAdded?.({ title: event.title, url: event.url, excerpt: event.excerpt })
      return
    case 'InterimSummary':
      callbacks.onInterimSummary?.(event.iteration, event.text)
      return
    case 'Result':
      callbacks.onResult?.(event.report)
      return
    case 'Error':
      callbacks.onError?.(event.message)
      return
    case 'Done':
      callbacks.onDone?.()
      return
  }
}

async function streamNativeDeepResearch(
  request: StartDeepResearchOptions,
  callbacks: DeepResearchCallbacks,
  signal?: AbortSignal,
): Promise<string | null> {
  const { listen } = await import('@tauri-apps/api/event')
  const eventName = createScopedStreamEventName('deep-research-stream')

  const abortNativeStream = (): void => {
    void invoke<boolean>('abort_deep_research', { eventName }).catch(() => {})
  }
  let removeAbortListener = (): void => {}
  if (signal) {
    if (signal.aborted) {
      abortNativeStream()
      return null
    }
    signal.addEventListener('abort', abortNativeStream, { once: true })
    removeAbortListener = () => signal.removeEventListener('abort', abortNativeStream)
  }

  const unlisten = await listen<DeepResearchEvent>(eventName, (event) => {
    handleDeepResearchEvent(event.payload, callbacks)
  })

  try {
    const sessionId = await invoke<string | null>('start_deep_research', {
      request: {
        query: request.query,
        vault_path: request.vaultPath,
        vault_paths: request.vaultPaths && request.vaultPaths.length > 0 ? request.vaultPaths : null,
        depth: request.depth ?? 3,
        model: request.model?.trim() || null,
        agent: request.agent ?? null,
        permission_mode: request.permissionMode ?? 'safe',
        event_name: eventName,
      },
    })
    return sessionId
  } finally {
    removeAbortListener()
    cleanupTauriEventListener(unlisten)
  }
}

/**
 * Runs a deep research session and resolves with the session id once the
 * stream completes (or null when aborted). Events are delivered through
 * callbacks so the hook can maintain progress state.
 */
export async function startDeepResearch(
  request: StartDeepResearchOptions,
  callbacks: DeepResearchCallbacks = {},
): Promise<string | null> {
  if (request.signal?.aborted) {
    callbacks.onDone?.()
    return null
  }

  if (!isTauri()) {
    mockDeepResearchResponse(request.query, callbacks)
    return 'mock-deep-research-session'
  }

  try {
    return await streamNativeDeepResearch(request, callbacks, request.signal)
  } catch (error) {
    callbacks.onError?.(error instanceof Error ? error.message : String(error))
    callbacks.onDone?.()
    return null
  }
}

/**
 * Manages a single deep research session: progress state (iterations,
 * sources, interim summaries) plus the final report, with abort support.
 */
export function useDeepResearch() {
  const [state, setState] = useState<DeepResearchState>(INITIAL_DEEP_RESEARCH_STATE)
  const abortRef = useRef<AbortController | null>(null)
  const runningRef = useRef(false)

  const reset = useCallback(() => {
    abortRef.current?.abort()
    abortRef.current = null
    runningRef.current = false
    setState(INITIAL_DEEP_RESEARCH_STATE)
  }, [])

  const run = useCallback(async (options: StartDeepResearchOptions) => {
    if (runningRef.current) return
    runningRef.current = true
    const controller = new AbortController()
    abortRef.current = controller

    setState({
      status: 'running',
      iterations: [],
      sources: [],
      interimSummaries: [],
      report: null,
      error: null,
      sessionId: null,
    })

    const callbacks: DeepResearchCallbacks = {
      onIterationStart: (iteration, goal) => {
        setState((current) => ({
          ...current,
          iterations: [...current.iterations, { number: iteration, goal }],
        }))
      },
      onSourceAdded: (source) => {
        setState((current) => ({
          ...current,
          sources: [...current.sources, source],
        }))
      },
      onInterimSummary: (_iteration, text) => {
        setState((current) => ({
          ...current,
          interimSummaries: [...current.interimSummaries, text],
        }))
      },
      onResult: (report) => {
        setState((current) => ({ ...current, report }))
      },
      onError: (message) => {
        setState((current) => ({ ...current, error: message, status: 'error' }))
      },
      onDone: () => {
        runningRef.current = false
        setState((current) => ({
          ...current,
          status: current.error ? 'error' : current.report ? 'done' : current.status,
        }))
      },
    }

    const sessionId = await startDeepResearch(
      { ...options, signal: controller.signal },
      callbacks,
    )
    if (controller.signal.aborted) {
      setState((current) => ({
        ...current,
        status: current.report ? 'done' : 'idle',
        error: null,
      }))
      return
    }
    setState((current) => ({ ...current, sessionId }))
  }, [])

  const abort = useCallback(() => {
    abortRef.current?.abort()
  }, [])

  useEffect(() => {
    return () => abortRef.current?.abort()
  }, [])

  return { state, run, abort, reset }
}