import type { AiAgentId } from '../lib/aiAgents'
import type { AiAgentPermissionMode } from '../lib/aiAgentPermissionMode'

/**
 * AI Agent utilities for app-managed CLI agent sessions.
 *
 * App-managed sessions can edit files in the active vault and use Nabu-specific
 * MCP tools (search_notes, get_vault_context, get_note, open_note).
 * The frontend receives streaming events for text, tool calls, and completion.
 */

// --- Agent system prompt ---

interface AgentSystemPromptOptions {
  vaultContext?: string
  agentDocsPath?: string
  permissionMode?: AiAgentPermissionMode
  agent?: AiAgentId
  vaultPaths?: string[]
}

function normalizePromptOptions(
  options?: string | AgentSystemPromptOptions,
): AgentSystemPromptOptions {
  return typeof options === 'string' ? { vaultContext: options } : options ?? {}
}

function miniAppBuilderInstructions(): string {
  return [
    'Mini App Builder mode is active. Your job is to build, iterate, and test Nabu mini-apps.',
    'Mini-apps live in `.apps/{id}/` inside the vault: a `manifest.json` plus static HTML/CSS/JS files.',
    'The manifest is JSON with `id`, `name`, `entrypoint_url`, `width`, `height`, `resizable`, and `allow_vault_access`.',
    'Set `allow_vault_access: true` only when the app must read or write notes; the vault MCP relay then exposes search_notes, get_note, create_note, update_note, append_to_note, open_note, and refresh_vault to the app over postMessage.',
    'Scaffold common patterns when they fit: dashboards (CSV/notes + Chart.js in an inline script), structured forms that write to notes, custom editors for a note type, and visualizers of note relationships.',
    'After writing files, call open_mini_app_window to test the app live, then use search_notes/read tools to inspect what it produced and iterate.',
  ].join('\n')
}

function permissionModeInstructions(
  mode: AiAgentPermissionMode = 'safe',
  agent?: AiAgentId,
): string {
  if (mode === 'power_user') {
    if (agent === 'pi') {
      return `Power User mode is selected, but Pi currently uses the same conservative Nabu MCP configuration in both modes. Do not promise shell execution unless the Pi CLI exposes it directly in this run.`
    }

    return `Power User mode is active. Local shell commands are available for this vault where the selected CLI agent supports them. Keep commands scoped to the active vault, avoid destructive commands unless explicitly requested, and do not expose note content unnecessarily.`
  }

  if (mode === 'mini_app_builder') {
    return `Mini App Builder mode is active. Local shell commands are available for this vault where the selected CLI agent supports them, in addition to the mini-app scaffolding workflow described below.\n\n${miniAppBuilderInstructions()}`
  }

  if (mode === 'rag') {
    return `RAG (Semantic Search) mode is active. Shell, terminal, Bash, Python/Node script execution, git, and command-line tools are not available; use file/search/edit tools and Nabu MCP tools instead.\n\nPrefer the search_notes_semantic MCP tool (or its alias query_vault_rag — both use the same local embedding model) for retrieval: it finds conceptually related notes that keyword matching misses. Use it first for conceptual questions and summaries; fall back to search_notes for exact keyword or title matches.\n\nTo ground an answer in retrieved notes, run search_notes_semantic/query_vault_rag first, then get_note on the top hits before composing the response.\n\nCite the source vault by name for every note you draw on. Retrieval results list each hit's vault on a "Vault: <label>" line; mirror that label in your answer, e.g. "per the note X in <vault label>…". When several vaults are active this tells the user which knowledge base each claim came from.`
  }

  return `Vault Safe mode is active. Do not use shell, terminal, Bash, Python/Node script execution, git, or command-line tools. If the user asks whether shell commands are available, say they are not available in Vault Safe. Use file/search/edit tools and Nabu MCP tools instead.`
}

function agentDocsInstructions(
  agentDocsPath: string | undefined,
  canUseShell: boolean,
): string {
  if (!agentDocsPath) {
    return `Read the vault's AGENTS.md when one exists before making vault-specific assumptions.`
  }

  const searchInstruction = canUseShell
    ? `Start with ${agentDocsPath}/index.md, then use ripgrep over that folder for specific concepts.`
    : `Start with ${agentDocsPath}/index.md, then use the available file and search tools for specific concepts.`

  return `Read the vault's AGENTS.md when one exists before making vault-specific assumptions.
For Nabu product behavior, workflows, and user questions about how Nabu works, search the bundled local docs at:
${agentDocsPath}

${searchInstruction} Prefer bundled docs over guesses for Nabu behavior.

When the user asks how to improve a knowledge base, make it better organized, choose better types, model relationships, or make the vault easier for humans and agents to use, treat Portent as Nabu's default best-practice model. Read ${agentDocsPath}/pages/templates/portent.md and combine it with Nabu concepts for types, relationships, properties, Inbox, archive, and custom views.`
}

function vaultScopeInstructions(vaultPaths?: string[]): string {
  const roots = (vaultPaths ?? []).map((path) => path.trim()).filter(Boolean)
  if (roots.length <= 1) {
    return `You can edit markdown files in the active vault. Keep file operations scoped to that vault unless the user explicitly gives another path.`
  }

  return [
    `Multiple Nabu vaults are active. You can read and edit markdown files in these vault roots:`,
    roots.map((path) => `- ${path}`).join('\n'),
    `When using Nabu MCP tools, pass the target vault path when a relative note path could be ambiguous.`,
  ].join('\n')
}

const SHEET_INSTRUCTIONS = `For financial analysis and tabular computation, use the evaluate_sheet MCP tool: it imports CSV (or markdown table) data, applies cell formulas (IronCalc semantics, including [[note]] wikilink references via dependencies/links), and returns the evaluated grid. IronCalc supports financial functions (NPV, IRR, XIRR, PMT, PV, FV, RATE), aggregations (SUM, SUMIF, SUMIFS, AVERAGE, COUNTIF), and lookups (VLOOKUP, HLOOKUP, INDEX, MATCH). Pass formulas in cellOverrides, e.g. {"B2": "=SUM(B1:B1)"}.

When the user wants a shareable deliverable, use create_report: it evaluates the sheet and saves a markdown report note (source-data table plus a sanitized HTML bar chart) under Research Reports/ in the vault, returning the note path.

When the task is specifically financial metrics, use crunch_financials instead: it evaluates the sheet, extracts every cell whose formula uses NPV, IRR, XIRR, PMT, PV, FV, or RATE, and returns a narrative metrics report; pass saveNote=true (with vaultPath) to persist it as a note.`

const AGENT_SYSTEM_PREAMBLE = `You are working inside Nabu, a local-first Markdown knowledge base.

Notes are Markdown files with YAML frontmatter. Organization is primarily expressed through H1 titles, types, properties, wikilinks, and relationships, not folder structure.
Prefer file edit tools for note changes.
Use the provided MCP tools for: full-text search (search_notes), vault orientation (get_vault_context), parsed note reading (get_note), and opening notes in the UI (open_note).
Use create_note(path, content, vaultPath?) for new Markdown notes when shell writes are unavailable.

Before creating a note, check list_templates() for an applicable saved template in vault/templates/, and use_template(name, params) when one fits.
Before making vault-specific assumptions or asking the user questions, read_agents_md() and read_vault_map() (or refresh_vault_map() to regenerate it).
Remember durable user preferences and decisions with update_soul() so future sessions keep continuity; read_soul() first when context is thin.

When you create or edit a note, call open_note(path) so the user sees it in Nabu.
When you mention or reference a note by name, always use [[Note Title]] wikilink syntax so the user can click to open it.
Be concise and helpful. When you've completed a task, briefly summarize what you did.`

export function buildAgentSystemPrompt(options?: string | AgentSystemPromptOptions): string {
  const { vaultContext, agentDocsPath, permissionMode, agent, vaultPaths } = normalizePromptOptions(options)
  const canUseShell = (permissionMode === 'power_user' || permissionMode === 'mini_app_builder') && agent !== 'pi'
  const prompt = [
    AGENT_SYSTEM_PREAMBLE,
    SHEET_INSTRUCTIONS,
    vaultScopeInstructions(vaultPaths),
    agentDocsInstructions(agentDocsPath, canUseShell),
    permissionModeInstructions(permissionMode, agent),
  ].join('\n\n')

  if (!vaultContext) return prompt
  return `${prompt}\n\nVault context:\n${vaultContext}`
}
