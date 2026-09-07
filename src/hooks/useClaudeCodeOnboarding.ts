import { useCallback, useState } from 'react'

const CLAUDE_CODE_ONBOARDING_DISMISSED_STORAGE_NAME = 'nabu:claude-code-onboarding-dismissed'

function wasDismissed(): boolean {
  try {
    return localStorage.getItem(CLAUDE_CODE_ONBOARDING_DISMISSED_STORAGE_NAME) === '1'
  } catch {
    return false
  }
}

function markDismissed(): void {
  try {
    localStorage.setItem(CLAUDE_CODE_ONBOARDING_DISMISSED_STORAGE_NAME, '1')
  } catch {
    // localStorage may be unavailable in restricted contexts
  }
}

export function useClaudeCodeOnboarding(enabled: boolean) {
  const [dismissed, setDismissed] = useState(() => wasDismissed())

  const dismissPrompt = useCallback(() => {
    markDismissed()
    setDismissed(true)
  }, [])

  return {
    dismissPrompt,
    showPrompt: enabled && !dismissed,
  }
}
