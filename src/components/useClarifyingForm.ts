import { useCallback, useSyncExternalStore } from 'react'
import type { ClarifyingQuestionForm } from '../hooks/useAiActivity'
import {
  getClarifyingFormSnapshot,
  subscribeClarifyingForm,
} from '../utils/clarifyingFormBridge'

const EMPTY: ClarifyingQuestionForm | null = null

function subscribe(onChange: () => void): () => void {
  return subscribeClarifyingForm(onChange)
}

/**
 * Picks up clarification forms published by the MCP UI bridge
 * (`ask_clarifying_question` ui_action) so the chat panel can render them.
 * The form is a shared external store: publishing shows it, submitting or
 * dismissing clears it for every panel instance.
 */
export function useClarifyingForm(enabled = true): ClarifyingQuestionForm | null {
  const form = useSyncExternalStore(subscribe, getClarifyingFormSnapshot, () => EMPTY)
  return enabled ? form : null
}

/** Convenience wrapper so components can dismiss without importing the bridge. */
export function useDismissClarifyingForm(): () => void {
  return useCallback(() => {
    // Imported lazily to keep this hook's surface minimal.
    void import('../utils/clarifyingFormBridge').then((m) => m.clearClarifyingForm())
  }, [])
}
