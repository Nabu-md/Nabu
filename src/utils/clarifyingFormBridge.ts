import type { ClarifyingQuestionForm } from '../hooks/useAiActivity'

export const CLARIFYING_FORM_AVAILABLE_EVENT = 'nabu:clarifying-form-available'

let pendingForm: ClarifyingQuestionForm | null = null
const listeners = new Set<() => void>()

function notifyListeners(): void {
  for (const listener of listeners) listener()
}

export function publishClarifyingForm(form: ClarifyingQuestionForm): void {
  pendingForm = form
  window.dispatchEvent(new Event(CLARIFYING_FORM_AVAILABLE_EVENT))
  notifyListeners()
}

export function getClarifyingFormSnapshot(): ClarifyingQuestionForm | null {
  return pendingForm
}

export function subscribeClarifyingForm(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function clearClarifyingForm(): void {
  if (!pendingForm) return
  pendingForm = null
  notifyListeners()
}
