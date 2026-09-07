import { useState, type CSSProperties } from 'react'
import { ShieldCheck } from '@phosphor-icons/react'
import { OnboardingShell } from './OnboardingShell'
import { Button } from './ui/button'
import { DEFAULT_APP_LOCALE, translate, type AppLocale } from '../lib/i18n'

interface TelemetryConsentDialogProps {
  locale?: AppLocale
  onAccept: () => Promise<boolean>
  onDecline: () => Promise<boolean>
}

const telemetryConsentContentStyle: CSSProperties = {
  width: 'min(440px, 100%)',
  padding: 32,
  display: 'flex',
  flexDirection: 'column',
  gap: 20,
  alignItems: 'center',
}

export function TelemetryConsentDialog({
  locale = DEFAULT_APP_LOCALE,
  onAccept,
  onDecline,
}: TelemetryConsentDialogProps) {
  const [isSaving, setIsSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

  const handleChoice = async (choice: () => Promise<boolean>) => {
    setIsSaving(true)
    setSaveError(null)
    try {
      if (!await choice()) {
        setSaveError(translate(locale, 'telemetryConsent.saveError'))
      }
    } catch {
      setSaveError(translate(locale, 'telemetryConsent.saveError'))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <OnboardingShell
      className="fixed inset-0 z-50"
      contentClassName="w-full rounded-lg border border-border bg-background shadow-[0_18px_55px_var(--shadow-dialog)]"
      style={{ background: 'var(--shadow-overlay)' }}
      contentStyle={telemetryConsentContentStyle}
      testId="telemetry-consent-shell"
    >

        <ShieldCheck size={40} weight="duotone" style={{ color: 'var(--primary)' }} />

        <div style={{ textAlign: 'center' }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, color: 'var(--foreground)', margin: 0 }}>
            Help improve Nabu
          </h2>
          <p style={{ fontSize: 13, color: 'var(--muted-foreground)', lineHeight: 1.6, marginTop: 8 }}>
            Send anonymous crash reports to help us fix bugs faster.
            No vault content, no personal data, no tracking.
          </p>
        </div>

        <div style={{ fontSize: 12, color: 'var(--muted-foreground)', lineHeight: 1.6, width: '100%' }}>
          <p style={{ margin: '0 0 6px', fontWeight: 500, color: 'var(--foreground)' }}>What we collect:</p>
          <ul style={{ margin: 0, paddingLeft: 18 }}>
            <li>Stack traces from errors (JS &amp; Rust)</li>
            <li>App version, OS, and architecture</li>
          </ul>
          <p style={{ margin: '10px 0 6px', fontWeight: 500, color: 'var(--foreground)' }}>What we never collect:</p>
          <ul style={{ margin: 0, paddingLeft: 18 }}>
            <li>No vault content, note titles, or file paths</li>
            <li>No personal data or IP addresses</li>
          </ul>
        </div>

        <div style={{ display: 'flex', gap: 12, width: '100%', marginTop: 4 }}>
          <Button type="button" variant="outline"
            style={{ flex: 1, fontSize: 13, padding: '10px 16px' }}
            onClick={() => void handleChoice(onDecline)}
            disabled={isSaving}
            data-testid="telemetry-decline"
            autoFocus>
            No thanks
          </Button>
          <Button type="button"
            style={{ flex: 1, fontSize: 13, padding: '10px 16px', fontWeight: 500 }}
            onClick={() => void handleChoice(onAccept)}
            disabled={isSaving}
            data-testid="telemetry-accept">
            Allow anonymous reporting
          </Button>
        </div>

        {saveError && (
          <p
            role="alert"
            style={{ fontSize: 12, color: 'var(--destructive)', lineHeight: 1.5, margin: 0, textAlign: 'center' }}
          >
            {saveError}
          </p>
        )}

        <p style={{ fontSize: 11, color: 'var(--muted-foreground)', margin: 0, textAlign: 'center' }}>
          You can change this anytime in Settings.
        </p>

    </OnboardingShell>
  )
}
