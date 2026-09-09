import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'
import { trackEvent } from '../lib/telemetry'
import type { useKokoroTts } from './useKokoroTts'

type Tts = ReturnType<typeof useKokoroTts>

function ttsInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

/** Strips frontmatter and markdown syntax so Kokoro reads prose, not markup. */
export function stripMarkdownForSpeech(markdown: string): string {
  let text = markdown.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, '')
  text = text
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, ' ')
    .replace(/\[\[([^\]|]*)(\|[^\]]*)?\]\]/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}#{1,6}\s+/gm, '')
    .replace(/^\s{0,3}>\s?/gm, '')
    .replace(/^\s*([-*+]|\d+\.)\s+/gm, '')
    .replace(/\|/g, ' ')
    .replace(/[*_~]+/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/https?:\/\/\S+/g, ' ')
    .replace(/\(\s*\)/g, '')
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
  return text.trim()
}

interface UseTtsTextSourceOptions {
  tts: Tts
  getText: () => string
  /** Test id prefix, e.g. 'note' -> 'note-tts-speaker'. */
  sourceId: string
  enabled?: boolean
}

/**
 * Shared speaker-button behavior for TTS surfaces (plan 4 §1.6): lazily
 * extracts text on demand, starts Kokoro playback, and auto-pauses on blur.
 * Returns a handler plus whether playback is currently active for this source.
 */
export function useTtsTextSource({ tts, getText, sourceId, enabled = true }: UseTtsTextSourceOptions) {
  const getTextRef = useRef(getText)
  getTextRef.current = getText
  const activeRef = useRef(false)

  useEffect(() => {
    if (!enabled) return
    const handleBlur = () => {
      if (activeRef.current && tts.isPlaying) tts.pause()
    }
    const handleFocus = () => {
      if (activeRef.current && tts.isPaused) tts.resume()
    }
    window.addEventListener('blur', handleBlur)
    window.addEventListener('focus', handleFocus)
    return () => {
      window.removeEventListener('blur', handleBlur)
      window.removeEventListener('focus', handleFocus)
    }
  }, [enabled, tts])

  useEffect(() => {
    if (!tts.isPlaying) activeRef.current = false
  }, [tts.isPlaying])

  const speak = async () => {
    if (tts.isPlaying || tts.isPaused) {
      tts.stop()
      return
    }
    const text = stripMarkdownForSpeech(getTextRef.current())
    if (!text) return
    activeRef.current = true
    trackEvent('tts_speak_started', { source: sourceId })
    await tts.speak(text)
  }

  return { speak, active: tts.isPlaying || tts.isPaused }
}
