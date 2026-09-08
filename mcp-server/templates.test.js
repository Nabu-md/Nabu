import { describe, it, beforeEach, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { applyTemplate, listTemplates } from './templates.js'

let tmpDir
let vault

beforeEach(async () => {
  tmpDir = await mkdtemp(path.join(os.tmpdir(), 'nabu-mcp-templates-'))
  vault = path.join(tmpDir, 'Test Vault')
  await mkdir(vault, { recursive: true })
})

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true })
})

describe('listTemplates', () => {
  it('returns an empty list when the vault has no templates directory', async () => {
    assert.deepEqual(await listTemplates(vault), [])
  })

  it('lists markdown templates sorted by name with titles from H1 or frontmatter', async () => {
    await mkdir(path.join(vault, 'templates'), { recursive: true })
    await writeFile(
      path.join(vault, 'templates', 'meeting.md'),
      '# Weekly Meeting\n\nAgenda for {{date}}',
      'utf-8',
    )
    await writeFile(
      path.join(vault, 'templates', 'book-note.md'),
      '---\ntitle: Book Note\n---\n\nBody',
      'utf-8',
    )
    await writeFile(path.join(vault, 'templates', 'ignored.txt'), 'not a template', 'utf-8')

    const templates = await listTemplates(vault)
    assert.deepEqual(
      templates.map((template) => template.name),
      ['book-note', 'meeting'],
    )
    assert.equal(templates[0].title, 'Book Note')
    assert.equal(templates[1].title, 'Weekly Meeting')
    assert.equal(templates[0].path, path.join('templates', 'book-note.md'))
  })
})

describe('applyTemplate', () => {
  beforeEach(async () => {
    await mkdir(path.join(vault, 'templates'), { recursive: true })
    await writeFile(
      path.join(vault, 'templates', 'meeting.md'),
      '# {{title|Untitled Meeting}}\n\nDate: {{date|TBD}}\nType: {{type}}\n',
      'utf-8',
    )
  })

  it('creates a note with placeholder substitution and defaults', async () => {
    const note = await applyTemplate(vault, {
      name: 'meeting',
      params: { date: '2026-09-08' },
      title: 'Standup',
      type: 'meeting',
    })

    const content = await readFile(note.absolutePath, 'utf-8')
    assert.equal(content, '# Standup\n\nDate: 2026-09-08\nType: meeting\n')
    assert.equal(note.path, path.join('templates', 'meeting.new.md'))
  })

  it('keeps the placeholder when no param and no default are given', async () => {
    const note = await applyTemplate(vault, { name: 'meeting' })
    const content = await readFile(note.absolutePath, 'utf-8')
    assert.match(content, /Type: \{\{type\}\}/)
  })

  it('rejects missing templates with a helpful message', async () => {
    await assert.rejects(
      () => applyTemplate(vault, { name: 'missing' }),
      /Template not found: missing/,
    )
  })

  it('rejects invalid template names', async () => {
    await assert.rejects(() => applyTemplate(vault, { name: '../escape' }), /Invalid template name/)
    await assert.rejects(() => applyTemplate(vault, { name: '' }), /name is required/)
  })

  it('fails when the target note already exists', async () => {
    await applyTemplate(vault, { name: 'meeting' })
    await assert.rejects(() => applyTemplate(vault, { name: 'meeting' }), /already exists/i)
  })

  it('honors an explicit target path', async () => {
    const note = await applyTemplate(vault, { name: 'meeting', path: 'notes/standup.md' })
    assert.equal(note.path, path.join('notes', 'standup.md'))
  })
})
