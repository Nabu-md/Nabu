import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'
import { useSpeechRecognition, type SpeechRecognitionState } from './useDictation'

export interface FluidVoiceModels {
  installed: boolean
  models: string[]
}

type FluidVoiceStatus = 'idle' | 'listening' | 'processing' | 'unavailable'

export type DictationBackend = 'web_speech' | 'fluidvoice'

const FLUIDVOICE_MODEL_STORAGE_KEY = 'nabu:fluidvoice-model'
const FLUIDVOICE_BACKEND_STORAGE_KEY = 'nabu:fluidvoice-backend'

function loadPersistedModel(): string {
  try {
    const stored = localStorage.getItem(FLUIDVOICE_MODEL_STORAGE_KEY)
    if (stored) return stored
  } catch {
    // Storage unavailable; fall through to default.
  }
  return 'parakeet'
}

function loadPersistedBackend(): DictationBackend {
  try {
    const stored = localStorage.getItem(FLUIDVOICE_BACKEND_STORAGE_KEY)
    if (stored === 'fluidvoice' || stored === 'web_speech') return stored
  } catch {
    // Storage unavailable; fall through to default.
  }
  return 'web_speech'
}

function persistModel(model: string): void {
  try {
    localStorage.setItem(FLUIDVOICE_MODEL_STORAGE_KEY, model)
  } catch {
    // Best-effort persistence only.
  }
}

function persistBackend(backend: DictationBackend): void {
  try {
    localStorage.setItem(FLUIDVOICE_BACKEND_STORAGE_KEY, backend)
  } catch {
    // Best-effort persistence only.
  }
}

interface FluidVoiceState {
  installed: boolean
  models: string[]
  model: string
  setModel: (model: string) => void
  backend: DictationBackend
  setBackend: (backend: DictationBackend) => void
  status: FluidVoiceStatus
  transcript: string
  startDictation: () => Promise<void>
  stopDictation: () => Promise<void>
}

function fluidVoiceInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

/**
 * FluidVoice dictation control surface (on-device macOS STT via the separate
 * FluidVoice app). Falls back to the Web Speech API when FluidVoice is not
 * installed, on non-macOS platforms, or in browser mode.
 */
export function useFluidVoiceDictation(lang = 'en-US'): FluidVoiceState & { webSpeech: SpeechRecognitionState } {
  const webSpeech = useSpeechRecognition(lang)
  const [installed, setInstalled] = useState(false)
  const [models, setModels] = useState<string[]>([])
  const [model, setModelState] = useState(loadPersistedModel)
  const [backend, setBackendState] = useState<DictationBackend>(loadPersistedBackend)
  const [status, setStatus] = useState<FluidVoiceStatus>('idle')
  const [transcript, setTranscript] = useState('')
  const pollRef = useRef<number | null>(null)

  useEffect(() => {
    let cancelled = false
    async function detect() {
      try {
        const [isInstalled, availableModels] = await Promise.all([
          fluidVoiceInvoke<boolean>('fluidvoice_installed'),
          fluidVoiceInvoke<string[]>('fluidvoice_models'),
        ])
        if (cancelled) return
        setInstalled(isInstalled)
        setModels(availableModels)
        // Only override the persisted model when it is no longer offered.
        if (availableModels.length > 0 && !availableModels.includes(model)) {
          setModelState(availableModels[0])
          persistModel(availableModels[0])
        }
        // Auto-select FluidVoice on first run; a persisted web_speech choice
        // is only overridden when FluidVoice is actually available.
        if (isInstalled) setBackendState('fluidvoice')
        else setBackendState('web_speech')
      } catch {
        if (!cancelled) setInstalled(false)
      }
    }
    void detect()
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const setModel = useCallback((next: string) => {
    setModelState(next)
    persistModel(next)
  }, [])

  const setBackend = useCallback((next: DictationBackend) => {
    setBackendState(next)
    persistBackend(next)
  }, [])

  const pollStatus = useCallback(async () => {
    try {
      const current = await fluidVoiceInvoke<number>('fluidvoice_status')
      setStatus(current === 1 ? 'listening' : current === 2 ? 'processing' : 'idle')
    } catch {
      setStatus('unavailable')
    }
  }, [])

  useEffect(() => {
    if (backend !== 'fluidvoice' || !installed) {
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current)
        pollRef.current = null
      }
      return
    }
    void pollStatus()
    pollRef.current = window.setInterval(() => void pollStatus(), 1000)
    return () => {
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current)
        pollRef.current = null
      }
    }
  }, [backend, installed, pollStatus])

  const startDictation = useCallback(async () => {
    if (backend === 'fluidvoice' && installed) {
      await fluidVoiceInvoke('fluidvoice_start', { model })
      setStatus('listening')
      return
    }
    webSpeech.start()
  }, [backend, installed, model, webSpeech])

  const stopDictation = useCallback(async () => {
    if (backend === 'fluidvoice' && installed) {
      await fluidVoiceInvoke('fluidvoice_stop')
      setStatus('processing')
      return
    }
    webSpeech.stop()
  }, [backend, installed, webSpeech])

  // Mirror Web Speech transcript so the pill shows text in either backend.
  useEffect(() => {
    if (webSpeech.text) setTranscript(webSpeech.text)
  }, [webSpeech.text])

  return {
    installed,
    models,
    model,
    setModel,
    backend,
    setBackend,
    status,
    transcript,
    startDictation,
    stopDictation,
    webSpeech,
  }
}
