import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { useClaudeCodeOnboarding } from './useClaudeCodeOnboarding'

const localStorageMock = (() => {
  let store: Record<string, string> = {}
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => { store[key] = value },
    removeItem: (key: string) => { delete store[key] },
    clear: () => { store = {} },
  }
})()

Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock, writable: true })

const DISMISSED_STORAGE_NAME = 'nabu:claude-code-onboarding-dismissed'

describe('useClaudeCodeOnboarding', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('shows the prompt when enabled and not dismissed', () => {
    const { result } = renderHook(() => useClaudeCodeOnboarding(true))

    expect(result.current.showPrompt).toBe(true)
  })

  it('hides the prompt when onboarding is disabled', () => {
    const { result } = renderHook(() => useClaudeCodeOnboarding(false))

    expect(result.current.showPrompt).toBe(false)
  })

  it('starts hidden when the prompt was already dismissed', () => {
    localStorage.setItem(DISMISSED_STORAGE_NAME, '1')

    const { result } = renderHook(() => useClaudeCodeOnboarding(true))

    expect(result.current.showPrompt).toBe(false)
  })

  it('persists dismissal and hides the prompt', () => {
    const { result } = renderHook(() => useClaudeCodeOnboarding(true))

    act(() => {
      result.current.dismissPrompt()
    })

    expect(result.current.showPrompt).toBe(false)
    expect(localStorage.getItem(DISMISSED_STORAGE_NAME)).toBe('1')
  })
})
