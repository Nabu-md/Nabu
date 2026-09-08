import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'

const { invokeMock, isTauriMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isTauriMock: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('../mock-tauri', () => ({ isTauri: isTauriMock }))

import { useAiActivity } from './useAiActivity'

class MockWebSocket {
  static latest: MockWebSocket | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: (() => void) | null = null
  onclose: (() => void) | null = null
  close = vi.fn()
  url: string

  constructor(url: string) {
    this.url = url
    MockWebSocket.latest = this
  }
}

beforeEach(() => {
  MockWebSocket.latest = null
  vi.stubGlobal('WebSocket', MockWebSocket)
  vi.useFakeTimers()
  invokeMock.mockReset()
  invokeMock.mockResolvedValue(undefined)
  isTauriMock.mockReset()
  isTauriMock.mockReturnValue(true)
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

function sendWsMessage(data: Record<string, unknown>) {
  MockWebSocket.latest?.onmessage?.(new MessageEvent('message', { data: JSON.stringify(data) }))
}

describe('useAiActivity', () => {
  it('initializes with null highlight', () => {
    const { result } = renderHook(() => useAiActivity())
    expect(result.current.highlightElement).toBeNull()
    expect(result.current.highlightPath).toBeNull()
  })

  it('connects to ws://localhost:9711', () => {
    renderHook(() => useAiActivity())
    expect(MockWebSocket.latest).not.toBeNull()
    expect(MockWebSocket.latest?.url).toBe('ws://localhost:9711')
  })

  it('sets highlight on ui_action highlight message', () => {
    const { result } = renderHook(() => useAiActivity())
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'highlight', element: 'editor', path: '/vault/test.md' })
    })
    expect(result.current.highlightElement).toBe('editor')
    expect(result.current.highlightPath).toBe('/vault/test.md')
  })

  it('auto-clears highlight after 800ms', () => {
    const { result } = renderHook(() => useAiActivity())
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'highlight', element: 'tab', path: '/vault/note.md' })
    })
    expect(result.current.highlightElement).toBe('tab')
    act(() => { vi.advanceTimersByTime(800) })
    expect(result.current.highlightElement).toBeNull()
    expect(result.current.highlightPath).toBeNull()
  })

  it('resets timer on repeated highlight messages', () => {
    const { result } = renderHook(() => useAiActivity())
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'highlight', element: 'editor' })
    })
    act(() => { vi.advanceTimersByTime(500) })
    // Second message resets the timer
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'highlight', element: 'notelist' })
    })
    expect(result.current.highlightElement).toBe('notelist')
    act(() => { vi.advanceTimersByTime(500) })
    // Still active — only 500ms since the second message
    expect(result.current.highlightElement).toBe('notelist')
    act(() => { vi.advanceTimersByTime(300) })
    expect(result.current.highlightElement).toBeNull()
  })

  it('ignores non-ui_action messages', () => {
    const { result } = renderHook(() => useAiActivity())
    act(() => {
      sendWsMessage({ type: 'other', action: 'highlight', element: 'editor' })
    })
    expect(result.current.highlightElement).toBeNull()
  })

  it('ignores malformed JSON', () => {
    const { result } = renderHook(() => useAiActivity())
    act(() => {
      MockWebSocket.latest?.onmessage?.(new MessageEvent('message', { data: 'not json' }))
    })
    expect(result.current.highlightElement).toBeNull()
  })

  it('closes WebSocket on unmount', () => {
    const { unmount } = renderHook(() => useAiActivity())
    unmount()
    expect(MockWebSocket.latest?.close).toHaveBeenCalled()
  })

  it('handles highlight with no path', () => {
    const { result } = renderHook(() => useAiActivity())
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'highlight', element: 'properties' })
    })
    expect(result.current.highlightElement).toBe('properties')
    expect(result.current.highlightPath).toBeNull()
  })

  it('calls onOpenNote callback on open_note action', () => {
    const onOpenNote = vi.fn()
    renderHook(() => useAiActivity({ onOpenNote }))
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'open_note', path: 'project/foo.md' })
    })
    expect(onOpenNote).toHaveBeenCalledWith('project/foo.md')
  })

  it('calls onOpenTab callback on open_tab action', () => {
    const onOpenTab = vi.fn()
    renderHook(() => useAiActivity({ onOpenTab }))
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'open_tab', path: 'note/bar.md' })
    })
    expect(onOpenTab).toHaveBeenCalledWith('note/bar.md')
  })

  it('calls onSetFilter callback on set_filter action', () => {
    const onSetFilter = vi.fn()
    renderHook(() => useAiActivity({ onSetFilter }))
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'set_filter', filterType: 'Project' })
    })
    expect(onSetFilter).toHaveBeenCalledWith('Project')
  })

  it('calls onVaultChanged callback on vault_changed action', () => {
    const onVaultChanged = vi.fn()
    renderHook(() => useAiActivity({ onVaultChanged }))
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'vault_changed', path: 'note/new.md' })
    })
    expect(onVaultChanged).toHaveBeenCalledWith('note/new.md')
  })

  it('calls onVaultRegistryChanged on vault_registry_changed action', () => {
    const onVaultRegistryChanged = vi.fn()
    renderHook(() => useAiActivity({ onVaultRegistryChanged }))
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'vault_registry_changed', path: '/vault/new', registrationType: 'attach' })
    })
    expect(onVaultRegistryChanged).toHaveBeenCalledWith('/vault/new')
  })

  it('does not call onOpenNote when path is missing', () => {
    const onOpenNote = vi.fn()
    renderHook(() => useAiActivity({ onOpenNote }))
    act(() => {
      sendWsMessage({ type: 'ui_action', action: 'open_note' })
    })
    expect(onOpenNote).not.toHaveBeenCalled()
  })

  it('reconnects on close after delay', () => {
    renderHook(() => useAiActivity())
    const firstWs = MockWebSocket.latest
    act(() => { firstWs?.onclose?.() })
    act(() => { vi.advanceTimersByTime(3000) })
    expect(MockWebSocket.latest).not.toBe(firstWs)
  })

  describe('tool_request relay', () => {
    function renderRelayHook() {
      return renderHook(() => useAiActivity())
    }

    function sendToolRequest(data: Record<string, unknown>) {
      return sendWsMessage({ type: 'tool_request', action: 'search_notes_semantic', id: 'relay-1', ...data })
    }

    it('invokes the Tauri command and responds with results', async () => {
      invokeMock.mockResolvedValue({ results: [{ path: 'a.md', score: 0.9 }], elapsedMs: 12 })
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendToolRequest({ vaultPath: '/vault', query: 'financing', limit: 5 })
      })

      expect(invokeMock).toHaveBeenCalledWith('search_notes_semantic', {
        request: { query: 'financing', vaultPath: '/vault', limit: 5, hideGitignoredFiles: false },
      })
      expect(sent).toHaveLength(1)
      const response = JSON.parse(sent[0])
      expect(response.type).toBe('tool_response')
      expect(response.id).toBe('relay-1')
      expect(response.result).toEqual({ results: [{ path: 'a.md', score: 0.9 }], elapsedMs: 12 })
    })

    it('responds with an error when required fields are missing', async () => {
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendToolRequest({ query: 'no vault path' })
      })

      expect(invokeMock).not.toHaveBeenCalled()
      const response = JSON.parse(sent[0])
      expect(response.error).toContain('vaultPath')
    })

    it('responds with an error when not running inside Tauri', async () => {
      isTauriMock.mockReturnValue(false)
      invokeMock.mockClear()
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendToolRequest({ vaultPath: '/vault', query: 'x' })
      })

      expect(invokeMock).not.toHaveBeenCalled()
      const response = JSON.parse(sent[0])
      expect(response.error).toContain('Tauri unavailable')
    })

    it('ignores tool requests for unsupported actions', async () => {
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendWsMessage({ type: 'tool_request', action: 'delete_everything', id: 'relay-2' })
      })

      expect(invokeMock).not.toHaveBeenCalled()
      expect(sent).toHaveLength(0)
    })

    it('relays evaluate_sheet_with_formulas with the full request shape', async () => {
      invokeMock.mockResolvedValue({ cells: { B4: '3000' }, warnings: [] })
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendWsMessage({
          type: 'tool_request',
          action: 'evaluate_sheet_with_formulas',
          id: 'relay-3',
          csvContent: 'Item,Amount\nAlpha,1000\nBeta,2000',
          cellOverrides: { B4: '=SUM(B2:B3)' },
          dependencies: [{ path: '/vault/b.md', content: '40' }],
          links: [{ sourcePath: '/vault/a.md', target: 'b', targetPath: '/vault/b.md' }],
          maxDepth: 4,
          timezone: 'UTC',
        })
      })

      expect(invokeMock).toHaveBeenCalledWith('evaluate_sheet_with_formulas', {
        request: {
          csvContent: 'Item,Amount\nAlpha,1000\nBeta,2000',
          cellOverrides: { B4: '=SUM(B2:B3)' },
          dependencies: [{ path: '/vault/b.md', content: '40' }],
          links: [{ sourcePath: '/vault/a.md', target: 'b', targetPath: '/vault/b.md' }],
          maxDepth: 4,
          timezone: 'UTC',
        },
      })
      const response = JSON.parse(sent[0])
      expect(response.result).toEqual({ cells: { B4: '3000' }, warnings: [] })
    })

    it('relays create_report with title and vaultPath', async () => {
      invokeMock.mockResolvedValue({ path: 'Research Reports/r.md', content: '# R', sheet: { cells: {}, warnings: [] } })
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendWsMessage({
          type: 'tool_request',
          action: 'create_report',
          id: 'relay-4',
          csvContent: 'A,B\n1,2',
          title: 'My Report',
          vaultPath: '/vault',
        })
      })

      expect(invokeMock).toHaveBeenCalledWith('create_report', {
        request: expect.objectContaining({ title: 'My Report', vaultPath: '/vault' }),
      })
      const response = JSON.parse(sent[0])
      expect(response.result.path).toBe('Research Reports/r.md')
    })

    it('relays crunch_financials with saveNote and guards missing vaultPath', async () => {
      invokeMock.mockResolvedValue({ cells: {}, metrics: [], report: '# FA', notePath: null, warnings: [] })
      renderRelayHook()
      const sent: string[] = []
      MockWebSocket.latest!.send = (raw: string) => { sent.push(raw) }

      await act(async () => {
        sendWsMessage({
          type: 'tool_request',
          action: 'crunch_financials',
          id: 'relay-5',
          csvContent: 'A\n1',
          vaultPath: '/vault',
          saveNote: true,
        })
      })

      expect(invokeMock).toHaveBeenCalledWith('crunch_financials', {
        request: expect.objectContaining({ vaultPath: '/vault', saveNote: true }),
      })

      // saveNote without vaultPath is rejected before invoking.
      invokeMock.mockClear()
      sent.length = 0
      await act(async () => {
        sendWsMessage({
          type: 'tool_request',
          action: 'crunch_financials',
          id: 'relay-6',
          csvContent: 'A\n1',
          saveNote: true,
        })
      })
      expect(invokeMock).not.toHaveBeenCalled()
      const errorResponse = JSON.parse(sent[0])
      expect(errorResponse.error).toContain('vaultPath is required')
    })
  })
})
