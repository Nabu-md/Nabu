/**
 * Interactive clarification forms — `ask_clarifying_question`.
 *
 * When an agent detects ambiguity in a request, it can call
 * `ask_clarifying_question` to present a structured form (2-3 suggested
 * options + one custom input) directly in the chat panel instead of asking a
 * free-text question. The chosen answer arrives back as the user's next
 * message; the agent resumes from there.
 */
import { randomUUID } from 'node:crypto'

/** Modes in which agents are allowed to present clarification forms. */
export const CLARIFYING_QUESTION_MODES = ['deep_research', 'rag', 'mini_app_builder']

const MAX_OPTIONS = 3

/**
 * Validate and normalize a clarification form request.
 * @param {{question: string, options?: Array<{label: string, description?: string}>, mode?: string, rememberKey?: string}} args
 * @returns {object} The form schema to render in the chat panel.
 */
export function buildClarifyingForm(args = {}) {
  const question = typeof args.question === 'string' ? args.question.trim() : ''
  if (!question) throw new Error('question is required')

  const mode = typeof args.mode === 'string' ? args.mode.trim() : ''
  if (mode && !CLARIFYING_QUESTION_MODES.includes(mode)) {
    throw new Error(
      `ask_clarifying_question is not available in ${mode} mode. ` +
        `It is gated to: ${CLARIFYING_QUESTION_MODES.join(', ')}.`,
    )
  }

  let options = Array.isArray(args.options) ? args.options : []
  if (options.length > MAX_OPTIONS) options = options.slice(0, MAX_OPTIONS)
  const normalizedOptions = options
    .map((option, index) => {
      const label = typeof option?.label === 'string' ? option.label.trim() : ''
      if (!label) return null
      return {
        id: typeof option?.id === 'string' && option.id.trim() ? option.id.trim() : `option-${index + 1}`,
        label,
        description: typeof option?.description === 'string' && option.description.trim()
          ? option.description.trim()
          : undefined,
      }
    })
    .filter((option) => option !== null)
  if (normalizedOptions.length === 0) {
    throw new Error('options must contain at least one labeled choice')
  }

  const rememberKey = typeof args.rememberKey === 'string' && args.rememberKey.trim()
    ? args.rememberKey.trim()
    : null

  return {
    id: randomUUID(),
    question,
    mode: mode || null,
    options: normalizedOptions,
    allow_custom: true,
    remember_key: rememberKey,
  }
}