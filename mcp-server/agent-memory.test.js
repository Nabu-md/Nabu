import { describe, it, beforeEach, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import {
  readAgentsMd,
  readSoul,
  readVaultMap,
  refreshVaultMap,
  updateSoul,
} from './agent-memory.js'

let tmpDir
let vault

beforeEach(async () => {
  tmpDir = await mkdtemp(path.join(os.tmpdir(), 'nabu-mcp-agent-memory-'))
  vault = path.join(tmpDir, 'Test Vault')
  await mkdir(vault, { recursive: true })
})

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true })
})

describe('readAgentsMd', () => {
  it('returns null when neither .ai/agents.md nor AGENTS.md exists', async () => {
    assert.equal(await readAgentsMd(vault), null)
  })

  it('reads .ai/agents.md when present', async () => {
    await mkdir(path.join(vault, '.ai'), { recursive: true })
    await writeFile(path.join(vault, '.ai', 'agents.md'), '# Vault rules\n', 'utf-8')

    const result = await readAgentsMd(vault)
    assert.equal(result.source, 'dot_ai')
    assert.equal(result.path, path.join('.ai', 'agents.md'))
    assert.equal(result.content, '# Vault rules\n')
  })

  it('falls back to vault-root AGENTS.md', async () => {
    await writeFile(path.join(vault, 'AGENTS.md'), '# Root rules\n', 'utf-8')

    const result = await readAgentsMd(vault)
    assert.equal(result.source, 'agents_md')
    assert.equal(result.path, 'AGENTS.md')
  })

  it('prefers .ai/agents.md over AGENTS.md', async () => {
    await mkdir(path.join(vault, '.ai'), { recursive: true })
    await writeFile(path.join(vault, '.ai', 'agents.md'), 'dot-ai', 'utf-8')
    await writeFile(path.join(vault, 'AGENTS.md'), 'root', 'utf-8')

    assert.equal((await readAgentsMd(vault)).content, 'dot-ai')
  })
})

describe('readSoul / updateSoul', () => {
  it('returns null when soul.md does not exist', async () => {
    assert.equal(await readSoul(vault), null)
  })

  it('creates .ai/soul.md with a header and a dated entry', async () => {
    const result = await updateSoul(vault, { content: 'Prefers concise answers' })

    assert.equal(result.path, path.join('.ai', 'soul.md'))
    assert.match(result.content, /^# Agent Memory/)
    assert.match(result.content, /- \[\d{4}-\d{2}-\d{2}\] Prefers concise answers/)
    assert.equal((await readSoul(vault)).content, result.content)
  })

  it('appends categorized entries without re-adding the header', async () => {
    await updateSoul(vault, { content: 'First entry' })
    const second = await updateSoul(vault, { content: 'Second entry', category: 'preference' })

    assert.match(second.content, /- \[\d{4}-\d{2}-\d{2}\] \(preference\) Second entry/)
    assert.equal(second.content.match(/# Agent Memory/g)?.length, 1)
  })

  it('rejects empty content', async () => {
    await assert.rejects(() => updateSoul(vault, { content: '   ' }), /content is required/)
  })

  it('persists to disk atomically', async () => {
    await updateSoul(vault, { content: 'Persisted' })
    const raw = await readFile(path.join(vault, '.ai', 'soul.md'), 'utf-8')
    assert.match(raw, /Persisted/)
  })
})

describe('readVaultMap / refreshVaultMap', () => {
  it('returns null before the map has been generated', async () => {
    assert.equal(await readVaultMap(vault), null)
  })

  it('regenerates and persists the vault map', async () => {
    await writeFile(
      path.join(vault, 'alpha.md'),
      '---\ntitle: Alpha\ntype: project\n---\n\nBody',
      'utf-8',
    )
    const map = await refreshVaultMap(vault)

    assert.equal(map.path, path.join('.ai', 'vault.map'))
    assert.equal(map.noteCount, 1)
    assert.match(map.content, /# Vault map/)
    assert.match(map.content, /## Types/)
    assert.match(map.content, /project/)
    assert.equal((await readVaultMap(vault)).content, map.content)
  })
})
