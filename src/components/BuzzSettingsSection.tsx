import { useCallback, useEffect, useState } from 'react'
import { ArrowsClockwise, Users } from '@phosphor-icons/react'
import type { AppLocale, TranslationKey, TranslationValues } from '../lib/i18n'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { SectionHeading, SettingsGroup, SettingsRow, SettingsSwitchRow } from './SettingsControls'
import { trackEvent } from '../lib/telemetry'
import { trackBuzzTeamContextFetched } from '../lib/productAnalytics'
import { useBuzzMultiplayer, type BuzzMessage } from '../hooks/useBuzzMultiplayer'

type Translate = (key: TranslationKey, values?: TranslationValues) => string

interface BuzzSettingsSectionProps {
  t: Translate
  locale: AppLocale
  buzzEnabled: boolean
  setBuzzEnabled: (value: boolean) => void
  buzzChannel: string
  setBuzzChannel: (value: string) => void
}

function formatMessageTime(timestamp: number, locale: string): string {
  try {
    return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(timestamp * 1000))
  } catch {
    return new Date(timestamp * 1000).toISOString()
  }
}

/** Buzz multiplayer section: readiness, default channel, and team message reader. */
export function BuzzSettingsSection(props: BuzzSettingsSectionProps) {
  const { t, locale, buzzEnabled, setBuzzEnabled, buzzChannel, setBuzzChannel } = props
  const { readiness, fetchMessages } = useBuzzMultiplayer()
  const [messages, setMessages] = useState<BuzzMessage[]>([])
  const [loading, setLoading] = useState(false)

  const loadMessages = useCallback(() => {
    if (!buzzChannel.trim() || readiness !== 'ready') return
    setLoading(true)
    fetchMessages(buzzChannel.trim())
      .then((fetched) => setMessages(fetched))
      .catch(() => setMessages([]))
      .finally(() => setLoading(false))
    trackBuzzTeamContextFetched('settings')
  }, [buzzChannel, readiness, fetchMessages])

  useEffect(() => {
    if (buzzEnabled && buzzChannel.trim() && readiness === 'ready') {
      loadMessages()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refresh on readiness transitions only
  }, [buzzEnabled, buzzChannel, readiness])

  const readinessHint = readiness === 'checking'
    ? null
    : readiness === 'not_installed'
      ? t('buzz.notInstalled')
      : readiness === 'no_identity'
        ? t('buzz.noIdentity')
        : null

  const handleToggle = (next: boolean) => {
    setBuzzEnabled(next)
    trackEvent(next ? 'buzz_multiplayer_enabled' : 'buzz_multiplayer_disabled')
  }

  return (
    <>
      <SectionHeading icon={<Users size={16} aria-hidden="true" />} title={t('settings.buzz.title')} />
      <SettingsGroup>
        <SettingsSwitchRow
          label={t('buzz.multiplayer')}
          description={t('buzz.multiplayerDescription')}
          checked={buzzEnabled}
          onChange={handleToggle}
          testId="settings-buzz-enabled"
        />
        {buzzEnabled && (
          <SettingsRow label={t('buzz.teamChannel')} description={t('settings.buzz.channelDescription')}>
            <Input
              value={buzzChannel}
              onChange={(event) => setBuzzChannel(event.target.value)}
              placeholder={t('buzz.teamChannelPlaceholder')}
              aria-label={t('buzz.teamChannel')}
              data-testid="settings-buzz-channel"
              className="bg-background"
            />
          </SettingsRow>
        )}
        {buzzEnabled && readinessHint && (
          <p className="px-3 text-[12px] text-muted-foreground" data-testid="settings-buzz-readiness">
            {readinessHint}
          </p>
        )}
      </SettingsGroup>

      {buzzEnabled && (
        <SettingsGroup>
          <SettingsRow
            label={t('settings.buzz.teamMessages')}
            description={t('settings.buzz.teamMessagesDescription')}
          >
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="gap-1.5"
              disabled={!buzzChannel.trim() || readiness !== 'ready' || loading}
              aria-label={t('settings.buzz.refreshMessages')}
              data-testid="settings-buzz-refresh-messages"
              onClick={loadMessages}
            >
              <ArrowsClockwise size={14} className={loading ? 'animate-spin' : undefined} aria-hidden />
              {t('settings.buzz.refreshMessages')}
            </Button>
          </SettingsRow>
          {messages.length > 0 && (
            <div className="rounded-md border border-border" data-testid="settings-buzz-messages">
              {messages.map((message, index) => (
                <BuzzMessageRow
                  key={`${message.created_at}-${index}`}
                  message={message}
                  locale={locale}
                />
              ))}
            </div>
          )}
        </SettingsGroup>
      )}
    </>
  )
}

function BuzzMessageRow({ message, locale }: { message: BuzzMessage; locale: string }) {
  const author = message.author.length > 12 ? `${message.author.slice(0, 12)}…` : message.author
  return (
    <div className="border-b border-border px-3 py-2 last:border-b-0">
      <div className="flex items-baseline justify-between gap-2">
        <span className="truncate font-mono text-[11px] text-muted-foreground">{author}</span>
        <span className="shrink-0 text-[10px] text-muted-foreground">
          {formatMessageTime(message.created_at, locale)}
        </span>
      </div>
      <p className="mt-0.5 whitespace-pre-wrap break-words text-[12px] leading-5 text-foreground">
        {message.content}
      </p>
    </div>
  )
}
