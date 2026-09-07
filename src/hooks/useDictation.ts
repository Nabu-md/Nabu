import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

export interface ClipboardCacheEntry {
  id: string
  kind: 'text' | 'image'
  preview: string
  created_at: string
}

export interface SpeechRecognitionState {
  text: string
  isListening: boolean
  supported: boolean
  start: () => void
  stop: () => void
}

type SpeechRecognitionLike = {
  lang: string
  continuous: boolean
  interimResults: boolean
  onresult: ((event: SpeechRecognitionResultEventLike) => void) | null
  onerror: ((event: { error: string }) => void) | null
  onend: (() => void) | null
  start: () => void
  stop: () => void
  abort: () => void
}

interface SpeechRecognitionResultEventLike {
  resultIndex: number
  results: ArrayLike<ArrayLike<{ transcript: string }> & { isFinal: boolean }>
}

type SpeechRecognitionConstructor = new () => SpeechRecognitionLike

function dictationInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

function resolveSpeechRecognitionConstructor(): SpeechRecognitionConstructor | null {
  if (typeof window === 'undefined') return null
  const candidate = (window as Window & {
    SpeechRecognition?: SpeechRecognitionConstructor
    webkitSpeechRecognition?: SpeechRecognitionConstructor
  })
  return candidate.SpeechRecognition ?? candidate.webkitSpeechRecognition ?? null
}

/**
 * Wraps the Web Speech API for live dictation transcription. Falls back to a
 * browser-only implementation when the native `start_dictation` IPC is
 * unavailable (non-macOS desktop builds).
 */
export function useSpeechRecognition(lang = 'en-US'): SpeechRecognitionState {
  const Recognition = resolveSpeechRecognitionConstructor()
  const [text, setText] = useState('')
  const [isListening, setIsListening] = useState(false)
  const recognitionRef = useRef<SpeechRecognitionLike | null>(null)
  const micStreamRef = useRef<MediaStream | null>(null)
  const supported = Recognition !== null

  const releaseMicrophone = useCallback(() => {
    micStreamRef.current?.getTracks().forEach((track) => track.stop())
    micStreamRef.current = null
  }, [])

  const stop = useCallback(() => {
    recognitionRef.current?.stop()
    releaseMicrophone()
    if (isTauri()) {
      void dictationInvoke<void>('stop_dictation', { sessionId: null }).catch(() => {})
    }
  }, [releaseMicrophone])

  const start = useCallback(async () => {
    if (!Recognition) return
    if (recognitionRef.current) {
      recognitionRef.current.abort()
      recognitionRef.current = null
    }

    // Explicitly request the microphone so the user sees the OS permission
    // prompt before recognition begins.
    try {
      if (typeof navigator !== 'undefined' && navigator.mediaDevices?.getUserMedia) {
        micStreamRef.current = await navigator.mediaDevices.getUserMedia({ audio: true })
      }
    } catch {
      // Recognition may still work in webviews that manage mic access itself.
    }

    const recognition = new Recognition()
    recognition.lang = lang
    recognition.continuous = true
    recognition.interimResults = true
    recognition.onresult = (event) => {
      let transcript = ''
      for (let index = event.resultIndex; index < event.results.length; index += 1) {
        const result = event.results[index]
        transcript += result[0]?.transcript ?? ''
      }
      setText((current) => (current ? `${current} ${transcript}`.trim() : transcript))
    }
    recognition.onerror = (event) => {
      if (event.error === 'no-speech' || event.error === 'aborted') return
      setIsListening(false)
      releaseMicrophone()
    }
    recognition.onend = () => {
      setIsListening(false)
      recognitionRef.current = null
      releaseMicrophone()
    }
    recognitionRef.current = recognition
    setText('')
    setIsListening(true)
    recognition.start()

    // Native dictation session for macOS audio input handling.
    if (isTauri()) {
      void dictationInvoke<string>('start_dictation', {}).catch(() => {})
    }
  }, [Recognition, lang, releaseMicrophone])

  useEffect(() => {
    return () => {
      recognitionRef.current?.abort()
      recognitionRef.current = null
      releaseMicrophone()
    }
  }, [releaseMicrophone])

  return { text, isListening, supported, start, stop }
}

/**
 * Loads the recent clipboard cache and restores entries on demand.
 */
export function useClipboardCache() {
  const [entries, setEntries] = useState<ClipboardCacheEntry[]>([])
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    try {
      const loaded = await dictationInvoke<ClipboardCacheEntry[]>('get_recent_clipboard_entries', {
        count: 10,
      })
      setEntries(loaded)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [])

  useEffect(() => {
    void refresh() // eslint-disable-line react-hooks/set-state-in-effect -- initial clipboard cache load
  }, [refresh])

  const restore = useCallback(async (id: string) => {
    try {
      const restored = await dictationInvoke<string>('restore_clipboard_entry', { id })
      await refresh()
      return restored
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      return null
    }
  }, [refresh])

  return { entries, error, refresh, restore }
}

/**
 * Handles file drops for the dictation pill's drop zone. Image files are
 * copied into the vault via the existing image pipeline; other files are
 * captured by the `capture_file_drop` backend command.
 */
export function useFileDropZone(vaultPath: string | null) {
  const [dragging, setDragging] = useState(false)
  const [captured, setCaptured] = useState<string[]>([])
  const [error, setError] = useState<string | null>(null)
  const dragDepth = useRef(0)

  const capture = useCallback(async (paths: string[]) => {
    if (!vaultPath || paths.length === 0) return
    try {
      const destinations = await dictationInvoke<string[]>('capture_file_drop', {
        vault_path: vaultPath,
        paths,
      })
      setCaptured(destinations)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [vaultPath])

  useEffect(() => {
    const handleDragEnter = (event: DragEvent) => {
      if (!vaultPath) return
      if (!Array.from(event.dataTransfer?.types ?? []).includes('Files')) return
      dragDepth.current += 1
      setDragging(true)
    }
    const handleDragLeave = () => {
      dragDepth.current = Math.max(0, dragDepth.current - 1)
      if (dragDepth.current === 0) setDragging(false)
    }
    const handleDrop = (event: DragEvent) => {
      dragDepth.current = 0
      setDragging(false)
      if (!vaultPath) return
      const files = Array.from(event.dataTransfer?.files ?? [])
      const paths = files
        .map((file) => (file as File & { path?: string }).path ?? file.name)
        .filter(Boolean)
      if (paths.length > 0) void capture(paths)
    }

    window.addEventListener('dragenter', handleDragEnter)
    window.addEventListener('dragleave', handleDragLeave)
    window.addEventListener('drop', handleDrop)
    return () => {
      window.removeEventListener('dragenter', handleDragEnter)
      window.removeEventListener('dragleave', handleDragLeave)
      window.removeEventListener('drop', handleDrop)
    }
  }, [capture, vaultPath])

  return { dragging, captured, error, capture }
}