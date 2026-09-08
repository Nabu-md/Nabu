import { useState, useEffect, useRef, useCallback } from 'react'
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
        ws.onmessage = handleMessage
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
