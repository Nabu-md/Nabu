import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

export interface WordTiming {
  word: string
  start: number
  end: number
}

export type KokoroEngineStatus = 'checking' | 'unavailable' | 'ready'

export interface KokoroTtsState {
  status: KokoroEngineStatus
  voices: string[]
  voice: string
  setVoice: (voice: string) => void
  speed: number
  setSpeed: (speed: number) => void
  highlightEnabled: boolean
  setHighlightEnabled: (enabled: boolean) => void
  isPlaying: boolean
  isPaused: boolean
  currentTime: number
  duration: number
  currentWord: string | null
  /** Index of the last word whose end time has passed; -1 before playback. */
  currentWordIndex: number
  speak: (text: string) => Promise<void>
  pause: () => void
  resume: () => void
  stop: () => void
  rewind15: () => void
  forward15: () => void
  seek: (seconds: number) => void
}

const KOKORO_VOICE_STORAGE_KEY = 'nabu:kokoro-voice'
const KOKORO_SPEED_STORAGE_KEY = 'nabu:kokoro-speed'

function ttsInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

function loadPersistedVoice(fallback: string): string {
  try {
    const stored = localStorage.getItem(KOKORO_VOICE_STORAGE_KEY)
    if (stored) return stored
  } catch {
    // Storage unavailable; fall through to default.
  }
  return fallback
}

function loadPersistedSpeed(): number {
  try {
    const stored = localStorage.getItem(KOKORO_SPEED_STORAGE_KEY)
    if (stored) {
      const parsed = Number(stored)
      if (Number.isFinite(parsed)) return Math.min(2, Math.max(0.5, parsed))
    }
  } catch {
    // Storage unavailable; fall through to default.
  }
  return 1
}

export function useKokoroTts(options: {
  initialVoice?: string | null
  initialSpeed?: number | null
  initialHighlight?: boolean | null
} = {}): KokoroTtsState {
  const { initialVoice, initialSpeed, initialHighlight } = options
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const rafRef = useRef<number | null>(null)
  const objectUrlRef = useRef<string | null>(null)

  const [status, setStatus] = useState<KokoroEngineStatus>('checking')
  const [voices, setVoices] = useState<string[]>([])
  const [voice, setVoiceState] = useState(() => loadPersistedVoice(initialVoice ?? 'af_sky'))
  const [speed, setSpeedState] = useState(() =>
    initialSpeed && initialSpeed >= 0.5 && initialSpeed <= 2 ? initialSpeed : loadPersistedSpeed(),
  )
  const [highlightEnabled, setHighlightState] = useState(initialHighlight ?? true)
  const [isPlaying, setIsPlaying] = useState(false)
  const [isPaused, setIsPaused] = useState(false)
  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [timings, setTimings] = useState<WordTiming[]>([])

  // Detect the kokoro CLI and load the voice list once.
  useEffect(() => {
    let cancelled = false
    async function detect() {
      try {
        const available = await ttsInvoke<boolean>('kokoro_available')
        if (cancelled) return
        if (!available) {
          setStatus('unavailable')
          return
        }
        const list = await ttsInvoke<string[]>('kokoro_list_voices')
        if (cancelled) return
        setVoices(list)
        setStatus('ready')
        setVoiceState((current) => (list.length > 0 && !list.includes(current) ? list[0] : current))
      } catch {
        if (!cancelled) setStatus('unavailable')
      }
    }
    void detect()
    return () => {
      cancelled = true
    }
  }, [])

  const setVoice = useCallback((next: string) => {
    setVoiceState(next)
    try {
      localStorage.setItem(KOKORO_VOICE_STORAGE_KEY, next)
    } catch {
      // Best-effort persistence only.
    }
  }, [])

  const setSpeed = useCallback((next: number) => {
    const clamped = Math.min(2, Math.max(0.5, next))
    setSpeedState(clamped)
    try {
      localStorage.setItem(KOKORO_SPEED_STORAGE_KEY, String(clamped))
    } catch {
      // Best-effort persistence only.
    }
  }, [])

  const setHighlightEnabled = useCallback((enabled: boolean) => {
    setHighlightState(enabled)
  }, [])

  const stop = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current)
      rafRef.current = null
    }
    const audio = audioRef.current
    if (audio) {
      audio.pause()
      audio.currentTime = 0
    }
    setIsPlaying(false)
    setIsPaused(false)
    setCurrentTime(0)
    void ttsInvoke('kokoro_stop').catch(() => undefined)
  }, [])

  // Release the object URL and stop playback on unmount.
  useEffect(
    () => () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current)
      audioRef.current?.pause()
      if (objectUrlRef.current) URL.revokeObjectURL(objectUrlRef.current)
      void ttsInvoke('kokoro_stop').catch(() => undefined)
    },
    [],
  )

  // The animation frame schedules itself, so the callback lives in a ref and
  // the effect below keeps it current without a circular useCallback reference.
  const rafTickRef = useRef<() => void>(() => undefined)
  const tick = useCallback(() => {
    const audio = audioRef.current
    if (audio && !audio.paused) {
      setCurrentTime(audio.currentTime)
      rafRef.current = requestAnimationFrame(() => rafTickRef.current())
    } else {
      rafRef.current = null
    }
  }, [])
  useEffect(() => {
    rafTickRef.current = tick
  }, [tick])

  const speak = useCallback(
    async (text: string) => {
      const trimmed = text.trim()
      if (!trimmed || status !== 'ready') return
      stop()
      try {
        const wavPath = await ttsInvoke<string>('kokoro_speak', { text: trimmed, voice, speed })
        const audio = new Audio(isTauri() ? `asset://localhost/${encodeURIComponent(wavPath)}` : wavPath)
        audioRef.current = audio
        audio.playbackRate = speed

        audio.onloadedmetadata = () => setDuration(Number.isFinite(audio.duration) ? audio.duration : 0)
        audio.onended = () => {
          setIsPlaying(false)
          setIsPaused(false)
          setCurrentTime(0)
        }
        audio.onplay = () => {
          setIsPlaying(true)
          setIsPaused(false)
          if (rafRef.current === null) rafRef.current = requestAnimationFrame(tick)
        }
        audio.onpause = () => {
          setIsPaused(true)
          setIsPlaying(false)
          if (rafRef.current !== null) {
            cancelAnimationFrame(rafRef.current)
            rafRef.current = null
          }
        }

        // Word timings are estimated from the reported duration once known.
        audio.oncanplay = () => {
          const durationMs = Number.isFinite(audio.duration) ? audio.duration * 1000 : 0
          void ttsInvoke<WordTiming[]>('kokoro_word_timings', { text: trimmed, speed, duration_ms: durationMs })
            .then((words) => setTimings(words))
            .catch(() => setTimings([]))
        }

        await audio.play()
      } catch {
        setIsPlaying(false)
        setIsPaused(false)
      }
    },
    [speed, status, stop, tick, voice],
  )

  const pause = useCallback(() => {
    audioRef.current?.pause()
  }, [])

  const resume = useCallback(() => {
    void audioRef.current?.play().catch(() => undefined)
  }, [])

  const seek = useCallback((seconds: number) => {
    const audio = audioRef.current
    if (!audio) return
    audio.currentTime = Math.max(0, Math.min(audio.duration || 0, seconds))
    setCurrentTime(audio.currentTime)
  }, [])

  const rewind15 = useCallback(() => {
    const audio = audioRef.current
    if (audio) seek(audio.currentTime - 15)
  }, [seek])

  const forward15 = useCallback(() => {
    const audio = audioRef.current
    if (audio) seek(audio.currentTime + 15)
  }, [seek])

  const { currentWord, currentWordIndex } = useMemo(() => {
    if (!highlightEnabled || timings.length === 0) return { currentWord: null, currentWordIndex: -1 }
    let index = -1
    for (let i = 0; i < timings.length; i += 1) {
      if (timings[i].start <= currentTime) index = i
      else break
    }
    return { currentWord: index >= 0 ? timings[index].word : null, currentWordIndex: index }
  }, [currentTime, highlightEnabled, timings])

  return {
    status,
    voices,
    voice,
    setVoice,
    speed,
    setSpeed,
    highlightEnabled,
    setHighlightEnabled,
    isPlaying,
    isPaused,
    currentTime,
    duration,
    currentWord,
    currentWordIndex,
    speak,
    pause,
    resume,
    stop,
    rewind15,
    forward15,
    seek,
  }
}
