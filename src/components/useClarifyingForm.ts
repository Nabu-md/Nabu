import { useCallback, useEffect, useState } from 'react'
import type { ClarifyingQuestionForm } from '../hooks/useAiActivity'
import {
  CLARIFYING_FORM_AVAILABLE_EVENT,
  takeClarifyingForm,
} from '../utils/clarifyingFormBridge'

/**
 * Picks up clarification forms published by the MCP UI bridge
 * (`ask_clarifying_question` ui_action) so the chat panel can render them.
 */
export function useClarifyingForm(enabled = true): ClarifyingQuestionForm | null {
  const [form, setForm] = useState<ClarifyingQuestionForm | null>(null)

  const consume = useCallback(() => {
    if (!enabled) return
    setForm(takeClarifyingForm())
  }, [enabled])

  useEffect(() => {
    if (!enabled) return
    consume()
    window.addEventListener(CLARIFYING_FORM_AVAILABLE_EVENT, consume)
    return () => window.removeEventListener(CLARIFYING_FORM_AVAILABLE_EVENT, consume)
  }, [consume, enabled])

  return form
}