import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

function grammarInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

export interface GrammarIssue {
  message: string
  suggestions: string[]
  start: number
  end: number
  severity: 'error' | 'warning' | 'info'
}

export interface GrammarCheckState {
  issues: GrammarIssue[]
  checking: boolean
  checkGrammar: (text: string) => Promise<GrammarIssue[]>
  addCustomWord: (word: string) => Promise<void>
  scheduleCheck: (text: string) => void
}

const DEFAULT_DEBOUNCE_MS = 800

/**
 * Harper grammar checking hook (offline, in-process). Debounced by default so
 * typing stays smooth; custom words are merged into every check.
 */
export function useGrammarCheck(
  options: { automatic?: boolean; debounceMs?: number; extraWords?: string[] } = {},
): GrammarCheckState {
  const { automatic = true, debounceMs = DEFAULT_DEBOUNCE_MS } = options
  const extraWordsRef = useRef<string[]>(options.extraWords ?? [])
  extraWordsRef.current = options.extraWords ?? []
  const [issues, setIssues] = useState<GrammarIssue[]>([])
  const [checking, setChecking] = useState(false)
  const customWordsRef = useRef<Set<string>>(new Set())

  const checkGrammar = useCallback(async (text: string) => {
    if (!text.trim()) {
      setIssues([])
      return []
    }
    setChecking(true)
    try {
      const words = [...new Set([...customWordsRef.current, ...extraWordsRef.current])]
      const result = await grammarInvoke<GrammarIssue[]>('grammar_check', {
        text,
        extraWords: words,
      })
      setIssues(result)
      return result
    } finally {
      setChecking(false)
    }
  }, [])

  // Debounced automatic checking of the latest text.
  const latestTextRef = useRef<string>('')
  const timerRef = useRef<number | null>(null)
  const scheduleCheck = useCallback(
    (text: string) => {
      latestTextRef.current = text
      if (!automatic) return
      if (timerRef.current !== null) window.clearTimeout(timerRef.current)
      timerRef.current = window.setTimeout(() => {
        void checkGrammar(latestTextRef.current)
      }, debounceMs)
    },
    [automatic, checkGrammar, debounceMs],
  )

  useEffect(
    () => () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current)
    },
    [],
  )

  const addCustomWord = useCallback(async (word: string) => {
    const trimmed = word.trim()
    if (!trimmed) return
    customWordsRef.current.add(trimmed)
  }, [])

  return {
    issues,
    checking,
    checkGrammar,
    addCustomWord,
    scheduleCheck,
  }
}
