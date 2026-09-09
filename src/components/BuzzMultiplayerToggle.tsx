import { useEffect, useMemo, useState } from 'react'
import { Users } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { createTranslator, type AppLocale } from '../lib/i18n'
import { trackEvent } from '../lib/telemetry'
import { useBuzzMultiplayer } from '../hooks/useBuzzMultiplayer'

const BUZZ_CHANNEL_STORAGE_KEY = 'nabu:buzz-team-channel'

interface BuzzMultiplayerToggleProps {
  locale?: AppLocale
  /** Two-way binding so the workspace knows which channel research results go to. */
  channel: string | null
  onChannelChange: (channel: string | null) => void
}

function loadLastChannel(): string {
  try {
    return localStorage.getItem(BUZZ_CHANNEL_STORAGE_KEY) ?? ''
  } catch {
    return ''
  }
}

/**
 * Multiplayer Mode toggle for the deep-research composer controls (plan 4
 * §2.5): reports Buzz CLI/identity readiness, asks for the team channel when
 * enabled, and threads that channel into research requests.
 */
export function BuzzMultiplayerToggle({ locale = 'en', channel, onChannelChange }: BuzzMultiplayerToggleProps) {
  const t = createTranslator(locale)
  const { readiness } = useBuzzMultiplayer()
  const [enabled, setEnabled] = useState(Boolean(channel))
  const [draftChannel, setDraftChannel] = useState(() => channel ?? loadLastChannel())

  // Reflect externally cleared channels (e.g. after a mode reset).
  useEffect(() => {
    setEnabled(Boolean(channel))
  }, [channel])

  const readinessHint = useMemo(() => {
    if (!enabled) return null
    if (readiness === 'checking') return null
    if (readiness === 'not_installed') return t('buzz.notInstalled')
    if (readiness === 'no_identity') return t('buzz.noIdentity')
    return null
  }, [enabled, readiness, t])

  const commitChannel = (next: string) => {
    const trimmed = next.trim()
    setDraftChannel(trimmed)
    if (trimmed) {
      try {
        localStorage.setItem(BUZZ_CHANNEL_STORAGE_KEY, trimmed)
      } catch {
        // Best-effort persistence only.
      }
    }
    onChannelChange(trimmed || null)
  }

  const toggle = () => {
    const next = !enabled
    setEnabled(next)
    trackEvent(next ? 'buzz_multiplayer_enabled' : 'buzz_multiplayer_disabled')
    if (next) {
      commitChannel(draftChannel)
    } else {
      onChannelChange(null)
    }
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <Button
        type="button"
        variant={enabled ? 'default' : 'ghost'}
        size="sm"
        className={cn('gap-1.5', !enabled && 'text-muted-foreground')}
        aria-pressed={enabled}
        title={t('buzz.multiplayer')}
        data-testid="buzz-multiplayer-toggle"
        onClick={toggle}
      >
        <Users size={14} weight={enabled ? 'fill' : 'regular'} aria-hidden />
        <span className="hidden text-[12px] lg:inline">{t('buzz.multiplayer')}</span>
      </Button>
      {enabled && (
        <div className="flex w-56 flex-col items-end gap-0.5">
          <Input
            value={draftChannel}
            onChange={(event) => commitChannel(event.target.value)}
            placeholder={t('buzz.teamChannelPlaceholder')}
            aria-label={t('buzz.teamChannel')}
            className="h-7 text-[12px]"
            data-testid="buzz-team-channel-input"
          />
          {readinessHint && (
            <span className="max-w-56 text-right text-[10px] leading-tight text-muted-foreground" data-testid="buzz-readiness-hint">
              {readinessHint}
            </span>
          )}
        </div>
      )}
    </div>
  )
}
