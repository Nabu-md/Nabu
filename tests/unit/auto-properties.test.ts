/**
 * Unit tests for the auto-properties logic — injecting `created` on note
 * creation and `modified` on note save, with frontmatter creation when none.
 *
 * These functions mirror the injectAutoProperty logic in ipc.ts so they can
 * be tested without importing Electron's main process.
 *
 * Requirements: 16.1, 16.2, 16.3, 16.4, 16.5, 16.6
 */

import { describe, it, expect } from 'vitest'
import { parse, stringify } from 'yaml'

// ---------------------------------------------------------------------------
// Frontmatter regex — mirrors FRONTMATTER_RE in ipc.ts
// ---------------------------------------------------------------------------

// Built from a string to avoid any literal-escaping ambiguity across the
// esbuild/vitest transform pipeline.
const FRONTMATTER_RE = new RegExp('^---\\n[\\s\\S]*?\\n---(?:\\n|$)')

// ---------------------------------------------------------------------------
// Frontmatter helpers — mirrors extractFrontmatter / replaceFrontmatterRaw
// ---------------------------------------------------------------------------

interface FrontmatterResult {
  yaml: string
  parsed: Record<string, unknown>
}

function extractFrontmatter(content: string): FrontmatterResult {
  const match = content.match(FRONTMATTER_RE)
  if (!match) {
    return { yaml: '', parsed: {} }
  }
  const yamlStr = match[0].replace(/^---\n/, '').replace(/\n---(?:\n|$)/, '')
  try {
    const parsed = parse(yamlStr)
    return {
      yaml: yamlStr,
      parsed:
        typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
          ? (parsed as Record<string, unknown>)
          : {}
    }
  } catch {
    return { yaml: yamlStr, parsed: {} }
  }
}

function replaceFrontmatterRaw(raw: string, yamlStr: string): string {
  if (!yamlStr.trim()) {
    return raw.replace(FRONTMATTER_RE, '')
  }
  const yamlBlock = `---\n${yamlStr.trim()}\n---\n`
  if (FRONTMATTER_RE.test(raw)) {
    return raw.replace(FRONTMATTER_RE, yamlBlock)
  }
  return yamlBlock + raw
}

/**
 * Inject or update a single frontmatter property — mirrors injectAutoProperty
 * in ipc.ts. When `onlyIfAbsent` is true, the value is only set if the key
 * does not already exist.
 */
function injectAutoProperty(
  content: string,
  key: string,
  value: string,
  onlyIfAbsent: boolean
): string {
  const { parsed } = extractFrontmatter(content)
  if (onlyIfAbsent && key in parsed) {
    return content
  }
  const updated = { ...parsed, [key]: value }
  const newYaml = stringify(updated)
  return replaceFrontmatterRaw(content, newYaml)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('injectAutoProperty — created (onlyIfAbsent: true)', () => {
  const iso = '2024-01-15T10:30:00.000Z'

  it('creates minimal frontmatter when none exists (Req 16.1, 16.4)', () => {
    const content = '# My Note\n\nSome body text.\n'
    const result = injectAutoProperty(content, 'created', iso, true)
    // Should now start with frontmatter
    expect(result.startsWith('---\n')).toBe(true)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.created).toBe(iso)
  })

  it('adds created to existing frontmatter that lacks it (Req 16.1)', () => {
    const content = '---\ntitle: Existing\n---\n\nBody.\n'
    const result = injectAutoProperty(content, 'created', iso, true)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.created).toBe(iso)
    expect(parsed.title).toBe('Existing') // preserved
  })

  it('does NOT overwrite existing created (Req 16.3)', () => {
    const userSet = '2020-01-01T00:00:00.000Z'
    const content = `---\ncreated: ${userSet}\n---\n\nBody.\n`
    const result = injectAutoProperty(content, 'created', iso, true)
    // Content should be unchanged
    expect(result).toBe(content)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.created).toBe(userSet)
  })

  it('preserves body content after injection', () => {
    const body = '# Heading\n\nA paragraph.\n\n- list item\n'
    const result = injectAutoProperty(body, 'created', iso, true)
    // Body should be preserved after the frontmatter block
    expect(result).toContain('# Heading')
    expect(result).toContain('A paragraph.')
    expect(result).toContain('- list item')
  })

  it('handles empty content (Req 16.4)', () => {
    const result = injectAutoProperty('', 'created', iso, true)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.created).toBe(iso)
  })

  it('handles content with only a heading and no body', () => {
    const result = injectAutoProperty('# Just a heading\n', 'created', iso, true)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.created).toBe(iso)
    expect(result).toContain('# Just a heading')
  })
})

describe('injectAutoProperty — modified (onlyIfAbsent: false)', () => {
  const iso = '2024-06-20T12:00:00.000Z'

  it('creates minimal frontmatter when none exists (Req 16.2, 16.4)', () => {
    const content = '# Note\n\nBody.\n'
    const result = injectAutoProperty(content, 'modified', iso, false)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.modified).toBe(iso)
  })

  it('adds modified to existing frontmatter (Req 16.2)', () => {
    const content = '---\ntitle: Note\ncreated: 2024-01-01T00:00:00.000Z\n---\n\nBody.\n'
    const result = injectAutoProperty(content, 'modified', iso, false)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.modified).toBe(iso)
    expect(parsed.created).toBe('2024-01-01T00:00:00.000Z') // preserved
    expect(parsed.title).toBe('Note')
  })

  it('overwrites existing modified on every save (Req 16.2)', () => {
    const oldModified = '2020-01-01T00:00:00.000Z'
    const content = `---\nmodified: ${oldModified}\n---\n\nBody.\n`
    const result = injectAutoProperty(content, 'modified', iso, false)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.modified).toBe(iso) // overwritten, not preserved
    expect(parsed.modified).not.toBe(oldModified)
  })
})

describe('injectAutoProperty — combined created + modified flow', () => {
  it('simulates create-then-save lifecycle (Req 16.1, 16.2)', () => {
    const createdIso = '2024-01-15T10:00:00.000Z'
    const modifiedIso = '2024-06-20T15:00:00.000Z'

    // 1. Create note — inject created
    let content = injectAutoProperty('# New Note\n\nInitial.\n', 'created', createdIso, true)
    const { parsed } = extractFrontmatter(content)
    expect(parsed.created).toBe(createdIso)
    expect(parsed.modified).toBeUndefined()

    // 2. User edits body, then saves — inject modified
    content = content.replace('Initial.', 'Updated content.')
    content = injectAutoProperty(content, 'modified', modifiedIso, false)
    const afterSave = extractFrontmatter(content)
    expect(afterSave.parsed.created).toBe(createdIso) // still preserved
    expect(afterSave.parsed.modified).toBe(modifiedIso) // now set
    expect(content).toContain('Updated content.')
  })

  it('does not inject created twice on re-save (Req 16.3, 16.6)', () => {
    const createdIso = '2024-01-15T10:00:00.000Z'
    const modifiedIso = '2024-06-20T15:00:00.000Z'

    // Content already has created
    const content = `---\ncreated: ${createdIso}\n---\n\nBody.\n`

    // Re-applying created with onlyIfAbsent should be a no-op
    const afterCreated = injectAutoProperty(content, 'created', '2099-01-01T00:00:00.000Z', true)
    expect(afterCreated).toBe(content)

    // Applying modified should add it without disturbing created
    const afterModified = injectAutoProperty(content, 'modified', modifiedIso, false)
    const { parsed } = extractFrontmatter(afterModified)
    expect(parsed.created).toBe(createdIso)
    expect(parsed.modified).toBe(modifiedIso)
  })
})

describe('injectAutoProperty — preserves existing properties (Req 16.4)', () => {
  it('preserves all existing frontmatter keys', () => {
    const content = '---\ntitle: Complex\ntags: [a, b]\naliases:\n  - alias1\n---\n\nBody.\n'
    const result = injectAutoProperty(content, 'modified', '2024-06-20T00:00:00.000Z', false)
    const { parsed } = extractFrontmatter(result)
    expect(parsed.title).toBe('Complex')
    expect(parsed.tags).toEqual(['a', 'b'])
    expect(parsed.aliases).toEqual(['alias1'])
    expect(parsed.modified).toBe('2024-06-20T00:00:00.000Z')
  })
})
