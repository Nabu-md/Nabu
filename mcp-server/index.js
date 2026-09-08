#!/usr/bin/env node
/**
 * Tolaria MCP Server — lightweight vault tools for AI agents.
 *
 * These MCP tools provide Tolaria-specific capabilities alongside each
 * app-managed agent's own Safe / Power User permission profile:
 *
 *   - search_notes: full-text search across vault notes
 *   - search_notes_semantic: embedding-based similarity search over vault notes
 *   - web_fetch: fetch a web page as text or HTML (deep research scraping)
 *   - evaluate_sheet: evaluate CSV + formulas through an IronCalc engine
 *   - get_vault_context: vault structure overview (types, note count, folders)
 *   - get_note: parsed frontmatter + content (convenience over raw cat)
 *   - create_note: create a new markdown note without overwriting existing files
 *   - update_note: replace the full content of an existing note (optional conflict guard)
 *   - append_to_note: append markdown to an existing note body
 *   - open_note: signal Tolaria UI to open a note as a tab
 *   - highlight_editor: visually highlight a UI element (editor, tab, etc.)
 *   - refresh_vault: trigger vault rescan so new/modified files appear
 */
import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js'
import WebSocket from 'ws'
import { createMcpToolService } from './tool-service.js'
import { attachVault, cloneVault } from './vault-lifecycle.js'

const WS_UI_PORT = parseInt(process.env.WS_UI_PORT || '9711', 10)
const WS_UI_URL = `ws://localhost:${WS_UI_PORT}`
const LOCAL_READ_ONLY_TOOL_ANNOTATIONS = Object.freeze({
  readOnlyHint: true,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
})
const LOCAL_CREATE_TOOL_ANNOTATIONS = Object.freeze({
  readOnlyHint: false,
  destructiveHint: false,
  idempotentHint: false,
  openWorldHint: false,
})
const LOCAL_UPDATE_TOOL_ANNOTATIONS = LOCAL_CREATE_TOOL_ANNOTATIONS
const LOCAL_OPEN_WORLD_CREATE_TOOL_ANNOTATIONS = Object.freeze({
  ...LOCAL_CREATE_TOOL_ANNOTATIONS,
  openWorldHint: true,
})

// Connect as a WebSocket CLIENT to the UI bridge (run by ws-bridge.js).
// The bridge relays messages to all other clients (the React frontend).
let uiSocket = null
let reconnectTimer = null
let shutdownStarted = false
const RECONNECT_INTERVAL_MS = 3000

function connectUiBridge() {
  if (shutdownStarted) return

  try {
    const ws = new WebSocket(WS_UI_URL)
    uiSocket = ws
    ws.on('open', () => {
      if (shutdownStarted) {
        closeUiSocket()
        return
      }
      console.error(`[mcp] Connected to UI bridge at ${WS_UI_URL}`)
    })
    ws.on('close', () => {
      if (uiSocket === ws) uiSocket = null
      scheduleUiReconnect()
    })
    ws.on('error', () => {
      // Silent — bridge may not be running yet, will retry
    })
  } catch {
    scheduleUiReconnect()
  }
}

function scheduleUiReconnect() {
  if (shutdownStarted) return

  clearUiReconnectTimer()
  reconnectTimer = setTimeout(connectUiBridge, RECONNECT_INTERVAL_MS)
  reconnectTimer.unref?.()
}

function clearUiReconnectTimer() {
  if (!reconnectTimer) return

  clearTimeout(reconnectTimer)
  reconnectTimer = null
}

function closeUiSocket() {
  const socket = uiSocket
  uiSocket = null
  if (!socket) return

  socket.removeAllListeners()
  socket.on('error', () => {})
  if (socket.readyState === WebSocket.CONNECTING) {
    socket.terminate?.()
    return
  }

  try {
    socket.close()
  } catch {
    // Ignore close races during process teardown.
  }
  socket.terminate?.()
}

function broadcastUiAction(action, payload) {
  if (!uiSocket || uiSocket.readyState !== WebSocket.OPEN) return
  uiSocket.send(JSON.stringify({ type: 'ui_action', action, ...payload }))
}

const toolService = createMcpToolService({ emitUiAction: broadcastUiAction, attachVault, cloneVault })

const TOOLS = [
  {
    name: 'search_notes',
    description: 'Full-text search across vault notes by title or content. Returns matching paths, titles, and snippets.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Search query string' },
        limit: { type: 'number', description: 'Maximum number of results (default: 10)' },
      },
      required: ['query'],
    },
  },
  {
    name: 'get_vault_context',
    description: 'Get vault orientation for the active Tolaria vaults: entity types, AGENTS.md instructions, note count, folders, and recent notes.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        vaultPath: { type: 'string', description: 'Optional target vault root. Omit to inspect all active vaults.' },
      },
    },
  },
  {
    name: 'list_vaults',
    description: 'List the current active Tolaria vaults available to MCP tools, including whether each vault has AGENTS.md instructions.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'attach_vault',
    description: 'Register an existing accessible local folder as a mounted Tolaria vault. This does not initialize Git or change the active vault.',
    annotations: LOCAL_CREATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Absolute path to an existing local folder.' },
        label: { type: 'string', description: 'Optional display label. Defaults to the folder name.' },
      },
      required: ['path'],
    },
  },
  {
    name: 'clone_vault',
    description: 'Clone a Git repository with the system Git configuration, then register the resulting folder as a mounted Tolaria vault. Existing destinations are rejected.',
    annotations: LOCAL_OPEN_WORLD_CREATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        remoteUrl: { type: 'string', description: 'Git remote URL understood by the system Git client.' },
        destinationPath: { type: 'string', description: 'Absolute path for the new clone. It must not already exist.' },
        label: { type: 'string', description: 'Optional display label. Defaults to the destination folder name.' },
      },
      required: ['remoteUrl', 'destinationPath'],
    },
  },
  {
    name: 'get_note',
    description: 'Read a note with parsed YAML frontmatter, markdown content, and mtimeMs for conflict-guarded edits. Returns {path, frontmatter, content, mtimeMs}.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Relative path to the note (e.g. "project/my-project.md")' },
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
      required: ['path'],
    },
  },
  {
    name: 'create_note',
    description: 'Create a new markdown note inside an active Tolaria vault. Does not overwrite existing files. Use content for the full markdown including YAML frontmatter and H1.',
    annotations: LOCAL_CREATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Relative path inside the vault, or an absolute path inside an active vault. Must end in .md.' },
        content: { type: 'string', description: 'Full markdown note content, including YAML frontmatter when needed.' },
        title: { type: 'string', description: 'Optional title used only when content is omitted.' },
        type: { type: 'string', description: 'Optional note type used only when content is omitted.' },
        is_a: { type: 'string', description: 'Legacy alias for type, used only when content is omitted.' },
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
      required: ['path'],
    },
  },
  {
    name: 'update_note',
    description: 'Replace the full content (frontmatter + body) of an existing note. The note must already exist — use create_note for new notes. Optionally pass expectedMtime (the note mtimeMs returned by get_note) to fail-fast when the note changed between read and write.',
    annotations: LOCAL_UPDATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Relative path inside the vault, or an absolute path inside an active vault.' },
        content: { type: 'string', description: 'Full new markdown note content (frontmatter + body).' },
        expectedMtime: { type: 'number', description: 'Optional on-disk mtimeMs from get_note. When set, the update fails if the note has changed.' },
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
      required: ['path', 'content'],
    },
  },
  {
    name: 'append_to_note',
    description: 'Append markdown to the end of an existing note body. Lower-risk than update_note for agents that only need to log or extend content. The note must already exist.',
    annotations: LOCAL_UPDATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Relative path inside the vault, or an absolute path inside an active vault.' },
        content: { type: 'string', description: 'Markdown to append after the existing body.' },
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
      required: ['path', 'content'],
    },
  },
  {
    name: 'search_notes_semantic',
    description: 'Semantic search across vault notes using embedding similarity. Finds conceptually and morphologically related content that keyword search misses — use it for conceptual queries, and fall back to search_notes for exact keyword matches.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'The semantic query to search for' },
        limit: { type: 'number', description: 'Maximum results (default: 10)' },
      },
      required: ['query'],
    },
  },
  {
    name: 'web_fetch',
    description: 'Fetch a web page and return its content as extracted text (default) or raw HTML. Use for deep research and source gathering instead of shelling out to curl. Read-only; follows redirects; 20s timeout.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        url: { type: 'string', description: 'The absolute http(s) URL to fetch' },
        format: { type: 'string', enum: ['text', 'html'], description: 'Output format (default: text)' },
      },
      required: ['url'],
    },
  },
  {
    name: 'evaluate_sheet',
    description: 'Evaluate a spreadsheet with formulas. Parses CSV or markdown table content, applies cell overrides with formulas (including IronCalc financial functions like NPV, IRR, PMT, SUMIF, VLOOKUP), and returns the evaluated cell grid.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        csvContent: { type: 'string', description: 'CSV or markdown table content' },
        cellOverrides: { type: 'object', description: 'Cell address => value or formula mapping, e.g. {"B2": "=SUM(B1:B1)"}' },
      },
      required: ['csvContent'],
    },
  },
  {
    name: 'open_note',
    description: 'Open a note in the Tolaria UI as a new tab. Use after creating or editing a note so the user can see it.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Relative path to the note' },
        vaultPath: { type: 'string', description: 'Optional target vault root when opening a note outside the default vault.' },
      },
      required: ['path'],
    },
  },
  {
    name: 'highlight_editor',
    description: 'Visually highlight a UI element in Tolaria (editor, tab, properties panel, or note list). The highlight auto-clears after a short delay.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        element: { type: 'string', enum: ['editor', 'tab', 'properties', 'notelist'], description: 'Which UI element to highlight' },
        path: { type: 'string', description: 'Optional note path to associate with the highlight' },
      },
      required: ['element'],
    },
  },
  {
    name: 'refresh_vault',
    description: 'Trigger a vault rescan so new or modified files appear immediately in the Tolaria note list.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Optional specific note path that changed' },
        vaultPath: { type: 'string', description: 'Optional target vault root when refreshing a note outside the default vault.' },
      },
    },
  },
  {
    name: 'list_templates',
    description: 'List the note templates saved in the active vault (vault/templates/*.md). Before creating a note, check this for an applicable template.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
    },
  },
  {
    name: 'use_template',
    description: 'Apply a saved vault template (see list_templates) with {{placeholder|default}} parameter substitution and create the resulting note. Does not overwrite existing notes.',
    annotations: LOCAL_CREATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Template name (filename stem under vault/templates/).' },
        path: { type: 'string', description: 'Relative path for the new note inside the vault, ending in .md. Defaults to templates/{name}.new.md.' },
        params: { type: 'object', description: 'Template parameter values, e.g. {"project_name": "Acme"}. Values fill {{project_name}} tags; tags with a default like {{project_name|My Project}} keep the default when omitted.' },
        title: { type: 'string', description: 'Shortcut for params.title.' },
        type: { type: 'string', description: 'Shortcut for params.type.' },
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
      required: ['name'],
    },
  },
  {
    name: 'read_agents_md',
    description: 'Read the vault-specific agent instructions (.ai/agents.md, falling back to AGENTS.md). Read this before making vault-specific assumptions or asking the user questions.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
    },
  },
  {
    name: 'read_vault_map',
    description: 'Read the persisted .ai/vault.map knowledge map (note types, folders, key notes, conventions). Returns null when the map has not been generated yet; use refresh_vault_map to generate it.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
    },
  },
  {
    name: 'refresh_vault_map',
    description: 'Regenerate .ai/vault.map from the current vault contents and return it. Consult the map before asking the user questions about vault organization.',
    annotations: LOCAL_UPDATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
    },
  },
  {
    name: 'read_soul',
    description: 'Read .ai/soul.md — the running memory of the user for this vault (preferences, working style, recurring goals, past decisions).',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
    },
  },
  {
    name: 'update_soul',
    description: 'Append a dated entry to .ai/soul.md so future agents remember durable user preferences, decisions, or goals. Keep entries short and factual.',
    annotations: LOCAL_CREATE_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        content: { type: 'string', description: 'One-sentence memory entry, e.g. "Prefers bullet summaries over prose".' },
        category: { type: 'string', description: 'Optional category tag, e.g. preferences, workflow, decisions.' },
        vaultPath: { type: 'string', description: 'Optional target vault root when multiple vaults are active.' },
      },
      required: ['content'],
    },
  },
  {
    name: 'ask_clarifying_question',
    description: 'When the user request is ambiguous, present a structured clarification form in the chat panel instead of asking a free-text question. The user picks an option (or types a custom answer); the answer arrives as the next user message, then continue. Only available in deep_research, rag, and mini_app_builder modes.',
    annotations: LOCAL_READ_ONLY_TOOL_ANNOTATIONS,
    inputSchema: {
      type: 'object',
      properties: {
        question: { type: 'string', description: 'The question to ask, e.g. "What kind of project structure?"' },
        options: {
          type: 'array',
          description: '2-3 suggested answers. The UI always adds a custom input as the last option.',
          items: {
            type: 'object',
            properties: {
              label: { type: 'string', description: 'Short answer label, e.g. "Software project (repo, CI/CD, tests)"' },
              description: { type: 'string', description: 'Optional one-line detail shown under the label.' },
            },
            required: ['label'],
          },
        },
        mode: { type: 'string', description: 'The active permission mode (deep_research, rag, or mini_app_builder).' },
        rememberKey: { type: 'string', description: 'Optional key to remember the user choice for future sessions (stored in .ai/vault.map).' },
      },
      required: ['question', 'options'],
    },
  },
]

async function handleSearchNotes(args) {
  const results = await toolService.searchNotes(args)
  const text = results.length === 0
    ? 'No matching notes found.'
    : results.map(r => `**${r.title}** (${r.vaultLabel} / ${r.path})\n${r.snippet}`).join('\n\n')
  return { content: [{ type: 'text', text }] }
}

async function handleVaultContext(args = {}) {
  const ctx = await toolService.vaultContext(args)
  return { content: [{ type: 'text', text: JSON.stringify(ctx, null, 2) }] }
}

async function handleListVaults() {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.listVaults(), null, 2) }] }
}

async function handleAttachVault(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.attachVault(args), null, 2) }] }
}

async function handleCloneVault(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.cloneVault(args), null, 2) }] }
}

async function handleGetNote(args) {
  const note = await toolService.readNote(args)
  return { content: [{ type: 'text', text: JSON.stringify(note, null, 2) }] }
}

async function handleCreateNote(args = {}) {
  const note = await toolService.createNote(args)
  return {
    content: [{
      type: 'text',
      text: JSON.stringify(note, null, 2),
    }],
  }
}

async function handleUpdateNote(args = {}) {
  const note = await toolService.updateNote(args)
  return {
    content: [{
      type: 'text',
      text: JSON.stringify(note, null, 2),
    }],
  }
}

async function handleAppendToNote(args = {}) {
  const note = await toolService.appendToNote(args)
  return {
    content: [{
      type: 'text',
      text: JSON.stringify(note, null, 2),
    }],
  }
}

function handleOpenNote(args) {
  // Refresh vault first so the new/modified note appears in the note list,
  // then signal the UI to open it in a tab.
  const { targetPath } = toolService.openNoteAsTab(args)
  return { content: [{ type: 'text', text: `Opening ${targetPath} in Tolaria` }] }
}

function handleHighlightEditor(args) {
  toolService.highlightEditor(args)
  return { content: [{ type: 'text', text: `Highlighting ${args.element}` }] }
}

function handleRefreshVault(args) {
  toolService.refreshVault(args)
  return { content: [{ type: 'text', text: 'Vault refresh triggered' }] }
}

async function handleListTemplates(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.listNoteTemplates(args), null, 2) }] }
}

async function handleUseTemplate(args = {}) {
  const note = await toolService.useNoteTemplate(args)
  return {
    content: [{
      type: 'text',
      text: JSON.stringify(note, null, 2),
    }],
  }
}

async function handleReadAgentsMd(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.readVaultAgentsMd(args), null, 2) }] }
}

async function handleReadVaultMap(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.readVaultMapFile(args), null, 2) }] }
}

async function handleRefreshVaultMap(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.regenerateVaultMap(args), null, 2) }] }
}

async function handleReadSoul(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.readVaultSoul(args), null, 2) }] }
}

async function handleUpdateSoul(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(await toolService.writeVaultSoul(args), null, 2) }] }
}

function handleAskClarifyingQuestion(args = {}) {
  return { content: [{ type: 'text', text: JSON.stringify(toolService.askClarifyingQuestion(args), null, 2) }] }
}

async function handleSearchNotesSemantic(args) {
  const results = await toolService.searchNotesSemantic(args)
  const text = results.length === 0
    ? 'No semantically matching notes found. Try search_notes for exact keyword matches.'
    : results.map(r => `**${r.title}** (${r.vaultLabel} / ${r.path}) [score ${r.score.toFixed(3)}]\n${r.snippet}`).join('\n\n')
  return { content: [{ type: 'text', text }] }
}

async function handleWebFetch(args) {
  const result = await toolService.fetchWebPage(args)
  return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] }
}

async function handleEvaluateSheet(args = {}) {
  const result = await toolService.evaluateSheetWithFormulas(args)
  return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] }
}

const TOOL_HANDLERS = new Map([
  ['search_notes', handleSearchNotes],
  ['search_notes_semantic', handleSearchNotesSemantic],
  ['web_fetch', handleWebFetch],
  ['evaluate_sheet', handleEvaluateSheet],
  ['get_vault_context', handleVaultContext],
  ['list_vaults', handleListVaults],
  ['attach_vault', handleAttachVault],
  ['clone_vault', handleCloneVault],
  ['get_note', handleGetNote],
  ['create_note', handleCreateNote],
  ['update_note', handleUpdateNote],
  ['append_to_note', handleAppendToNote],
  ['open_note', handleOpenNote],
  ['highlight_editor', handleHighlightEditor],
  ['refresh_vault', handleRefreshVault],
  ['list_templates', handleListTemplates],
  ['use_template', handleUseTemplate],
  ['read_agents_md', handleReadAgentsMd],
  ['read_vault_map', handleReadVaultMap],
  ['refresh_vault_map', handleRefreshVaultMap],
  ['read_soul', handleReadSoul],
  ['update_soul', handleUpdateSoul],
  ['ask_clarifying_question', handleAskClarifyingQuestion],
])

function callToolHandler(name, args) {
  const handler = TOOL_HANDLERS.get(name)
  if (!handler) throw new Error(`Unknown tool: ${name}`)
  return handler(args)
}

// --- Server setup ---

const server = new Server(
  { name: 'tolaria-mcp-server', version: '0.3.0' },
  { capabilities: { tools: {} } },
)

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: TOOLS,
}))

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params
  try {
    return await callToolHandler(name, args)
  } catch (error) {
    return {
      content: [{ type: 'text', text: `Error: ${error.message}` }],
      isError: true,
    }
  }
})

async function shutdown(exitCode = 0) {
  if (shutdownStarted) return

  shutdownStarted = true
  clearUiReconnectTimer()
  closeUiSocket()

  try {
    await server.close()
  } catch (error) {
    console.error(`[mcp] Error while closing server: ${error.message}`)
  }

  process.exitCode = exitCode
  setImmediate(() => process.exit(exitCode))
}

async function main() {
  const transport = new StdioServerTransport()
  server.onclose = () => {
    void shutdown(0)
  }
  process.stdin.once('end', () => {
    void shutdown(0)
  })
  process.stdin.once('close', () => {
    void shutdown(0)
  })
  process.once('SIGINT', () => {
    void shutdown(0)
  })
  process.once('SIGTERM', () => {
    void shutdown(0)
  })

  connectUiBridge()
  await server.connect(transport)
  console.error('Tolaria MCP server running (vaults resolved per call)')
}

main().catch((error) => {
  console.error(error)
  void shutdown(1)
})
