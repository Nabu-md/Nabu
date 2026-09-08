import type { ClarifyingQuestionForm } from '../hooks/useAiActivity'

export const CLARIFYING_FORM_AVAILABLE_EVENT = 'nabu:clarifying-form-available'

let pendingForm: ClarifyingQuestionForm | null = null

export function publishClarifyingForm(form: ClarifyingQuestionForm): void {
  pendingForm = form
  window.dispatchEvent(new Event(CLARIFYING_FORM_AVAILABLE_EVENT))
}

export function takeClarifyingForm(): ClarifyingQuestionForm | null {
  const form = pendingForm
  pendingForm = null
  return form
}

export function clearClarifyingForm(): void {
  pendingForm = null
}