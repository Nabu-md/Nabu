/**
 * unique-note.ts
 *
 * Generates unique note names based on configurable timestamp format.
 * Used for Zettelkasten-style note creation.
 *
 * Requirements: 27.1, 27.2, 27.3, 27.4, 27.5
 */

/**
 * Generate a unique note name from the current timestamp.
 * Format: YYYYMMDDHHmmss by default.
 */
export function generateUniqueNoteName(
  format: string = 'YYYYMMDDHHmmss',
  now: Date = new Date()
): string {
  const year = now.getFullYear()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  const hour = String(now.getHours()).padStart(2, '0')
  const min = String(now.getMinutes()).padStart(2, '0')
  const sec = String(now.getSeconds()).padStart(2, '0')

  return format
    .replace('YYYY', String(year))
    .replace('MM', month)
    .replace('DD', day)
    .replace('HH', hour)
    .replace('mm', min)
    .replace('ss', sec)
}

/**
 * Parse a template and inject unique-note variables.
 */
export function substituteUniqueNoteVariables(template: string, noteName: string): string {
  return template
    .replace(/\{\{title\}\}/g, noteName)
    .replace(/\{\{date\}\}/g, noteName.slice(0, 8)) // YYYYMMDD
    .replace(/\{\{time\}\}/g, noteName.slice(8)) // HHmmss
}
