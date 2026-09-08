import { useState, useEffect, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '../mock-tauri'
import { trackEvent } from '../lib/telemetry'

export type HighlightElement = 'editor' | 'tab' | 'properties' | 'notelist' | null

export interface ClarifyingQuestionForm {
  id: string
  question: string
  mode: string | null
  options: Array<{
    id: string
    label: string
    description?: string
  }>
  allow_custom: boolean
  remember_key: string | null
}

export interface AiActivity {
  highlightElement: HighlightElement
  highlightPath: string | null
  clarifyingForm: ClarifyingQuestionForm | null
}

export interface AiActivityCallbacks {
  onOpenNote?: (path: string) => void
  onOpenTab?: (path: string) => void
  onSetFilter?: (type: string) => void
  onVaultChanged?: (path?: string) => void
  onVaultRegistryChanged?: (path?: string) => void
}

const WS_UI_URL = 'ws://localhost:9711'
const HIGHLIGHT_DURATION_MS = 800
const RECONNECT_DELAY_MS = 3000

type UiActionMessage = Record<string, unknown> & {
  action: string
  type: 'ui_action'
}

/** Tools the frontend will execute on behalf of the MCP server via Tauri. */
const RELAYED_TOOLS = new Set([
  'search_notes_semantic',
  'evaluate_sheet_with_formulas',
  'create_report',
  'crunch_financials',
])

interface RelayedToolRequest {
  type: 'tool_request'
  action: string
  id: string
  vaultPath?: unknown
  query?: unknown
  limit?: unknown
  [key: string]: unknown
}

interface SheetRelayPayload {
  csvContent?: unknown
  cellOverrides?: unknown
  dependencies?: unknown
  links?: unknown
  maxDepth?: unknown
  timezone?: unknown
  title?: unknown
  vaultPath?: unknown
  saveNote?: unknown
}

function relayPayloadString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null
}

function relayPayloadLimit(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : undefined
}

function relayPayloadStringOrUndefined(value: unknown): string | undefined {
  return relayPayloadString(value) ?? undefined
}

function relayPayloadOverrides(value: unknown): Record<string, string> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return {}
  const overrides: Record<string, string> = {}
  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (typeof entry === 'string') overrides[key] = entry
  }
  return overrides
}

function relayPayloadDependencies(value: unknown): Array<{ path: string; content: string }> {
  if (!Array.isArray(value)) return []
  return value.flatMap((entry) => {
    if (typeof entry !== 'object' || entry === null) return []
    const record = entry as Record<string, unknown>
    const path = relayPayloadString(record.path)
    const content = typeof record.content === 'string' ? record.content : null
    return path && content !== null ? [{ path, content }] : []
  })
}

function relayPayloadLinks(
  value: unknown,
): Array<{ sourcePath: string; target: string; targetPath: string }> {
  if (!Array.isArray(value)) return []
  return value.flatMap((entry) => {
    if (typeof entry !== 'object' || entry === null) return []
    const record = entry as Record<string, unknown>
    const sourcePath = relayPayloadString(record.sourcePath)
    const target = relayPayloadString(record.target)
    const targetPath = relayPayloadString(record.targetPath)
    return sourcePath && target && targetPath ? [{ sourcePath, target, targetPath }] : []
  })
}

function relayPayloadBoolean(value: unknown): boolean {
  return value === true
}

/** Build the Tauri `EvaluateSheetRequest` payload from a relay request. */
function sheetRelayRequest(request: SheetRelayPayload) {
  return {
    csvContent: relayPayloadString(request.csvContent) ?? '',
    cellOverrides: relayPayloadOverrides(request.cellOverrides),
    dependencies: relayPayloadDependencies(request.dependencies),
    links: relayPayloadLinks(request.links),
    maxDepth: relayPayloadLimit(request.maxDepth) ?? null,
    timezone: relayPayloadStringOrUndefined(request.timezone) ?? null,
  }
}

function parseToolRequest(event: MessageEvent): RelayedToolRequest | null {
  try {
    const data = JSON.parse(String(event.data))
    if (
      isRecord(data)
      && data.type === 'tool_request'
      && typeof data.action === 'string'
      && typeof data.id === 'string'
      && RELAYED_TOOLS.has(data.action)
    ) {
      return data as RelayedToolRequest
    }
  } catch {
    // Malformed JSON — not a tool request.
  }
  return null
}

/**
 * Execute a relayed MCP tool request through the matching Tauri command and
 * send a `tool_response` back over the same socket. Errors are reported as
 * `tool_response` errors so the MCP server can surface them to the agent.
 */
async function handleToolRequest(
  request: RelayedToolRequest,
  socket: WebSocket,
): Promise<void> {
  const respond = (payload: Record<string, unknown>): void => {
    if (socket.readyState !== WebSocket.OPEN) return
    socket.send(JSON.stringify({ type: 'tool_response', id: request.id, ...payload }))
  }

  try {
    if (!isTauri()) {
      respond({ error: 'Tauri unavailable' })
      return
    }
    if (request.action === 'search_notes_semantic') {
      const vaultPath = relayPayloadString(request.vaultPath)
      const query = relayPayloadString(request.query)
      if (!vaultPath || !query) {
        respond({ error: 'vaultPath and query are required' })
        return
      }
      const result = await invoke<unknown>('search_notes_semantic', {
        request: {
          query,
          vaultPath,
          limit: relayPayloadLimit(request.limit) ?? 10,
          hideGitignoredFiles: false,
        },
      })
      respond({ result })
      return
    }
    if (request.action === 'evaluate_sheet_with_formulas') {
      const result = await invoke<unknown>('evaluate_sheet_with_formulas', {
        request: sheetRelayRequest(request),
      })
      respond({ result })
      return
    }
    if (request.action === 'create_report') {
      const vaultPath = relayPayloadString(request.vaultPath)
      if (!vaultPath) {
        respond({ error: 'vaultPath is required' })
        return
      }
      const result = await invoke<unknown>('create_report', {
        request: {
          ...sheetRelayRequest(request),
          title: relayPayloadStringOrUndefined(request.title) ?? null,
          vaultPath,
        },
      })
      respond({ result })
      return
    }
    if (request.action === 'crunch_financials') {
      const vaultPath = relayPayloadString(request.vaultPath)
      const saveNote = relayPayloadBoolean(request.saveNote)
      if (saveNote && !vaultPath) {
        respond({ error: 'vaultPath is required when saveNote is true' })
        return
      }
      const result = await invoke<unknown>('crunch_financials', {
        request: {
          ...sheetRelayRequest(request),
          title: relayPayloadStringOrUndefined(request.title) ?? null,
          vaultPath: vaultPath ?? null,
          saveNote,
        },
      })
      respond({ result })
      return
    }
    respond({ error: `Unsupported relayed tool: ${request.action}` })
  } catch (error) {
    respond({ error: error instanceof Error ? error.message : String(error) })
  }
}
type StringPayloadAction = 'open_note' | 'open_tab' | 'set_filter'
type StringPayloadCallback = 'onOpenNote' | 'onOpenTab' | 'onSetFilter'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined
}

function parseUiActionMessage(event: MessageEvent): UiActionMessage | null {
  try {
    const data = JSON.parse(String(event.data))
    if (!isRecord(data) || data.type !== 'ui_action' || typeof data.action !== 'string') return null
    return data as UiActionMessage
  } catch {
    return null
  }
}

function highlightElementFromValue(value: unknown): HighlightElement {
  if (value === 'editor' || value === 'tab' || value === 'properties' || value === 'notelist') return value
  return null
}

function useLatestAiActivityCallbacks(callbacks?: AiActivityCallbacks) {
  const callbacksRef = useRef(callbacks)
  useEffect(() => { callbacksRef.current = callbacks })
  return callbacksRef
}

function parseClarifyingForm(value: unknown): ClarifyingQuestionForm | null {
  if (!isRecord(value)) return null
  const id = optionalString(value.id)
  const question = optionalString(value.question)
  if (!id || !question) return null
  const rawOptions = Array.isArray(value.options) ? value.options : []
  const options = rawOptions
    .map((option) => {
      if (!isRecord(option)) return null
      const label = optionalString(option.label)
      if (!label) return null
      return {
        id: optionalString(option.id) ?? label,
        label,
        description: optionalString(option.description),
      }
    })
    .filter((option) => option !== null)
  if (options.length === 0) return null
  return {
    id,
    question,
    mode: optionalString(value.mode) ?? null,
    options,
    allow_custom: value.allow_custom !== false,
    remember_key: optionalString(value.remember_key) ?? null,
  }
}

function useAiHighlightState() {
  const [highlightElement, setHighlightElement] = useState<HighlightElement>(null)
  const [highlightPath, setHighlightPath] = useState<string | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearHighlightTimer = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = null
  }, [])

  const clearHighlight = useCallback(() => {
    setHighlightElement(null)
    setHighlightPath(null)
  }, [])

  const showHighlight = useCallback((message: UiActionMessage) => {
    setHighlightElement(highlightElementFromValue(message.element))
    setHighlightPath(optionalString(message.path) ?? null)
    clearHighlightTimer()
    timerRef.current = setTimeout(clearHighlight, HIGHLIGHT_DURATION_MS)
  }, [clearHighlight, clearHighlightTimer])

  return {
    clearHighlightTimer,
    highlightElement,
    highlightPath,
    showHighlight,
  }
}

function dispatchStringPayload(value: unknown, callback?: (value: string) => void): void {
  const payload = optionalString(value)
  if (payload) callback?.(payload)
}

const STRING_PAYLOAD_CALLBACKS: Record<StringPayloadAction, StringPayloadCallback> = {
  open_note: 'onOpenNote',
  open_tab: 'onOpenTab',
  set_filter: 'onSetFilter',
}

function isStringPayloadAction(action: string): action is StringPayloadAction {
  return action === 'open_note' || action === 'open_tab' || action === 'set_filter'
}

function stringPayloadValue(message: UiActionMessage): unknown {
  return message.action === 'set_filter' ? message.filterType : message.path
}

function dispatchVaultRegistryChanged(
  message: UiActionMessage,
  callbacksRef: ReturnType<typeof useLatestAiActivityCallbacks>,
): void {
  const path = optionalString(message.path)
  const registrationType = message.registrationType === 'clone' ? 'clone' : 'attach'
  trackEvent('mcp_vault_registered', { registration_type: registrationType })
  callbacksRef.current?.onVaultRegistryChanged?.(path)
  window.dispatchEvent(new CustomEvent('nabu:vault-registry-changed', { detail: { path } }))
}

function dispatchClarifyingQuestion(
  message: UiActionMessage,
  setClarifyingForm: (form: ClarifyingQuestionForm | null) => void,
): void {
  setClarifyingForm(parseClarifyingForm(message.form))
}

function dispatchUiActionMessage(
  message: UiActionMessage,
  callbacksRef: ReturnType<typeof useLatestAiActivityCallbacks>,
  showHighlight: (message: UiActionMessage) => void,
  setClarifyingForm: (form: ClarifyingQuestionForm | null) => void,
): void {
  if (message.action === 'highlight') {
    showHighlight(message)
    return
  }
  if (message.action === 'vault_changed') {
    callbacksRef.current?.onVaultChanged?.(optionalString(message.path))
    return
  }
  if (message.action === 'vault_registry_changed') {
    dispatchVaultRegistryChanged(message, callbacksRef)
    return
  }
  if (message.action === 'ask_clarifying_question') {
    dispatchClarifyingQuestion(message, setClarifyingForm)
    return
  }
  if (!isStringPayloadAction(message.action)) return

  const callbackName = STRING_PAYLOAD_CALLBACKS[message.action]
  dispatchStringPayload(stringPayloadValue(message), callbacksRef.current?.[callbackName])
}

function useUiActionMessageHandler(
  callbacksRef: ReturnType<typeof useLatestAiActivityCallbacks>,
  showHighlight: (message: UiActionMessage) => void,
  setClarifyingForm: (form: ClarifyingQuestionForm | null) => void,
) {
  return useCallback((event: MessageEvent) => {
    const message = parseUiActionMessage(event)
    if (message) {
      dispatchUiActionMessage(message, callbacksRef, showHighlight, setClarifyingForm)
    }
  }, [callbacksRef, setClarifyingForm, showHighlight])
}

function useUiActionSocket(handleMessage: (event: MessageEvent) => void, clearHighlightTimer: () => void): void {
  useEffect(() => {
    let ws: WebSocket | null = null
    let mounted = true
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null

    function connect() {
      if (!mounted) return
      try {
        ws = new WebSocket(WS_UI_URL)
        ws.onmessage = (event: MessageEvent) => {
          // MCP → app relay: execute the tool via Tauri and answer in-place.
          const toolRequest = parseToolRequest(event)
          if (toolRequest && ws && ws.readyState === WebSocket.OPEN) {
            void handleToolRequest(toolRequest, ws)
            return
          }
          handleMessage(event)
        }
        ws.onclose = () => {
          if (mounted) reconnectTimer = setTimeout(connect, RECONNECT_DELAY_MS)
        }
        ws.onerror = () => { /* Silent — bridge may not be running */ }
      } catch {
        if (mounted) reconnectTimer = setTimeout(connect, RECONNECT_DELAY_MS)
      }
    }

    connect()

    return () => {
      mounted = false
      ws?.close()
      clearHighlightTimer()
      if (reconnectTimer) clearTimeout(reconnectTimer)
    }
  }, [clearHighlightTimer, handleMessage])
}

/**
 * Listens on the UI WebSocket bridge (port 9711) for UI action events
 * from the MCP server. Handles highlight, open_note, open_tab, set_filter,
 * and vault_changed actions.
 */
export function useAiActivity(callbacks?: AiActivityCallbacks): AiActivity {
  const callbacksRef = useLatestAiActivityCallbacks(callbacks)
  const {
    clearHighlightTimer,
    highlightElement,
    highlightPath,
    showHighlight,
  } = useAiHighlightState()
  const [clarifyingForm, setClarifyingForm] = useState<ClarifyingQuestionForm | null>(null)
  const handleMessage = useUiActionMessageHandler(callbacksRef, showHighlight, setClarifyingForm)

  useUiActionSocket(handleMessage, clearHighlightTimer)

  return { clarifyingForm, highlightElement, highlightPath }
}
