/**
 * Agent memory helpers — persistent per-vault knowledge files under `.ai/`.
 *
 *   - `.ai/agents.md`  — vault-specific instructions agents read before working
 *   - `.ai/vault.map`  — AI-readable knowledge map (types, folders, key notes)
 *   - `.ai/soul.md`    — running memory of the user (preferences, decisions)
 */
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { findMarkdownFiles, getNote, vaultContext } from './vault.js'

const AI_DIR = '.ai'
const SOUL_FILE = 'soul.md'
const VAULT_MAP_FILE = 'vault.map'
const SOUL_MAX_CHARS = 64 * 1024

function aiDir(vaultPath) {
  return path.join(vaultPath, AI_DIR)
}

async function readAiFile(vaultPath, fileName) {
  try {
    return {
      path: path.join(AI_DIR, fileName),
      content: await readFile(path.join(aiDir(vaultPath), fileName), 'utf8'),
    }
  } catch (error) {
    if (error?.code === 'ENOENT') return null
    throw error
  }
}

/**
 * Read `.ai/agents.md` — the vault-specific agent instructions file.
 * Falls back to the vault-root AGENTS.md when `.ai/agents.md` is absent.
 * @param {string} vaultPath
 * @returns {Promise<{path: string, content: string, source: 'dot_ai'|'agents_md'} | null>}
 */
export async function readAgentsMd(vaultPath) {
  const dotAi = await readAiFile(vaultPath, 'agents.md')
  if (dotAi) return { ...dotAi, source: 'dot_ai' }

  try {
    return {
      path: 'AGENTS.md',
      content: await readFile(path.join(vaultPath, 'AGENTS.md'), 'utf8'),
      source: 'agents_md',
    }
  } catch (error) {
    if (error?.code === 'ENOENT') return null
    throw error
  }
}

/**
 * Read `.ai/soul.md` — the running memory of the user for this vault.
 * @param {string} vaultPath
 * @returns {Promise<{path: string, content: string} | null>}
 */
export function readSoul(vaultPath) {
  return readAiFile(vaultPath, SOUL_FILE)
}

/**
 * Append a dated entry to `.ai/soul.md`, creating the file (and `.ai/`) when
 * missing. Keeps the file bounded by trimming the oldest entries once the
 * content exceeds SOUL_MAX_CHARS.
 *
 * @param {string} vaultPath
 * @param {{content: string, category?: string}} args
 * @returns {Promise<{path: string, content: string, mtimeMs: number}>}
 */
export async function updateSoul(vaultPath, args = {}) {
  const entry = typeof args.content === 'string' ? args.content.trim() : ''
  if (!entry) throw new Error('content is required')

  const category = typeof args.category === 'string' && args.category.trim()
    ? args.category.trim()
    : null
  const datedEntry = `- [${new Date().toISOString().slice(0, 10)}]${category ? ` (${category})` : ''} ${entry}`

  const existing = await readSoul(vaultPath)
  const header = existing?.content?.trim()
    ? ''
    : '# Agent Memory\n\nRunning memory of the user: preferences, working style, recurring goals, and past decisions. Updated by Nabu agents via the update_soul tool.\n'
  const base = existing?.content ?? ''
  const separator = base && !base.endsWith('\n') ? '\n' : ''
  let content = `${base}${separator}${datedEntry}\n`

  if (header) content = `${header}\n${content}`
  if (content.length > SOUL_MAX_CHARS) content = trimOldestSoulEntries(content)

  const filePath = path.join(aiDir(vaultPath), SOUL_FILE)
  await mkdir(aiDir(vaultPath), { recursive: true })
  await writeFileAtomic(filePath, content)
  return {
    path: path.join(AI_DIR, SOUL_FILE),
    content,
    mtimeMs: Date.now(),
  }
}

function trimOldestSoulEntries(content) {
  const lines = content.split('\n')
  const headerLines = lines.slice(0, 2).every((line) => line.startsWith('#') || line === '')
    ? lines.slice(0, 2)
    : []
  const entryLines = lines.slice(headerLines.length).filter((line) => line.trim().length > 0)
  while (entryLines.length > 1 && entryLines.join('\n').length > SOUL_MAX_CHARS) {
    entryLines.shift()
  }
  return `${[...headerLines, ...entryLines].join('\n')}\n`
}

async function writeFileAtomic(filePath, content) {
  const tempPath = `${filePath}.${process.pid}.${Date.now()}.tmp`
  try {
    await writeFile(tempPath, content, { encoding: 'utf8', flag: 'wx' })
    await rename(tempPath, filePath)
  } catch (error) {
    await rm(tempPath, { force: true })
    throw error
  }
}

/**
 * Build the AI-readable knowledge map for a vault: note types, folder tree,
 * organizational conventions, and key notes per type.
 * @param {string} vaultPath
 * @returns {Promise<{path: string, generatedAt: string, content: string, noteCount: number}>}
 */
export async function buildVaultMap(vaultPath) {
  const context = await vaultContext(vaultPath)
  const typeSamples = await collectTypeSamples(vaultPath, context.types)
  const lines = [
    `# Vault map`,
    '',
    `Generated: ${new Date().toISOString()}`,
    `Notes: ${context.noteCount}`,
    '',
    '## Types',
    context.types.length > 0
      ? context.types.map((type) => `- ${type}${typeSamples.get(type) ? ` — e.g. ${typeSamples.get(type)}` : ''}`).join('\n')
      : '- (none — notes are untyped)',
    '',
    '## Folders',
    context.folders.length > 0
      ? context.folders.map((folder) => `- ${folder}`).join('\n')
      : '- (root only)',
    '',
    '## Recent notes',
    context.recentNotes.length > 0
      ? context.recentNotes.map((note) => `- [[${note.title}]]${note.type ? ` (${note.type})` : ''} — ${note.path}`).join('\n')
      : '- (none)',
    '',
  ]

  return {
    path: path.join(AI_DIR, VAULT_MAP_FILE),
    generatedAt: new Date().toISOString(),
    content: lines.join('\n'),
    noteCount: context.noteCount,
  }
}

async function collectTypeSamples(vaultPath, types) {
  const samples = new Map()
  if (types.length === 0) return samples

  try {
    const files = await findMarkdownFiles(vaultPath)
    for (const filePath of files) {
      if (samples.size >= types.length && types.every((type) => samples.has(type))) break
      const relativePath = path.relative(vaultPath, filePath)
      const note = await getNote(vaultPath, relativePath).catch(() => null)
      const type = note?.frontmatter?.type ?? note?.frontmatter?.is_a ?? null
      if (typeof type !== 'string' || samples.has(type)) continue
      samples.set(type, `[[${note.frontmatter.title || note.path}]]`)
    }
  } catch {
    // Sample enrichment is best-effort; the map still lists types without it.
  }
  return samples
}

/**
 * Read the persisted `.ai/vault.map` (or null when it has not been generated).
 * @param {string} vaultPath
 */
export function readVaultMap(vaultPath) {
  return readAiFile(vaultPath, VAULT_MAP_FILE)
}

/**
 * Regenerate `.ai/vault.map` from the current vault contents.
 * @param {string} vaultPath
 */
export async function refreshVaultMap(vaultPath) {
  const map = await buildVaultMap(vaultPath)
  await mkdir(aiDir(vaultPath), { recursive: true })
  await writeFileAtomic(path.join(aiDir(vaultPath), VAULT_MAP_FILE), map.content)
  return map
}
