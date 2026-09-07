import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useMcpStatus } from './useMcpStatus'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

const runtimeMock = vi.hoisted(() => ({
  isTauri: false,
}))

vi.mock('../mock-tauri', () => ({
  isTauri: () => runtimeMock.isTauri,
  mockInvoke: vi.fn(),
}))

const { invoke } = await import('@tauri-apps/api/core') as { invoke: ReturnType<typeof vi.fn> }
const { mockInvoke } = await import('../mock-tauri') as { mockInvoke: ReturnType<typeof vi.fn> }

function mockCommands(handlers: Partial<Record<string, unknown>>) {
  mockInvoke.mockImplementation((command: string) => {
    if (command in handlers) return Promise.resolve(handlers[command])
    return Promise.resolve(null)
  })
}

function renderSubject(onToast = vi.fn()) {
  return renderHook(() => useMcpStatus('/vault', onToast))
}

function mockClipboard(writeText = vi.fn(() => Promise.resolve())) {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText },
  })
  return writeText
}

function mockStatusFlow(
  initialStatus: 'installed' | 'not_installed',
  overrides: Partial<Record<'register_mcp_tools' | 'remove_mcp_tools', unknown>> = {},
) {
  mockInvoke.mockImplementation((command: string) => {
    if (command === 'check_mcp_status') return Promise.resolve(initialStatus)
    if (command in overrides) {
      const result = overrides[command as keyof typeof overrides]
      if (result instanceof Error) return Promise.reject(result)
      return Promise.resolve(result)
    }
    return Promise.resolve(null)
  })
}

async function renderReadySubject(initialStatus: 'installed' | 'not_installed') {
  const onToast = vi.fn()
  const hook = renderSubject(onToast)
  await waitFor(() => {
    expect(hook.result.current.mcpStatus).toBe(initialStatus)
  })
  return { onToast, ...hook }
}

async function runMutationScenario({
  action,
  initialStatus,
  overrideKey,
  overrideValue,
}: {
  action: 'connect' | 'disconnect'
  initialStatus: 'installed' | 'not_installed'
  overrideKey: 'register_mcp_tools' | 'remove_mcp_tools'
  overrideValue: unknown
}) {
  mockStatusFlow(initialStatus, { [overrideKey]: overrideValue })
  const hook = await renderReadySubject(initialStatus)

  await act(async () => {
    if (action === 'connect') {
      await hook.result.current.connectMcp()
      return
    }
    await hook.result.current.disconnectMcp()
  })

  return hook
}

function standardMcpSnippet() {
  return JSON.stringify({ mcpServers: { nabu: { type: 'stdio' } } })
}

function opencodeMcpSnippet() {
  return JSON.stringify({ mcp: { nabu: { command: ['node', 'index.js'], type: 'local' } } })
}

async function copySnippetInBrowser(kind: 'standard' | 'opencode') {
  const writeText = mockClipboard()
  const onToast = vi.fn()
  const snippet = standardMcpSnippet()
  const opencodeSnippet = opencodeMcpSnippet()
  mockCommands({
    check_mcp_status: 'not_installed',
    get_mcp_config_snippet: snippet,
    get_opencode_mcp_config_snippet: opencodeSnippet,
  })
  const { result } = renderSubject(onToast)

  await waitFor(() => {
    expect(result.current.mcpStatus).toBe('not_installed')
  })

  await act(async () => {
    const copy = kind === 'opencode'
      ? result.current.copyOpenCodeMcpConfig
      : result.current.copyMcpConfig
    await expect(copy()).resolves.toBe(true)
  })

  return {
    expectedSnippet: kind === 'opencode' ? opencodeSnippet : snippet,
    onToast,
    writeText,
  }
}

describe('useMcpStatus', () => {
  beforeEach(() => {
    runtimeMock.isTauri = false
    vi.clearAllMocks()
    mockClipboard()
  })

  it('checks the active vault status without auto-registering on mount', async () => {
    mockCommands({
      check_mcp_status: 'installed',
    })

    const { result } = renderSubject()

    expect(result.current.mcpStatus).toBe('checking')

    await waitFor(() => {
      expect(result.current.mcpStatus).toBe('installed')
    })

    expect(mockInvoke).toHaveBeenCalledWith('check_mcp_status', { vaultPath: '/vault' })
    expect(mockInvoke).not.toHaveBeenCalledWith('register_mcp_tools', { vaultPath: '/vault' })
  })

  it('resolves to not_installed when the active vault is not connected', async () => {
    mockCommands({
      check_mcp_status: 'not_installed',
    })

    const { result } = renderSubject()

    await waitFor(() => {
      expect(result.current.mcpStatus).toBe('not_installed')
    })
  })

  it.each([
    {
      action: 'connect' as const,
      commandArgs: { vaultPath: '/vault' },
      expectedStatus: 'installed' as const,
      initialStatus: 'not_installed' as const,
      name: 'connects external AI tools for the current vault on demand',
      overrideKey: 'register_mcp_tools' as const,
      overrideValue: 'registered',
      toastFragment: 'Nabu external AI tools connected successfully',
    },
    {
      action: 'connect' as const,
      commandArgs: { vaultPath: '/vault' },
      expectedStatus: 'not_installed' as const,
      initialStatus: 'not_installed' as const,
      name: 'reports setup failures without mutating the active status silently',
      overrideKey: 'register_mcp_tools' as const,
      overrideValue: new Error('disk full'),
      toastFragment: 'External AI tool setup failed',
    },
    {
      action: 'disconnect' as const,
      commandArgs: undefined,
      expectedStatus: 'not_installed' as const,
      initialStatus: 'installed' as const,
      name: 'disconnects external AI tools explicitly',
      overrideKey: 'remove_mcp_tools' as const,
      overrideValue: 'removed',
      toastFragment: 'Nabu external AI tools disconnected successfully',
    },
    {
      action: 'disconnect' as const,
      commandArgs: undefined,
      expectedStatus: 'installed' as const,
      initialStatus: 'installed' as const,
      name: 'refreshes status after a disconnect failure',
      overrideKey: 'remove_mcp_tools' as const,
      overrideValue: new Error('permission denied'),
      toastFragment: 'External AI tool disconnect failed',
    },
  ])('$name', async ({ action, commandArgs, expectedStatus, initialStatus, overrideKey, overrideValue, toastFragment }) => {
    const { result, onToast } = await runMutationScenario({
      action,
      initialStatus,
      overrideKey,
      overrideValue,
    })

    expect(result.current.mcpStatus).toBe(expectedStatus)
    expect(mockInvoke).toHaveBeenCalledWith(overrideKey, commandArgs)
    expect(onToast).toHaveBeenCalledWith(expect.stringContaining(toastFragment))
  })

  it('loads the exact manual MCP config snippet for the active vault', async () => {
    const snippet = standardMcpSnippet()
    const opencodeSnippet = opencodeMcpSnippet()
    mockCommands({
      check_mcp_status: 'installed',
      get_mcp_config_snippet: snippet,
      get_opencode_mcp_config_snippet: opencodeSnippet,
    })
    const { result } = renderSubject()

    await waitFor(() => {
      expect(result.current.mcpStatus).toBe('installed')
    })

    await act(async () => {
      await result.current.loadMcpConfigSnippets()
    })

    expect(result.current.mcpConfigSnippet).toBe(snippet)
    expect(result.current.opencodeMcpConfigSnippet).toBe(opencodeSnippet)
    expect(result.current.mcpConfigError).toBeNull()
    expect(mockInvoke).toHaveBeenCalledWith('get_mcp_config_snippet', { vaultPath: '/vault' })
    expect(mockInvoke).toHaveBeenCalledWith('get_opencode_mcp_config_snippet', { vaultPath: '/vault' })
  })

  it('copies the manual MCP config snippet to the clipboard', async () => {
    const { expectedSnippet, onToast, writeText } = await copySnippetInBrowser('standard')

    expect(writeText).toHaveBeenCalledWith(expectedSnippet)
    expect(onToast).toHaveBeenCalledWith('Nabu MCP config copied to clipboard')
  })

  it('copies the OpenCode MCP config snippet to the clipboard', async () => {
    const { expectedSnippet, onToast, writeText } = await copySnippetInBrowser('opencode')

    expect(writeText).toHaveBeenCalledWith(expectedSnippet)
    expect(onToast).toHaveBeenCalledWith('Nabu MCP config copied to clipboard')
  })

  it('uses the native clipboard command inside the Tauri app', async () => {
    runtimeMock.isTauri = true
    const writeText = mockClipboard()
    const onToast = vi.fn()
    const snippet = standardMcpSnippet()
    const opencodeSnippet = opencodeMcpSnippet()
    invoke.mockImplementation((command: string) => {
      if (command === 'check_mcp_status') return Promise.resolve('not_installed')
      if (command === 'get_mcp_config_snippet') return Promise.resolve(snippet)
      if (command === 'get_opencode_mcp_config_snippet') return Promise.resolve(opencodeSnippet)
      if (command === 'copy_text_to_clipboard') return Promise.resolve(null)
      return Promise.resolve(null)
    })
    const { result } = renderSubject(onToast)

    await waitFor(() => {
      expect(result.current.mcpStatus).toBe('not_installed')
    })

    await act(async () => {
      await expect(result.current.copyMcpConfig()).resolves.toBe(true)
    })

    expect(invoke).toHaveBeenCalledWith('copy_text_to_clipboard', { text: snippet })
    expect(writeText).not.toHaveBeenCalled()
    expect(onToast).toHaveBeenCalledWith('Nabu MCP config copied to clipboard')
  })
})
