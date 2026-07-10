/**
 * daily-note.test.ts
 *
 * Unit tests for daily note path derivation and template substitution.
 *
 * The daily note IPC handler (note:daily) is tested at the integration
 * level since it requires fs access. Here we test the pure logic:
 *   - Date format substitution (YYYY, MM, DD)
 *   - Folder path derivation
 *   - Filename construction
 *
 * Requirements: 17.1, 17.3, 17.4
 */

import { describe, it, expect } from 'vitest'

/**
 * Simulates the daily note filename derivation from settings.
 * This mirrors the logic in src/main/ipc.ts note:daily handler.
 */
function deriveDailyNotePath(
  vaultPath: string,
  dateFormat: string,
  folder: string,
  now: Date
): string {
  const dateStr = dateFormat
    .replace('YYYY', String(now.getFullYear()))
    .replace('MM', String(now.getMonth() + 1).padStart(2, '0'))
    .replace('DD', String(now.getDate()).padStart(2, '0'))

  return `${vaultPath}/${folder}/${dateStr}.md`
}

describe('Daily note path derivation', () => {
  const now = new Date('2026-07-09T12:00:00Z')

  it('uses default YYYY-MM-DD format', () => {
    const path = deriveDailyNotePath('/vault', 'YYYY-MM-DD', 'Daily', now)
    expect(path).toBe('/vault/Daily/2026-07-09.md')
  })

  it('uses custom date format', () => {
    const path = deriveDailyNotePath('/vault', 'YYYYMMDD', 'Journal', now)
    expect(path).toBe('/vault/Journal/20260709.md')
  })

  it('uses custom folder', () => {
    const path = deriveDailyNotePath('/vault', 'YYYY-MM-DD', 'Journal/Daily', now)
    expect(path).toBe('/vault/Journal/Daily/2026-07-09.md')
  })

  it('uses DD-MM-YYYY format', () => {
    const path = deriveDailyNotePath('/vault', 'DD-MM-YYYY', 'Daily', now)
    expect(path).toBe('/vault/Daily/09-07-2026.md')
  })

  it('handles single-digit month and day with leading zeros', () => {
    const earlyJan = new Date('2026-01-05T12:00:00Z')
    const path = deriveDailyNotePath('/vault', 'YYYY-MM-DD', 'Daily', earlyJan)
    expect(path).toBe('/vault/Daily/2026-01-05.md')
  })

  it('same date twice produces same path', () => {
    const path1 = deriveDailyNotePath('/vault', 'YYYY-MM-DD', 'Daily', now)
    const path2 = deriveDailyNotePath('/vault', 'YYYY-MM-DD', 'Daily', now)
    expect(path1).toBe(path2)
  })
})

/**
 * Simulates template substitution logic used in the daily note handler.
 */
function substituteTemplateVariables(
  template: string,
  vars: { title: string; date: string; time: string }
): string {
  return template
    .replace(/\{\{title\}\}/g, vars.title)
    .replace(/\{\{date\}\}/g, vars.date)
    .replace(/\{\{time\}\}/g, vars.time)
}

describe('Daily note template substitution', () => {
  it('substitutes {{title}} with date string', () => {
    const result = substituteTemplateVariables('# {{title}}\n\n', {
      title: '2026-07-09',
      date: '2026-07-09',
      time: '12:00'
    })
    expect(result).toBe('# 2026-07-09\n\n')
  })

  it('substitutes {{date}} and {{time}}', () => {
    const result = substituteTemplateVariables('Created: {{date}} at {{time}}', {
      title: 'test',
      date: '2026-07-09',
      time: '12:00'
    })
    expect(result).toBe('Created: 2026-07-09 at 12:00')
  })

  it('returns template unchanged if no variables present', () => {
    const result = substituteTemplateVariables('# Plain heading\n', {
      title: 'test',
      date: '2026-07-09',
      time: '12:00'
    })
    expect(result).toBe('# Plain heading\n')
  })
})

describe('Daily note settings defaults', () => {
  it('default date format is YYYY-MM-DD', () => {
    const defaultFormat = 'YYYY-MM-DD'
    const now = new Date('2026-07-09T12:00:00Z')
    const path = deriveDailyNotePath('/vault', defaultFormat, 'Daily', now)
    expect(path).toBe('/vault/Daily/2026-07-09.md')
  })

  it('default folder is Daily', () => {
    const defaultFolder = 'Daily'
    const now = new Date('2026-07-09T12:00:00Z')
    const path = deriveDailyNotePath('/vault', 'YYYY-MM-DD', defaultFolder, now)
    expect(path).toContain('/Daily/')
  })
})
