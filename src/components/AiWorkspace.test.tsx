import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import type { ReactNode } from 'react'
import { AiWorkspace } from './AiWorkspace'
import { buildAiWorkspaceTargetGroups } from './aiWorkspaceTargetGroups'
import {
  createAiAgentAvailability,
  createMissingAiAgentsStatus,
  type AiAgentReadiness,
  type AiAgentsStatus,
} from '../lib/aiAgents'
import type { AiModelProvider } from '../lib/aiTargets'
import type { AgentStatus } from '../hooks/useCliAiAgent'
import { resetVaultConfigStore } from '../utils/vaultConfigStore'
import type { VaultEntry } from '../types'
import type { VaultAiGuidanceStatus } from '../lib/vaultAiGuidance'
import { setPreferredAgentModel } from '../lib/aiAgentModels'

let mockedAgentStatus: AgentStatus = 'idle'
let mockMessages: ReturnType<typeof import('../hooks/useCliAiAgent').useCliAiAgent>['messages'] = []
let controllerCalls: unknown[] = []
const addLocalMarkerMock = vi.fn()
const { generateTitleMock } = vi.hoisted(() => ({
  generateTitleMock: vi.fn(),
}))

const localStorageMock = (() => {
  let store: Record<string, string> = {}
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => { store[key] = value },
    removeItem: (key: string) => { delete store[key] },
    clear: () => { store = {} },
  }
})()

Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock, configurable: true, writable: true })

vi.mock('./useAiPanelController', () => ({
  useAiPanelController: (args: unknown) => {
    controllerCalls.push(args)
    return {
      agent: {
        messages: mockMessages,
        status: mockedAgentStatus,
        sendMessage: vi.fn(),
        clearConversation: vi.fn(),
        addLocalMarker: addLocalMarkerMock,
      },
      input: '',
      setInput: vi.fn(),
      linkedEntries: [],
      hasContext: false,
      isActive: false,
      permissionMode: 'safe',
      handleSend: vi.fn(),
      handleNavigateWikilink: vi.fn(),
      handlePermissionModeChange: vi.fn(),
      handleNewChat: vi.fn(),
    }
  },
}))

vi.mock('./AiPanel', () => ({
  AiPanelView: ({
    composerControls,
    defaultAiAgentReadiness,
    defaultAiAgentReady,
    onMessageHistoryScrollStateChange,
    onSendPrompt,
    showHeader,
  }: {
    composerControls?: ReactNode
    defaultAiAgentReadiness?: AiAgentReadiness
    defaultAiAgentReady?: boolean
    onMessageHistoryScrollStateChange?: (scrolled: boolean) => void
    onSendPrompt?: (prompt: string) => void
    showHeader?: boolean
  }) => (
    <div
      data-testid="ai-panel-view"
      data-agent-readiness={defaultAiAgentReadiness}
      data-agent-ready={String(defaultAiAgentReady)}
      data-show-header={String(showHeader)}
    >
      <button type="button" onClick={() => onSendPrompt?.('summarize quarterly sponsor outreach')}>
        Send mocked prompt
      </button>
      <button type="button" onClick={() => onMessageHistoryScrollStateChange?.(true)}>
        Mock history scroll
      </button>
      {composerControls}
    </div>
  ),
}))

vi.mock('../utils/aiConversationTitle', () => ({
  generateAiConversationTitleForTarget: generateTitleMock,
}))

function installedStatuses(): AiAgentsStatus {
  return {
    ...createMissingAiAgentsStatus(),
    claude_code: createAiAgentAvailability('installed', '1.0.0'),
    codex: createAiAgentAvailability('installed', '0.9.0'),
    antigravity: createAiAgentAvailability('missing', null),
  }
}

const providers: AiModelProvider[] = [
  {
    id: 'ollama-local',
    name: 'Ollama',
    kind: 'ollama',
    api_key_storage: 'none',
    models: [{
      id: 'llama3.2',
      display_name: 'Llama 3.2',
      capabilities: { streaming: true, tools: false, vision: false, json_mode: false, reasoning: false },
    }],
  },
  {
    id: 'openai',
    name: 'OpenAI',
    kind: 'open_ai',
    api_key_storage: 'env',
    api_key_env_var: 'OPENAI_API_KEY',
    models: [{
      id: 'gpt-4.1',
      display_name: 'GPT-4.1',
      capabilities: { streaming: true, tools: true, vision: true, json_mode: true, reasoning: true },
    }],
  },
]

function makeEntry(overrides: Partial<VaultEntry> = {}): VaultEntry {
  return {
    aliases: [],
    archived: false,
    belongsTo: [],
    color: null,
    createdAt: 1700000000,
    favorite: false,
    favoriteIndex: null,
    fileSize: 100,
    filename: 'active.md',
    hasH1: false,
    icon: null,
    isA: 'Note',
    listPropertiesDisplay: [],
    modifiedAt: 1700000000,
    order: null,
    organized: false,
    outgoingLinks: [],
    path: '/tmp/vault/active.md',
    properties: {},
    relatedTo: [],
    relationships: {},
    sidebarLabel: null,
    snippet: '',
    sort: null,
    status: null,
    template: null,
    title: 'Active',
    view: null,
    visible: null,
    wordCount: 0,
    ...overrides,
  }
}

function contextReadyControllerCalls(): unknown[] {
  return controllerCalls.filter((args) => {
    const call = args as { activeEntry?: unknown; entries?: unknown[] }
    return call.activeEntry !== null && call.activeEntry !== undefined && Array.isArray(call.entries)
  })
}

describe('AiWorkspace', () => {
  beforeEach(() => {
    mockedAgentStatus = 'idle'
    mockMessages = []
    controllerCalls = []
    addLocalMarkerMock.mockReset()
    generateTitleMock.mockReset()
    generateTitleMock.mockResolvedValue('Quarterly sponsor outreach')
    localStorage.clear()
    resetVaultConfigStore()
  })

  it('groups installed agents and configured local/API models', () => {
    const groups = buildAiWorkspaceTargetGroups(installedStatuses(), providers)

    expect(groups.localAgents.map((target) => target.agent)).toEqual(['claude_code', 'codex'])
    expect(groups.localAgents.some((target) => target.agent === 'antigravity')).toBe(false)
    expect(groups.localModels.map((target) => target.shortLabel)).toEqual(['Llama 3.2'])
    expect(groups.apiModels.map((target) => target.shortLabel)).toEqual(['GPT-4.1'])
  })

  it('uses parent readiness for browser mock agents when raw CLI status is missing', () => {
    render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={createMissingAiAgentsStatus()}
        aiModelProviders={[]}
        defaultAiAgentReady
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByTestId('ai-panel-view')).toHaveAttribute('data-agent-readiness', 'ready')
    expect(screen.getByTestId('ai-panel-view')).toHaveAttribute('data-agent-ready', 'true')
    expect(controllerCalls[0]).toEqual(expect.objectContaining({
      defaultAiAgentReadiness: 'ready',
      defaultAiAgentReady: true,
    }))
  })

  it('keeps unavailable default agents missing when parent readiness is false', () => {
    render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={createMissingAiAgentsStatus()}
        aiModelProviders={[]}
        defaultAiAgentReady={false}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByTestId('ai-panel-view')).toHaveAttribute('data-agent-readiness', 'missing')
    expect(screen.getByTestId('ai-panel-view')).toHaveAttribute('data-agent-ready', 'false')
    expect(controllerCalls[0]).toEqual(expect.objectContaining({
      defaultAiAgentReadiness: 'missing',
      defaultAiAgentReady: false,
    }))
  })

  it('creates chats from the sidebar and hides the legacy AI panel header', () => {
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    const workspace = screen.getByTestId('ai-workspace')
    expect(workspace).toHaveAttribute('data-ai-workspace-mode', 'docked')
    expect(workspace).toHaveStyle({ width: '560px' })
    expect(workspace.className).toContain('bottom-[30px]')
    expect(workspace.className).not.toContain('shadow')
    expect(screen.getByTestId('ai-panel-view')).toHaveAttribute('data-show-header', 'false')
    expect(screen.queryByText('Agents')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Expand AI chat list' }))
    expect(screen.getByText('Agents')).toBeTruthy()
    expect(screen.queryByText('AI Agent')).toBeNull()
    expect(screen.queryByText('Idle')).toBeNull()

    fireEvent.click(screen.getByTestId('ai-workspace-sidebar-new-chat'))

    expect(screen.getAllByText('AI Chat').length).toBeGreaterThan(0)
    expect(screen.getAllByText('AI Chat 2').length).toBeGreaterThan(0)
  })

  it('renders side mode as an in-editor tabbed panel and expands in place', () => {
    const onClose = vi.fn()
    render(<AiWorkspace open mode="side" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={onClose} />)

    const workspace = screen.getByTestId('ai-workspace')
    expect(workspace).toHaveAttribute('data-ai-workspace-mode', 'side')
    expect(workspace).toHaveAttribute('data-ai-workspace-expanded', 'false')
    expect(workspace).not.toHaveClass('fixed')
    expect(workspace).toHaveClass('bg-sidebar')
    expect(workspace).toHaveStyle({ width: '360px', minWidth: '320px' })
    const header = screen.getByTestId('ai-workspace-side-header')
    const tabStrip = screen.getByTestId('ai-workspace-side-tabs')
    expect(header).not.toHaveClass('border-b')
    fireEvent.click(screen.getByRole('button', { name: 'Mock history scroll' }))
    expect(header).toHaveClass('border-b')
    expect(tabStrip).toHaveClass('overflow-x-auto')
    expect(screen.getByRole('button', { name: 'AI Chat' })).toBeTruthy()
    expect(
      within(header).getByRole('button', { name: 'Expand AI workspace' }).compareDocumentPosition(
        within(header).getByRole('button', { name: 'Close AI workspace' }),
      ) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'New chat' }))
    expect(screen.getByRole('button', { name: 'AI Chat 2' })).toBeTruthy()
    expect(
      within(tabStrip).getByRole('button', { name: 'AI Chat 2' }).compareDocumentPosition(
        within(tabStrip).getByRole('button', { name: 'New chat' }),
      ) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Close AI Chat 2' }))
    expect(screen.queryByRole('button', { name: 'AI Chat 2' })).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'Expand AI workspace' }))
    expect(workspace).toHaveAttribute('data-ai-workspace-expanded', 'true')
    expect(workspace).toHaveClass('absolute')
    expect(screen.getByRole('button', { name: 'Restore AI workspace panel' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Close AI workspace' }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('opens AI settings from the composer controls', () => {
    const onOpenAiSettings = vi.fn()
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onOpenAiSettings={onOpenAiSettings}
      />,
    )

    fireEvent.click(screen.getByTestId('ai-workspace-composer-settings'))

    expect(onOpenAiSettings).toHaveBeenCalledOnce()
  })

  it('reports the active side chat so reopening can restore it', () => {
    const onActiveConversationChange = vi.fn()

    const { unmount } = render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        conversationSettings={[
          { id: 'chat-a', title: 'Planning notes', target_id: null, archived: false },
          { id: 'chat-b', title: 'Follow-up draft', target_id: null, archived: false },
        ]}
        initialActiveConversationId="chat-a"
        vaultPath="/tmp/vault"
        onActiveConversationChange={onActiveConversationChange}
        onClose={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Follow-up draft' }))
    expect(onActiveConversationChange).toHaveBeenLastCalledWith('chat-b')
    unmount()

    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        conversationSettings={[
          { id: 'chat-a', title: 'Planning notes', target_id: null, archived: false },
          { id: 'chat-b', title: 'Follow-up draft', target_id: null, archived: false },
        ]}
        initialActiveConversationId="chat-b"
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByRole('button', { name: 'Follow-up draft' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('renames a side chat from the tab on double-click', () => {
    const onConversationSettingsChange = vi.fn()
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        conversationSettings={[{ id: 'chat-a', title: 'AI Chat', target_id: null, archived: false }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onConversationSettingsChange={onConversationSettingsChange}
      />,
    )

    fireEvent.doubleClick(screen.getByRole('button', { name: 'AI Chat' }))
    const input = screen.getByLabelText('Rename chat')
    fireEvent.change(input, { target: { value: 'Research thread' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(screen.getByRole('button', { name: 'Research thread' })).toBeTruthy()
    expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ id: 'chat-a', title: 'Research thread' }),
    ])
  })

  it('resizes the docked workspace from the left edge and the sidebar split', () => {
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    const workspace = screen.getByTestId('ai-workspace')
    const horizontalResizeHandle = screen.getByTestId('ai-workspace-left-resize')
    expect(horizontalResizeHandle).toHaveClass('inset-y-0', 'h-auto', 'min-h-0', 'w-2', 'rounded-none', 'p-0')
    expect(screen.getByTestId('ai-workspace-top-resize')).toHaveClass('h-2', 'min-h-0', 'w-auto', 'rounded-none', 'p-0')
    fireEvent.click(screen.getByRole('button', { name: 'Expand AI chat list' }))
    fireEvent.mouseDown(horizontalResizeHandle, { clientX: 100, clientY: 20 })
    fireEvent.mouseMove(window, { clientX: 60, clientY: 20 })
    fireEvent.mouseUp(window)
    expect(workspace).toHaveStyle({ width: '600px' })

    const sidebar = screen.getByTestId('ai-workspace-sidebar-header').parentElement
    const sidebarHandle = workspace.querySelector('.cursor-col-resize:not([data-testid])')
    expect(sidebar).toHaveStyle({ width: '168px' })
    fireEvent.mouseDown(sidebarHandle as Element, { clientX: 100, clientY: 20 })
    fireEvent.mouseMove(document, { clientX: 120, clientY: 20 })
    fireEvent.mouseUp(document)
    expect(sidebar).toHaveStyle({ width: '188px' })
  })

  it('restores the last resized side panel width when reopened', () => {
    const { unmount } = render(<AiWorkspace open mode="side" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    const workspace = screen.getByTestId('ai-workspace')
    fireEvent.mouseDown(screen.getByTestId('ai-workspace-left-resize'), { clientX: 100, clientY: 20 })
    fireEvent.mouseMove(window, { clientX: 40, clientY: 20 })
    fireEvent.mouseUp(window)
    expect(workspace).toHaveStyle({ width: '420px' })

    unmount()
    render(<AiWorkspace open mode="side" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    expect(screen.getByTestId('ai-workspace')).toHaveStyle({ width: '420px' })
  })

  it('separates the guidance warning from the header and uses a short restore action', () => {
    const status: VaultAiGuidanceStatus = {
      agentsState: 'missing',
      claudeState: 'managed',
      geminiState: 'managed',
      canRestore: true,
    }

    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        vaultAiGuidanceStatus={status}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onRestoreVaultAiGuidance={vi.fn()}
      />,
    )

    expect(screen.getByText('Vault guidance needs attention: Nabu guidance missing or broken')).toHaveClass('min-w-0')
    expect(screen.getByRole('button', { name: 'Restore' })).toBeTruthy()
    expect(screen.getByText('Vault guidance needs attention: Nabu guidance missing or broken').parentElement).toHaveClass('border-y')
  })

  it('does not archive an empty chat', () => {
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    const archiveButtons = screen.getAllByRole('button', { name: 'Archive chat' })
    expect(archiveButtons.every((button) => button.hasAttribute('disabled'))).toBe(true)
    fireEvent.click(archiveButtons[0])

    expect(screen.getAllByText('AI Chat').length).toBeGreaterThan(0)
    expect(screen.queryByText('AI Chat 2')).toBeNull()
  })

  it('activates a visible chat when persisted settings start with an archived chat', () => {
    render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        conversationSettings={[
          { id: 'archived-chat', title: 'Old Chat', target_id: null, archived: true },
          { id: 'visible-chat', title: 'Live Chat', target_id: null, archived: false },
        ]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByTestId('ai-workspace-session-visible-chat')).toHaveClass('flex')
    expect(screen.getByTestId('ai-workspace-session-archived-chat')).toHaveClass('hidden')
  })

  it('passes vault context only to the active conversation controller', () => {
    const entries = [
      makeEntry({ path: '/tmp/vault/active.md', filename: 'active.md', title: 'Active' }),
      makeEntry({ path: '/tmp/vault/related.md', filename: 'related.md', title: 'Related' }),
    ]
    const singleConversation = render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        activeEntry={entries[0]}
        entries={entries}
        conversationSettings={[
          { id: 'active-chat', title: 'Live Chat', target_id: null, archived: false },
        ]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )
    const activeContextCallCount = contextReadyControllerCalls().length
    singleConversation.unmount()
    controllerCalls = []

    render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        activeEntry={entries[0]}
        entries={entries}
        conversationSettings={[
          { id: 'active-chat', title: 'Live Chat', target_id: null, archived: false },
          { id: 'inactive-chat', title: 'Later Chat', target_id: null, archived: false },
        ]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByTestId('ai-workspace-session-active-chat')).toHaveClass('flex')
    expect(screen.getByTestId('ai-workspace-session-inactive-chat')).toHaveClass('hidden')
    expect(contextReadyControllerCalls()).toHaveLength(activeContextCallCount)
  })

  it('shows grouped target choices without missing agents', async () => {
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    const trigger = screen.getByTestId('ai-workspace-target-trigger')
    act(() => {
      trigger.focus()
      fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    })
    const menu = await screen.findByRole('menu')

    expect(within(menu).getByText('Local models')).toBeTruthy()
    expect(within(menu).getByText('API models')).toBeTruthy()
    expect(within(menu).getByText('Claude Code')).toBeTruthy()
    expect(within(menu).getByText('Codex')).toBeTruthy()
    expect(within(menu).queryByText('Antigravity CLI')).toBeNull()
    expect(within(menu).getByText('Ollama · Llama 3.2')).toBeTruthy()
    expect(within(menu).getByText('OpenAI · GPT-4.1')).toBeTruthy()
  })

  it('persists a verified model with the conversation and next invocation', async () => {
    const onConversationSettingsChange = vi.fn()
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        conversationSettings={[{
          id: 'codex-chat',
          title: 'Codex chat',
          target_id: 'agent:codex',
          model_id: null,
          archived: false,
        }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onConversationSettingsChange={onConversationSettingsChange}
      />,
    )

    const trigger = await screen.findByTestId('ai-workspace-target-trigger')
    expect(trigger).toHaveAccessibleName('AI target: Codex, Model: Agent default')
    await waitFor(() => expect(trigger).toHaveAttribute('aria-busy', 'false'))
    act(() => {
      trigger.focus()
      fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    })
    fireEvent.click(within(await screen.findByRole('menu')).getByText('GPT-5.6 Sol'))

    await waitFor(() => {
      expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
        expect.objectContaining({ id: 'codex-chat', model_id: 'gpt-5.6-sol' }),
      ])
    })
    expect(controllerCalls.at(-1)).toEqual(expect.objectContaining({ model: 'gpt-5.6-sol' }))
  })

  it('combines installed agents and their models in one composer selector', async () => {
    const onConversationSettingsChange = vi.fn()
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        conversationSettings={[{
          id: 'combined-picker-chat',
          title: 'Combined picker chat',
          target_id: 'agent:codex',
          model_id: null,
          archived: false,
        }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onConversationSettingsChange={onConversationSettingsChange}
      />,
    )

    const trigger = await screen.findByTestId('ai-workspace-target-trigger')
    expect(screen.queryByTestId('ai-workspace-model-trigger')).toBeNull()
    expect(trigger).toHaveAccessibleName('AI target: Codex, Model: Agent default')
    expect(screen.getByTestId('ai-workspace-composer-controls')).toHaveClass('flex', 'items-center')
    await waitFor(() => expect(trigger).toHaveAttribute('aria-busy', 'false'))

    act(() => {
      trigger.focus()
      fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    })
    const menu = await screen.findByRole('menu')
    expect(within(menu).getByText('Claude Code')).toBeTruthy()
    expect(within(menu).getByText('Codex')).toBeTruthy()
    fireEvent.click(within(menu).getByText('GPT-5.6 Sol'))

    await waitFor(() => {
      expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
        expect.objectContaining({
          id: 'combined-picker-chat',
          target_id: 'agent:codex',
          model_id: 'gpt-5.6-sol',
        }),
      ])
    })
    expect(trigger).toHaveAccessibleName('AI target: Codex, Model: GPT-5.6 Sol')
  })

  it('restores independent model preferences when switching agents', async () => {
    setPreferredAgentModel('codex', 'gpt-5.6-terra')
    setPreferredAgentModel('claude_code', 'sonnet')
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        conversationSettings={[{
          id: 'switch-chat',
          title: 'Switch chat',
          target_id: 'agent:codex',
          model_id: 'gpt-5.6-terra',
          archived: false,
        }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    const targetTrigger = await screen.findByTestId('ai-workspace-target-trigger')
    await waitFor(() => {
      expect(targetTrigger).toHaveAccessibleName('AI target: Codex, Model: GPT-5.6 Terra')
      expect(targetTrigger).toHaveAttribute('aria-busy', 'false')
    })
    act(() => {
      targetTrigger.focus()
      fireEvent.keyDown(targetTrigger, { key: 'ArrowDown' })
    })
    fireEvent.click(within(await screen.findByRole('menu')).getByText('Sonnet'))
    await waitFor(() => {
      expect(screen.getByTestId('ai-workspace-target-trigger')).toHaveAccessibleName('AI target: Claude Code, Model: Sonnet')
    })

    const claudeTrigger = screen.getByTestId('ai-workspace-target-trigger')
    act(() => {
      claudeTrigger.focus()
      fireEvent.keyDown(claudeTrigger, { key: 'ArrowDown' })
    })
    fireEvent.click(within(await screen.findByRole('menu')).getByText('GPT-5.6 Terra'))
    await waitFor(() => {
      expect(screen.getByTestId('ai-workspace-target-trigger')).toHaveAccessibleName('AI target: Codex, Model: GPT-5.6 Terra')
    })
  })

  it('falls back visibly when a saved model is no longer available', async () => {
    setPreferredAgentModel('codex', 'removed-model')
    const onConversationSettingsChange = vi.fn()
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        conversationSettings={[{
          id: 'legacy-model-chat',
          title: 'Legacy model chat',
          target_id: 'agent:codex',
          model_id: 'removed-model',
          archived: false,
        }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onConversationSettingsChange={onConversationSettingsChange}
      />,
    )

    await waitFor(() => {
      expect(addLocalMarkerMock).toHaveBeenCalledWith(expect.stringContaining('Agent default'))
    })
    expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ id: 'legacy-model-chat', model_id: null }),
    ])
    expect(controllerCalls.at(-1)).toEqual(expect.objectContaining({ model: undefined }))
  })

  it('locks the combined agent and model selector while a response is active', async () => {
    mockedAgentStatus = 'thinking'
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        conversationSettings={[{
          id: 'running-chat',
          title: 'Running chat',
          target_id: 'agent:codex',
          model_id: 'gpt-5.6-sol',
          archived: false,
        }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByTestId('ai-workspace-target-trigger')).toBeDisabled()
    expect(screen.queryByTestId('ai-workspace-model-trigger')).toBeNull()
  })

  it('keeps direct API model targets separate from agent model choices', async () => {
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        conversationSettings={[{
          id: 'api-chat',
          title: 'API chat',
          target_id: 'model:openai/gpt-4.1',
          model_id: null,
          archived: false,
        }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    await waitFor(() => {
      expect(screen.queryByTestId('ai-workspace-model-trigger')).toBeNull()
      expect(controllerCalls.at(-1)).toEqual(expect.objectContaining({ model: undefined }))
    })
  })

  it('clamps legacy narrow side-panel widths so the composer remains visible', async () => {
    localStorage.setItem('nabu:ai-workspace-side-width', '240')
    render(
      <AiWorkspace
        open
        mode="side"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByTestId('ai-workspace')).toHaveStyle({ minWidth: '320px', width: '320px' })
    expect(await screen.findByTestId('ai-workspace-target-trigger')).toHaveClass('min-w-0', 'flex-1')
    expect(screen.queryByTestId('ai-workspace-model-trigger')).toBeNull()
    expect(screen.getByTestId('ai-workspace-composer-controls')).toHaveClass('flex', 'items-center', 'overflow-hidden')
  })

  it('reports the selected workspace target to parent surfaces', async () => {
    const onActiveTargetChange = vi.fn()
    render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        vaultPath="/tmp/vault"
        onActiveTargetChange={onActiveTargetChange}
        onClose={vi.fn()}
      />,
    )

    await waitFor(() => {
      expect(onActiveTargetChange).toHaveBeenCalledWith(expect.objectContaining({
        agent: 'claude_code',
        id: 'agent:claude_code',
        kind: 'agent',
      }))
    })

    const trigger = screen.getByTestId('ai-workspace-target-trigger')
    act(() => {
      trigger.focus()
      fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    })
    const menu = await screen.findByRole('menu')
    fireEvent.click(within(menu).getByRole('menuitemradio', { name: 'Codex, Agent default' }))

    await waitFor(() => {
      expect(onActiveTargetChange).toHaveBeenLastCalledWith(expect.objectContaining({
        agent: 'codex',
        id: 'agent:codex',
        kind: 'agent',
      }))
    })
  })

  it('does not repeat active target notifications when equivalent target props are recreated', async () => {
    const onActiveTargetChange = vi.fn()
    const { rerender } = render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={[]}
        defaultAiAgent="codex"
        vaultPath="/tmp/vault"
        onActiveTargetChange={onActiveTargetChange}
        onClose={vi.fn()}
      />,
    )

    await waitFor(() => {
      expect(onActiveTargetChange).toHaveBeenCalledTimes(1)
    })
    expect(onActiveTargetChange).toHaveBeenLastCalledWith(expect.objectContaining({
      agent: 'codex',
      id: 'agent:codex',
      kind: 'agent',
    }))

    await act(async () => {
      rerender(
        <AiWorkspace
          open
          mode="docked"
          aiAgentsStatus={installedStatuses()}
          aiModelProviders={[]}
          defaultAiAgent="codex"
          vaultPath="/tmp/vault"
          onActiveTargetChange={onActiveTargetChange}
          onClose={vi.fn()}
        />,
      )
      await Promise.resolve()
    })

    expect(onActiveTargetChange).toHaveBeenCalledTimes(1)
  })

  it('marks the first chat active when a prompt is submitted', () => {
    const onConversationSettingsChange = vi.fn()
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} onConversationSettingsChange={onConversationSettingsChange} />)

    fireEvent.click(screen.getByText('Send mocked prompt'))

    expect(screen.getAllByText('AI Chat').length).toBeGreaterThan(0)
    expect(screen.getAllByRole('button', { name: 'Archive chat' }).some((button) => !button.hasAttribute('disabled'))).toBe(true)
    expect(generateTitleMock).not.toHaveBeenCalled()
    expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ title: 'AI Chat' }),
    ])
  })

  it('renames the first chat after the first assistant reply and stores conversation settings', async () => {
    mockMessages = [{
      userMessage: 'summarize quarterly sponsor outreach',
      actions: [],
      response: 'The next step is to follow up with the sponsor pipeline.',
      id: 'msg-title',
    }]
    const onConversationSettingsChange = vi.fn()
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} onConversationSettingsChange={onConversationSettingsChange} />)

    await waitFor(() => {
      expect(screen.getAllByText('Quarterly sponsor outreach').length).toBeGreaterThan(0)
    })
    expect(generateTitleMock).toHaveBeenCalledWith(expect.objectContaining({
      assistantResponse: 'The next step is to follow up with the sponsor pipeline.',
      permissionMode: 'safe',
      prompt: 'summarize quarterly sponsor outreach',
      target: expect.objectContaining({ kind: 'agent', agent: 'claude_code' }),
      targetReady: true,
      vaultPath: '/tmp/vault',
    }))
    expect(screen.getAllByRole('button', { name: 'Archive chat' }).some((button) => !button.hasAttribute('disabled'))).toBe(true)
    await waitFor(() => {
      expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
        expect.objectContaining({ title: 'Quarterly sponsor outreach' }),
      ])
    })
  })

  it('renames a persisted default chat title after the first assistant reply', async () => {
    mockMessages = [{
      userMessage: 'summarize quarterly sponsor outreach',
      actions: [],
      response: 'The next step is to follow up with the sponsor pipeline.',
      id: 'msg-title',
    }]
    const onConversationSettingsChange = vi.fn()
    render(
      <AiWorkspace
        open
        mode="docked"
        aiAgentsStatus={installedStatuses()}
        aiModelProviders={providers}
        conversationSettings={[{ id: 'stored-chat', title: 'Chat 1', target_id: null, archived: false }]}
        vaultPath="/tmp/vault"
        onClose={vi.fn()}
        onConversationSettingsChange={onConversationSettingsChange}
      />,
    )

    await waitFor(() => {
      expect(screen.getAllByText('Quarterly sponsor outreach').length).toBeGreaterThan(0)
    })
    await waitFor(() => {
      expect(onConversationSettingsChange.mock.calls.some(([settings]) => (
        settings.some((setting) => (
          setting.id === 'stored-chat'
          && setting.title === 'Quarterly sponsor outreach'
        ))
      ))).toBe(true)
    })
  })

  it('allows a chat title to be renamed from the sidebar', () => {
    const onConversationSettingsChange = vi.fn()
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} onConversationSettingsChange={onConversationSettingsChange} />)

    fireEvent.click(screen.getByRole('button', { name: 'Expand AI chat list' }))
    fireEvent.doubleClick(screen.getByRole('button', { name: /^AI Chat$/i }))
    const input = screen.getByLabelText('Rename chat')
    fireEvent.change(input, { target: { value: 'Sponsor Plan' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(screen.getAllByText('Sponsor Plan').length).toBeGreaterThan(0)
    expect(onConversationSettingsChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ title: 'Sponsor Plan' }),
    ])
  })

  it('opens with the workspace sidebar collapsed and expands from the sidebar header', () => {
    render(<AiWorkspace open mode="docked" aiAgentsStatus={installedStatuses()} aiModelProviders={providers} vaultPath="/tmp/vault" onClose={vi.fn()} />)

    expect(screen.queryByText('Agents')).toBeNull()
    expect(screen.getByRole('button', { name: 'Expand AI chat list' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Expand AI chat list' }))

    expect(screen.getByText('Agents')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Collapse AI chat list' })).toBeTruthy()
  })
})
