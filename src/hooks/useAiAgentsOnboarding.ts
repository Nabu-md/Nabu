import { useCallback, useState } from 'react'

const AI_AGENTS_ONBOARDING_DISMISSED_STORAGE_NAME = 'nabu:ai-agents-onboarding-dismissed'
const LEGACY_CLAUDE_ONBOARDING_DISMISSED_STORAGE_NAME = 'nabu:claude-code-onboarding-dismissed'

function wasDismissed(): boolean {
  try {
    return (
      localStorage.getItem(AI_AGENTS_ONBOARDING_DISMISSED_STORAGE_NAME) === '1'
      || localStorage.getItem(LEGACY_CLAUDE_ONBOARDING_DISMISSED_STORAGE_NAME) === '1'
    )
  } catch {
    return false
  }
}

function markDismissed(): void {
  try {
    localStorage.setItem(AI_AGENTS_ONBOARDING_DISMISSED_STORAGE_NAME, '1')
    localStorage.setItem(LEGACY_CLAUDE_ONBOARDING_DISMISSED_STORAGE_NAME, '1')
  } catch {
    // localStorage may be unavailable in restricted contexts
  }
}

export function useAiAgentsOnboarding(enabled: boolean) {
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
