import { memo, useCallback, useState } from 'react'
import { Question } from '@phosphor-icons/react'
import { createTranslator, type AppLocale } from '../lib/i18n'
import type { ClarifyingQuestionForm } from '../hooks/useAiActivity'
import { queueAiPrompt } from '../utils/aiPromptBridge'
import { clearClarifyingForm } from '../utils/clarifyingFormBridge'

const WS_TOOL_URL = 'ws://localhost:9710'

/**
 * Interactive clarification form rendered inline in the chat panel when an
 * agent calls `ask_clarifying_question`. The chosen answer is queued as the
 * user's next message so the agent resumes from it.
 */
export const AiClarifyingForm = memo(function AiClarifyingForm({
  form,
  locale = 'en',
}: {
  form: ClarifyingQuestionForm
  locale?: AppLocale
}) {
  const t = createTranslator(locale)
  const [selectedOptionId, setSelectedOptionId] = useState<string | null>(null)
  const [customAnswer, setCustomAnswer] = useState('')
  const [rememberChoice, setRememberChoice] = useState(false)
  const [submitted, setSubmitted] = useState(false)

  const answer = useCallback(() => {
    if (selectedOptionId) {
      const option = form.options.find((candidate) => candidate.id === selectedOptionId)
      if (option) return option.label
    }
    return customAnswer.trim()
  }, [customAnswer, form.options, selectedOptionId])

  const submit = useCallback(async () => {
    const value = answer()
    if (!value || submitted) return
    setSubmitted(true)

    if (rememberChoice && form.remember_key) {
      void rememberPreference(form.remember_key, value)
    }

    queueAiPrompt(value, [])
    clearClarifyingForm()
  }, [answer, form.remember_key, rememberChoice, submitted])

  if (submitted) return null

  return (
    <div
      className="mb-2 rounded-xl border border-border bg-background p-3 shadow-xs"
      data-testid="ai-clarifying-form"
    >
      <div className="flex items-center gap-2">
        <Question size={14} className="shrink-0 text-muted-foreground" aria-hidden />
        <span className="min-w-0 flex-1 text-[13px] font-medium text-foreground">{form.question}</span>
      </div>
      <div className="mt-2 flex flex-col gap-1.5" role="radiogroup" aria-label={form.question}>
        {form.options.map((option) => (
          <label
            key={option.id}
            className="flex cursor-pointer items-start gap-2 rounded-md border border-border px-2 py-1.5 text-[13px] leading-5 transition-colors hover:bg-[var(--hover)]"
          >
            <input
              type="radio"
              name={`clarifying-${form.id}`}
              value={option.id}
              checked={selectedOptionId === option.id}
              onChange={() => {
                setSelectedOptionId(option.id)
                setCustomAnswer('')
              }}
              className="mt-0.5 shrink-0 accent-[var(--primary)]"
            />
            <span className="min-w-0 flex-1">
              <span className="block text-foreground">{option.label}</span>
              {option.description && (
                <span className="block text-[11px] leading-4 text-muted-foreground">
                  {option.description}
                </span>
              )}
            </span>
          </label>
        ))}
        {form.allow_custom && (
          <label className="flex cursor-pointer items-start gap-2 rounded-md border border-border px-2 py-1.5 text-[13px] leading-5">
            <input
              type="radio"
              name={`clarifying-${form.id}`}
              value="__custom__"
              checked={!selectedOptionId}
              onChange={() => {
                setSelectedOptionId(null)
              }}
              className="mt-0.5 shrink-0 accent-[var(--primary)]"
            />
            <input
              type="text"
              value={customAnswer}
              placeholder={t('ai.clarifying.other')}
              onChange={(event) => {
                setCustomAnswer(event.currentTarget.value)
                setSelectedOptionId(null)
              }}
              className="min-w-0 flex-1 rounded border border-border bg-background px-2 py-1 text-[13px] text-foreground outline-none focus:border-[var(--primary)]"
            />
          </label>
        )}
      </div>
      <div className="mt-2 flex items-center gap-2">
        {form.remember_key && (
          <label className="flex cursor-pointer items-center gap-1.5 text-[11px] text-muted-foreground">
            <input
              type="checkbox"
              checked={rememberChoice}
              onChange={(event) => setRememberChoice(event.currentTarget.checked)}
              className="shrink-0 accent-[var(--primary)]"
            />
            <span>{t('ai.clarifying.remember')}</span>
          </label>
        )}
        <button
          type="button"
          className="ml-auto rounded-md bg-[var(--primary)] px-3 py-1 text-[12px] font-medium text-[var(--primary-foreground)] transition-colors hover:bg-[var(--primary)]/90 disabled:opacity-50"
          disabled={!answer()}
          onClick={() => void submit()}
          data-testid="ai-clarifying-submit"
        >
          {t('ai.clarifying.next')}
        </button>
      </div>
    </div>
  )
})

function rememberPreference(key: string, value: string): Promise<void> {
  return new Promise((resolve) => {
    try {
      const ws = new WebSocket(WS_TOOL_URL)
      const timer = setTimeout(() => {
        closeSocket(ws)
        resolve()
      }, 1500)
      ws.onopen = () => {
        ws.send(JSON.stringify({
          id: `clarify-${Date.now()}`,
          tool: 'remember_preference',
          args: { key, value },
        }))
      }
      ws.onmessage = () => {
        clearTimeout(timer)
        closeSocket(ws)
        resolve()
      }
      ws.onerror = () => {
        clearTimeout(timer)
        closeSocket(ws)
        resolve()
      }
    } catch {
      resolve()
    }
  })
}

function closeSocket(ws: WebSocket): void {
  try {
    if (ws.readyState === WebSocket.OPEN) ws.close()
  } catch {
    // Best-effort cleanup.
  }
}