import type { TranslationKey } from '../lib/i18n'
import type { createTranslator } from '../lib/i18n'
import { SettingsSection } from './SettingsControls'

type Translate = ReturnType<typeof createTranslator>

const OPEN_SOURCE_CREDITS: Array<{ name: string; url: string; license: string; role: string }> = [
  { name: 'anydoc', url: 'https://crates.io/crates/anydoc', license: 'MIT', role: 'Document conversion' },
  { name: 'Harper', url: 'https://github.com/automattic/harper', license: 'Apache-2.0', role: 'Grammar checking' },
  { name: 'FluidVoice', url: 'https://github.com/altic-dev/FluidVoice', license: 'GPLv3', role: 'On-device dictation' },
  { name: 'Tauri', url: 'https://tauri.app', license: 'Apache-2.0', role: 'Desktop framework' },
  { name: 'React', url: 'https://react.dev', license: 'MIT', role: 'UI library' },
  { name: 'BlockNote', url: 'https://www.blocknotejs.org', license: 'MPL-2.0', role: 'Rich text editor' },
  { name: 'vis-network', url: 'https://visjs.github.io/vis-network/', license: 'Apache-2.0', role: 'Graph view' },
]

/** Settings → About: open-source attribution for bundled/integrated projects. */
export function AboutSection({ t }: { t: Translate }) {
  void t
  return (
    <SettingsSection id="settings-section-about" showDivider={false}>
      <h3 style={{ fontSize: 14, fontWeight: 600, color: 'var(--foreground)' }}>
        About Nabu
      </h3>
      <p style={{ fontSize: 12, color: 'var(--muted-foreground)' }}>
        Nabu is licensed under AGPL-3.0-or-later and stands on the shoulders of
        these open-source projects. Full details in{' '}
        <a href="https://github.com/refactoringhq/nabu/blob/main/NOTICE.md" target="_blank" rel="noreferrer" style={{ textDecoration: 'underline' }}>
          NOTICE.md
        </a>
        .
      </p>
      <table style={{ fontSize: 12, borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            <th className="pr-4 pb-1 text-left" style={{ color: 'var(--muted-foreground)' }}>Project</th>
            <th className="pr-4 pb-1 text-left" style={{ color: 'var(--muted-foreground)' }}>License</th>
            <th className="pb-1 text-left" style={{ color: 'var(--muted-foreground)' }}>What it does</th>
          </tr>
        </thead>
        <tbody>
          {OPEN_SOURCE_CREDITS.map((credit) => (
            <tr key={credit.name}>
              <td className="pr-4 py-1">
                <a href={credit.url} target="_blank" rel="noreferrer" style={{ textDecoration: 'underline' }}>
                  {credit.name}
                </a>
              </td>
              <td className="pr-4 py-1" style={{ color: 'var(--muted-foreground)' }}>{credit.license}</td>
              <td className="py-1" style={{ color: 'var(--muted-foreground)' }}>{credit.role}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </SettingsSection>
  )
}

export type AboutSectionTranslationKey = TranslationKey
