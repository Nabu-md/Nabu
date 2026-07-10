/**
 * templates.ts
 *
 * Pure utility for substituting template variables in Nabu template files.
 * Templates are `.md` files located inside `_templates/` at the vault root
 * and may contain `{{title}}`, `{{date}}`, and `{{time}}` placeholders.
 *
 * Requirements: 9.5
 */

/**
 * Substitute all occurrences of `{{title}}`, `{{date}}`, and `{{time}}`
 * in the given template string with the corresponding values from `vars`.
 *
 * - Uses global regex replacement so every occurrence is replaced, not just
 *   the first.
 * - `vars.date` and `vars.time` are passed in as pre-formatted strings
 *   (e.g. `"2025-01-15"` and `"14:30"`); this function performs no `Date`
 *   calls itself, keeping it a pure transformation.
 *
 * @param template - The raw template content containing substitution variables.
 * @param vars     - The values to inject into the template.
 * @returns The template content with all variables replaced.
 */
export function substituteVariables(
  template: string,
  vars: { title: string; date: string; time: string }
): string {
  // Use replacer functions to avoid interpreting $&, $1, etc. in the values
  return template
    .replace(/\{\{title\}\}/g, () => vars.title)
    .replace(/\{\{date\}\}/g, () => vars.date)
    .replace(/\{\{time\}\}/g, () => vars.time)
}
