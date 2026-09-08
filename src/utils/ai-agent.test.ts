import { describe, expect, it } from 'vitest'

import { buildAgentSystemPrompt } from './ai-agent'

// --- buildAgentSystemPrompt ---

describe('buildAgentSystemPrompt', () => {
  it('returns preamble when no vault context', () => {
    const prompt = buildAgentSystemPrompt()
    expect(prompt).toContain('working inside Nabu')
    expect(prompt).toContain('active vault')
    expect(prompt).toContain("vault's AGENTS.md")
    expect(prompt).toContain('Vault Safe mode is active')
    expect(prompt).toContain('not available in Vault Safe')
    expect(prompt).not.toContain('full shell access')
    expect(prompt).not.toContain('Vault context')
  })

  it('appends vault context when provided', () => {
    const prompt = buildAgentSystemPrompt('Recent notes: foo, bar')
    expect(prompt).toContain('working inside Nabu')
    expect(prompt).toContain('Vault context:')
    expect(prompt).toContain('Recent notes: foo, bar')
  })

  it('points safe-mode agents to bundled Nabu docs without shell commands', () => {
    const prompt = buildAgentSystemPrompt({ agentDocsPath: '/app/agent-docs' })

    expect(prompt).toContain('/app/agent-docs/index.md')
    expect(prompt).toContain('/app/agent-docs/pages/templates/portent.md')
    expect(prompt).toContain("Portent as Nabu's default best-practice model")
    expect(prompt).not.toContain('ripgrep')
    expect(prompt).toContain('Prefer bundled docs over guesses')
  })

  it('keeps ripgrep guidance for bundled docs in shell-capable power user mode', () => {
    const prompt = buildAgentSystemPrompt({
      agent: 'codex',
      agentDocsPath: '/app/agent-docs',
      permissionMode: 'power_user',
    })

    expect(prompt).toContain('ripgrep')
    expect(prompt).toContain('Power User mode is active')
  })

  it('allows shell commands in power user mode where supported', () => {
    const prompt = buildAgentSystemPrompt({ agent: 'codex', permissionMode: 'power_user' })
    expect(prompt).toContain('Power User mode is active')
    expect(prompt).toContain('Local shell commands are available')
    expect(prompt).not.toContain('not available in Vault Safe')
  })

  it('does not promise shell execution for Pi power user mode', () => {
    const prompt = buildAgentSystemPrompt({ agent: 'pi', permissionMode: 'power_user' })
    expect(prompt).toContain('Pi currently uses the same conservative Nabu MCP configuration')
    expect(prompt).not.toContain('Local shell commands are available')
  })

  it('instructs AI to use wikilink syntax', () => {
    const prompt = buildAgentSystemPrompt()
    expect(prompt).toContain('[[')
    expect(prompt).toMatch(/wikilink/i)
  })

  it('mentions templates, agent memory, and the vault map tools', () => {
    const prompt = buildAgentSystemPrompt()
    expect(prompt).toContain('list_templates()')
    expect(prompt).toContain('use_template(name, params)')
    expect(prompt).toContain('read_agents_md()')
    expect(prompt).toContain('read_vault_map()')
    expect(prompt).toContain('update_soul()')
  })

  it('describes the mini app builder workflow in mini_app_builder mode', () => {
    const prompt = buildAgentSystemPrompt({ permissionMode: 'mini_app_builder' })
    expect(prompt).toContain('Mini App Builder mode is active')
    expect(prompt).toContain('.apps/{id}/')
    expect(prompt).toContain('allow_vault_access')
    expect(prompt).not.toContain('Vault Safe mode is active')
  })

  it('tells RAG-mode agents to cite the source vault for retrieved notes', () => {
    const prompt = buildAgentSystemPrompt({ permissionMode: 'rag' })

    expect(prompt).toContain('RAG (Semantic Search) mode is active')
    expect(prompt).toMatch(/Cite the source vault by name/) 
    expect(prompt).toContain('Vault: <label>')
    expect(prompt).toContain('query_vault_rag')
  })

  it('does not add vault-citation instructions outside RAG mode', () => {
    const safePrompt = buildAgentSystemPrompt({ permissionMode: 'safe' })
    const powerPrompt = buildAgentSystemPrompt({ agent: 'codex', permissionMode: 'power_user' })

    expect(safePrompt).not.toMatch(/Cite the source vault by name/)
    expect(powerPrompt).not.toMatch(/Cite the source vault by name/)
  })
})
