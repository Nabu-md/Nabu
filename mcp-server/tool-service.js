import path from 'node:path'
import {
  appendToNote as appendVaultNote,
  createNote as createVaultNote,
  getNote,
  searchNotes as searchVaultNotes,
  updateNote as updateVaultNote,
} from './vault.js'
import { findMarkdownFiles } from './vault.js'
import { rankNotesBySimilarity } from './semantic-search.js'
import { evaluateSheet } from './sheet-eval.js'
import { requireVaultPaths } from './vault-path.js'
import { readAgentInstructions, vaultContextWithInstructions } from './agent-instructions.js'
import { applyTemplate, listTemplates } from './templates.js'
import {
  readAgentsMd,
  readSoul,
  readVaultMap,
  refreshVaultMap,
  updateSoul,
} from './agent-memory.js'
import { buildClarifyingForm } from './clarifying-questions.js'

export function createMcpToolService({
  resolveVaultPaths = () => requireVaultPaths(),
  emitUiAction = () => {},
  attachVault: attachVaultOperation,
  cloneVault: cloneVaultOperation,
} = {}) {
  const sessionVaultPaths = []

  function activeVaultPaths() {
    return [...new Set([...resolveVaultPaths(), ...sessionVaultPaths])]
  }

  async function registerVault(operation, args, registrationType) {
    if (!operation) throw new Error('Vault lifecycle operation is unavailable')
    const entry = await operation(args)
    if (!sessionVaultPaths.includes(entry.path)) sessionVaultPaths.push(entry.path)
    emitUiAction('vault_registry_changed', { path: entry.path, registrationType })
    return entry
  }

  function attachVault(args = {}) {
    return registerVault(attachVaultOperation, args, 'attach')
  }

  function cloneVault(args = {}) {
    return registerVault(cloneVaultOperation, args, 'clone')
  }

  function requestedVaultPath(args = {}) {
    const requested = typeof args.vaultPath === 'string' ? args.vaultPath.trim() : ''
    if (!requested) return null
    if (!activeVaultPaths().includes(requested)) {
      throw new Error(`Vault is not active in Tolaria: ${requested}`)
    }
    return requested
  }

  function resolveUiPath(args = {}) {
    const notePath = typeof args.path === 'string' ? args.path : ''
    if (path.isAbsolute(notePath)) return notePath
    const roots = activeVaultPaths()
    const vaultPath = requestedVaultPath(args) ?? (roots.length === 1 ? roots[0] : '')
    return vaultPath ? path.join(vaultPath, notePath) : notePath
  }

  async function readNote(args = {}) {
    return getNoteFromActiveVaults(notePathArg(args), requestedVaultPath(args))
  }

  async function searchNotes(args = {}) {
    const requestedLimit = Number.isFinite(args.limit) && args.limit > 0 ? args.limit : 10
    const results = []

    for (const vaultPath of activeVaultPaths()) {
      const vaultResults = await searchVaultNotes(vaultPath, args.query, requestedLimit)
      results.push(...vaultResults.map((result) => withVaultMetadata(result, vaultPath)))
      if (results.length >= requestedLimit) break
    }

    return results.slice(0, requestedLimit)
  }

  async function searchNotesSemantic(args = {}) {
    if (typeof args.query !== 'string' || !args.query.trim()) {
      throw new Error('query is required')
    }

    const limit = Number.isFinite(args.limit) && args.limit > 0 ? args.limit : 10
    const candidates = []
    for (const vaultPath of activeVaultPaths()) {
      const files = await findMarkdownFiles(vaultPath)
      for (const filePath of files) {
        const content = await readNoteFileContent(filePath)
        if (content === null) continue
        candidates.push(buildSemanticCandidate(vaultPath, filePath, content))
      }
    }

    const ranked = rankNotesBySimilarity(args.query, candidates, { limit })
    return ranked.map((result) => {
      const { candidate, content, ...rest } = result
      return withVaultMetadata(rest, candidate.vaultPath)
    })
  }

  async function vaultContext(args = {}) {
    const targetVaultPath = requestedVaultPath(args)
    const roots = activeVaultPaths()
    if (targetVaultPath) return vaultContextWithInstructions(targetVaultPath)
    if (roots.length === 1) return vaultContextWithInstructions(roots[0])

    return {
      vaults: await Promise.all(roots.map(vaultContextWithInstructions)),
    }
  }

  async function listVaults() {
    const vaults = await Promise.all(activeVaultPaths().map(async (vaultPath) => {
      const agentInstructions = await readAgentInstructions(vaultPath)
      return {
        path: vaultPath,
        label: vaultLabel(vaultPath),
        agentInstructionsPath: agentInstructions?.path ?? null,
        hasAgentInstructions: agentInstructions !== null,
      }
    }))

    return { vaults }
  }

  async function createNote(args = {}) {
    const vaultPath = writableVaultPath(args)
    const notePath = writableNotePath(args, vaultPath)
    const note = await createVaultNote(vaultPath, notePath, createNoteContent(args))
    const targetPath = resolveUiPath({ ...args, path: note.path, vaultPath })
    emitUiAction('vault_changed', { path: targetPath })
    emitUiAction('open_tab', { path: targetPath })
    return { path: note.path, absolutePath: note.absolutePath, vaultPath }
  }

  async function updateNote(args = {}) {
    const { vaultPath, notePath } = await resolveWritableNote(args)
    const note = await updateVaultNote(vaultPath, notePath, noteContent(args), {
      expectedMtime: args.expectedMtime,
    })
    emitUiAction('vault_changed', { path: resolveUiPath({ ...args, path: note.path, vaultPath }) })
    return { path: note.path, absolutePath: note.absolutePath, vaultPath, mtimeMs: note.mtimeMs }
  }

  async function appendToNote(args = {}) {
    const { vaultPath, notePath } = await resolveWritableNote(args)
    const note = await appendVaultNote(vaultPath, notePath, noteContent(args))
    emitUiAction('vault_changed', { path: resolveUiPath({ ...args, path: note.path, vaultPath }) })
    return { path: note.path, absolutePath: note.absolutePath, vaultPath, mtimeMs: note.mtimeMs }
  }

  async function resolveWritableNote(args = {}) {
    const vaultPath = writableVaultPath(args)
    const notePath = writableNotePath(args, vaultPath)
    return { vaultPath, notePath }
  }

  function templatesVaultPath(args = {}) {
    const requested = requestedVaultPath(args)
    if (requested) return requested
    const roots = activeVaultPaths()
    if (roots.length === 1) return roots[0]
    throw new Error('Multiple vaults are active. Pass vaultPath for template operations.')
  }

  async function listNoteTemplates(args = {}) {
    return listTemplates(templatesVaultPath(args))
  }

  async function useNoteTemplate(args = {}) {
    const vaultPath = templatesVaultPath(args)
    const note = await applyTemplate(vaultPath, args)
    const targetPath = resolveUiPath({ ...args, path: note.path, vaultPath })
    emitUiAction('vault_changed', { path: targetPath })
    emitUiAction('open_tab', { path: targetPath })
    return { path: note.path, absolutePath: note.absolutePath, vaultPath }
  }

  function memoryVaultPath(args = {}) {
    const requested = requestedVaultPath(args)
    if (requested) return requested
    const roots = activeVaultPaths()
    if (roots.length === 1) return roots[0]
    throw new Error('Multiple vaults are active. Pass vaultPath for agent memory operations.')
  }

  async function readVaultAgentsMd(args = {}) {
    return readAgentsMd(memoryVaultPath(args))
  }

  async function readVaultSoul(args = {}) {
    return readSoul(memoryVaultPath(args))
  }

  async function writeVaultSoul(args = {}) {
    const vaultPath = memoryVaultPath(args)
    const result = await updateSoul(vaultPath, args)
    emitUiAction('vault_changed', { path: path.join(vaultPath, result.path) })
    return result
  }

  async function readVaultMapFile(args = {}) {
    return readVaultMap(memoryVaultPath(args))
  }

  async function regenerateVaultMap(args = {}) {
    const vaultPath = memoryVaultPath(args)
    const result = await refreshVaultMap(vaultPath)
    emitUiAction('vault_changed', { path: path.join(vaultPath, result.path) })
    return result
  }

  function askClarifyingQuestion(args = {}) {
    const form = buildClarifyingForm(args)
    emitUiAction('ask_clarifying_question', { form })
    return {
      status: 'presented',
      form_id: form.id,
      instruction: 'The form is rendered in the chat panel. The user answer will arrive as the next user message; do not guess an answer yourself.',
    }
  }

  function openNoteAsTab(args = {}) {
    const targetPath = resolveUiPath(args)
    emitUiAction('vault_changed', { path: targetPath })
    emitUiAction('open_tab', { path: targetPath })
    return { targetPath }
  }

  function openNoteInEditor(args = {}) {
    const targetPath = resolveUiPath(args)
    emitUiAction('vault_changed', { path: targetPath })
    emitUiAction('open_note', { path: targetPath })
    return { targetPath }
  }

  function highlightEditor(args = {}) {
    emitUiAction('highlight', { element: args.element, path: args.path })
  }

  function setFilter(args = {}) {
    emitUiAction('set_filter', { filterType: args.type })
  }

  function refreshVault(args = {}) {
    emitUiAction('vault_changed', { path: resolveUiPath(args) })
  }

  async function getNoteFromActiveVaults(notePath, vaultPath = null) {
    const candidates = vaultPath ? [vaultPath] : activeVaultPaths()
    const matches = []
    const errors = []

    for (const candidate of candidates) {
      try {
        matches.push(withVaultMetadata(await getNote(candidate, notePath), candidate))
      } catch (error) {
        errors.push(error)
      }
    }

    if (matches.length === 1) return matches[0]
    if (matches.length > 1) {
      throw new Error(`Note path is ambiguous across active vaults. Pass vaultPath for ${notePath}.`)
    }
    throw errors[0] ?? new Error(`Note not found: ${notePath}`)
  }

  function writableVaultPath(args = {}) {
    const requested = requestedVaultPath(args)
    if (requested) return requested

    const roots = activeVaultPaths()
    const notePath = notePathArg(args)
    if (path.isAbsolute(notePath)) {
      const root = roots.find(vaultPath => isInsideVaultRoot(vaultPath, notePath))
      if (root) return root
    }
    if (roots.length === 1) return roots[0]
    throw new Error(`Note path is ambiguous across active vaults. Pass vaultPath for ${notePath}.`)
  }

  async function evaluateSheetWithFormulas(args = {}) {
    return evaluateSheet({
      csvContent: args.csvContent,
      cellOverrides: args.cellOverrides,
    })
  }

  async function fetchWebPage(args = {}) {
    const url = typeof args.url === 'string' ? args.url.trim() : ''
    if (!/^https?:\/\//i.test(url)) {
      throw new Error('url must be an absolute http(s) URL')
    }

    let timeoutHandle
    try {
      const response = await fetch(url, {
        redirect: 'follow',
        signal: AbortSignal.timeout(WEB_FETCH_TIMEOUT_MS),
      })
      const html = await response.text()
      if (!response.ok) {
        throw new Error(`Request failed with status ${response.status}`)
      }
      if (typeof args.format === 'string' && args.format === 'html') {
        return {
          url: response.url || url,
          status: response.status,
          format: 'html',
          content: html,
        }
      }
      return {
        url: response.url || url,
        status: response.status,
        format: 'text',
        content: htmlToText(html),
      }
    } catch (error) {
      if (error instanceof Error && error.name === 'TimeoutError') {
        throw new Error(`Request timed out after ${Math.round(WEB_FETCH_TIMEOUT_MS / 1000)}s: ${url}`)
      }
      throw error instanceof Error ? error : new Error(String(error))
    } finally {
      clearTimeout(timeoutHandle)
    }
  }

  return {
    activeVaultPaths,
    appendToNote,
    attachVault,
    cloneVault,
    createNote,
    evaluateSheetWithFormulas,
    fetchWebPage,
    highlightEditor,
    listNoteTemplates,
    listVaults,
    openNoteAsTab,
    openNoteInEditor,
    readNote,
    refreshVault,
    requestedVaultPath,
    resolveUiPath,
    searchNotes,
    searchNotesSemantic,
    setFilter,
    askClarifyingQuestion,
    readVaultAgentsMd,
    readVaultSoul,
    writeVaultSoul,
    readVaultMapFile,
    regenerateVaultMap,
    updateNote,
    useNoteTemplate,
    vaultContext,
  }
}

function writableNotePath(args, vaultPath) {
  const notePath = notePathArg(args)
  if (!path.isAbsolute(notePath) || !isInsideVaultRoot(vaultPath, notePath)) return notePath
  return path.relative(vaultPath, notePath)
}

function withVaultMetadata(note, vaultPath) {
  return {
    ...note,
    vaultPath,
    vaultLabel: vaultLabel(vaultPath),
  }
}

function vaultLabel(vaultPath) {
  return path.basename(vaultPath) || vaultPath
}

function isInsideVaultRoot(vaultPath, notePath) {
  const relative = path.relative(vaultPath, notePath)
  return Boolean(relative) && !relative.startsWith('..') && !path.isAbsolute(relative)
}

function notePathArg(args = {}) {
  const notePath = typeof args.path === 'string' ? args.path.trim() : ''
  if (!notePath) throw new Error('Note path is required')
  return notePath
}

function yamlScalar(value) {
  return JSON.stringify(value)
}

function trimmedString(value) {
  return typeof value === 'string' ? value.trim() : ''
}

function fallbackCreateNoteContent(args = {}) {
  const title = trimmedString(args.title) || path.basename(notePathArg(args), '.md')
  const type = trimmedString(args.type) || trimmedString(args.is_a) || 'Note'
  return `---\ntype: ${yamlScalar(type)}\n---\n\n# ${title}\n`
}

function createNoteContent(args = {}) {
  return typeof args.content === 'string' && args.content.trim()
    ? args.content
    : fallbackCreateNoteContent(args)
}

function noteContent(args = {}) {
  if (typeof args.content === 'string' && args.content.length > 0) return args.content
  throw new Error('content is required')
}

const WEB_FETCH_TIMEOUT_MS = 20_000
const WEB_FETCH_MAX_BYTES = 1_500_000

async function readNoteFileContent(filePath) {
  const { readFile } = await import('node:fs/promises')
  try {
    return await readFile(filePath, 'utf-8')
  } catch {
    return null
  }
}

function buildSemanticCandidate(vaultPath, filePath, content) {
  const relativePath = path.relative(vaultPath, filePath)
  const filename = path.basename(filePath, '.md')
  const h1Match = content.match(/^#\s+(.+)$/m)
  const titleMatch = content.match(/^title:\s*(.+)$/m)
  return {
    candidate: { vaultPath },
    path: relativePath,
    title: (titleMatch ? titleMatch[1].trim() : '')
      || (h1Match ? h1Match[1].trim() : '')
      || filename,
    content,
  }
}

const HTML_BLOCK_TAGS = [
  'address', 'article', 'aside', 'blockquote', 'details', 'dialog', 'dd',
  'div', 'dl', 'dt', 'fieldset', 'figcaption', 'figure', 'footer', 'form',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'header', 'hgroup', 'hr', 'li',
  'main', 'nav', 'ol', 'p', 'pre', 'section', 'table', 'tbody', 'td',
  'tfoot', 'th', 'thead', 'tr', 'ul',
]
const SCRIPT_STYLE_BLOCK_RE = /<(?:script|style|noscript|template)\b[^>]*>[\s\S]*?<\/(?:script|style|noscript|template)>/gi

function htmlToText(html) {
  return html
    .replace(SCRIPT_STYLE_BLOCK_RE, ' ')
    .replace(/<\!--[\s\S]*?-->/g, ' ')
    .replace(new RegExp(`</?(?:${HTML_BLOCK_TAGS.join('|')})\\b[^>]*>`, 'gi'), '\n')
    .replace(/<br\b[^>]*>/gi, '\n')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/gi, ' ')
    .replace(/&amp;/gi, '&')
    .replace(/&lt;/gi, '<')
    .replace(/&gt;/gi, '>')
    .replace(/&quot;/gi, '"')
    .replace(/&#0?39;/g, "'")
    .replace(/&apos;/gi, "'")
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 4000)
    .join('\n')
    .slice(0, WEB_FETCH_MAX_BYTES)
}
