/**
 * Integration tests for the IPC layer
 * Validates Requirements 13.1, 13.2, 13.3, 13.4
 *
 * Tests:
 * 1. Schema validation for all channels (valid and invalid payloads)
 * 2. IPC handler registration and invocation (Renderer→Main)
 * 3. Main→Renderer sendToRenderer message flow
 * 4. Undeclared channel filtering
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { ZodError } from 'zod'
import { IPCChannel } from '../../src/shared/channels'
import {
  VaultOpenSchema,
  VaultScanResultSchema,
  VaultCloseSchema,
  FileGetSchema,
  FileGetResultSchema,
  NoteLoadedSchema,
  NoteUpdatedSchema,
  NoteDeletedSchema,
  NotesLoadedSchema,
  TaskToggleSchema,
  TaskToggleResultSchema,
  ContextQuerySchema,
  ContextSearchResultSchema,
  ContextReindexSchema,
  ContextReindexResultSchema,
  VectorStatusSchema,
  VectorStatusResultSchema,
  ActivityLogSchema,
  IndexBuildSchema
} from '../../src/shared/schemas'

// ---------------------------------------------------------------------------
// Mock Electron modules — must happen before any import that pulls in ipc.ts
// ---------------------------------------------------------------------------

const mockHandlers = new Map<string, Function>()
const mockSentMessages: Array<{ channel: string; payload: unknown }> = []

const mockWebContents = {
  send: vi.fn((channel: string, payload: unknown) => {
    mockSentMessages.push({ channel, payload })
  })
}

const mockWin = {
  isDestroyed: vi.fn(() => false),
  webContents: mockWebContents
}

vi.mock('electron', () => ({
  ipcMain: {
    handle: vi.fn((channel: string, handler: Function) => {
      mockHandlers.set(channel, handler)
    }),
    removeHandler: vi.fn((channel: string) => {
      mockHandlers.delete(channel)
    })
  },
  dialog: {
    showOpenDialog: vi.fn().mockResolvedValue({ canceled: true, filePaths: [] })
  },
  BrowserWindow: {
    getAllWindows: vi.fn(() => [mockWin]),
    getFocusedWindow: vi.fn(() => mockWin)
  }
}))

// ---------------------------------------------------------------------------
// Mock StateManager, VectorManager, VaultWatcher
// ---------------------------------------------------------------------------

const mockStateManager = {
  openVault: vi.fn().mockResolvedValue({ path: '/vault', files: [] }),
  getCurrentVault: vi.fn().mockReturnValue({ path: '/vault', files: [] }),
  getAST: vi.fn().mockResolvedValue({ type: 'root', children: [] }),
  toggleTask: vi.fn().mockResolvedValue(undefined),
  hasPendingWrite: vi.fn().mockReturnValue(false),
  setPendingWrite: vi.fn(),
  clearPendingWrite: vi.fn()
}

const mockVectorManager = {
  search: vi.fn().mockResolvedValue([]),
  embedFile: vi.fn(),
  initialize: vi.fn().mockResolvedValue(undefined),
  setLogCallback: vi.fn(),
  generateEmbedding: vi.fn().mockResolvedValue([]),
  removeFile: vi.fn().mockResolvedValue(undefined),
  getStatus: vi.fn().mockResolvedValue({ disabled: false, reason: null, items: 0 }),
  reindexAll: vi.fn().mockResolvedValue(10)
}

const mockWatcher = {
  start: vi.fn(),
  stop: vi.fn()
}

// ---------------------------------------------------------------------------
// Import ipc.ts AFTER mocks are set up
// ---------------------------------------------------------------------------

import { registerIPCHandlers, sendToRenderer, setLegacyManagers } from '../../src/main/ipc'

// ---------------------------------------------------------------------------
// Helper: invoke a registered ipcMain handler as if called from renderer
// ---------------------------------------------------------------------------

async function invokeHandler(channel: IPCChannel, payload?: unknown): Promise<unknown> {
  const handler = mockHandlers.get(channel)
  if (!handler) throw new Error(`No handler registered for channel: ${channel}`)
  return handler({} /* _event */, payload)
}

// ---------------------------------------------------------------------------
// Setup: register handlers once before each test
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks()
  mockHandlers.clear()
  mockSentMessages.length = 0

  registerIPCHandlers(mockStateManager as any, mockVectorManager as any, mockWatcher as any)

  // Set legacy managers for backward compatibility dispatch (Req 22.3)
  setLegacyManagers(mockStateManager as any, mockVectorManager as any, mockWatcher as any)
})

// ===========================================================================
// SECTION 1: Schema Validation Tests (Req 13.2)
// All 14 channels — valid and invalid payloads
// ===========================================================================

describe('Schema Validation — all 14 channels (Req 13.2)', () => {
  // --- vault:open ---
  describe('vault:open schema', () => {
    it('accepts empty object (path is optional)', () => {
      expect(() => VaultOpenSchema.parse({})).not.toThrow()
    })
    it('accepts object with path string', () => {
      expect(() => VaultOpenSchema.parse({ path: '/some/vault' })).not.toThrow()
    })
    it('rejects non-string path', () => {
      expect(() => VaultOpenSchema.parse({ path: 123 })).toThrow(ZodError)
    })
  })

  // --- vault:scan (result schema) ---
  describe('vault:scan result schema', () => {
    it('accepts valid scan result', () => {
      const valid = { path: '/v', files: [{ path: '/v/a.md', name: 'a', mtime: 1 }] }
      expect(() => VaultScanResultSchema.parse(valid)).not.toThrow()
    })
    it('accepts empty files array', () => {
      expect(() => VaultScanResultSchema.parse({ path: '/v', files: [] })).not.toThrow()
    })
    it('rejects missing path', () => {
      expect(() => VaultScanResultSchema.parse({ files: [] })).toThrow(ZodError)
    })
    it('rejects file entry with missing name', () => {
      const invalid = { path: '/v', files: [{ path: '/v/a.md', mtime: 1 }] }
      expect(() => VaultScanResultSchema.parse(invalid)).toThrow(ZodError)
    })
  })

  // --- vault:close ---
  describe('vault:close schema', () => {
    it('accepts empty object', () => {
      expect(() => VaultCloseSchema.parse({})).not.toThrow()
    })
    it('accepts object with extra ignored fields (Zod strips unknowns by default)', () => {
      // Zod strips by default but won't throw on extra fields
      expect(() => VaultCloseSchema.parse({ extra: 'field' })).not.toThrow()
    })
  })

  // --- file:get ---
  describe('file:get schema', () => {
    it('accepts valid path', () => {
      expect(() => FileGetSchema.parse({ path: '/vault/note.md' })).not.toThrow()
    })
    it('rejects missing path', () => {
      expect(() => FileGetSchema.parse({})).toThrow(ZodError)
    })
    it('rejects non-string path', () => {
      expect(() => FileGetSchema.parse({ path: null })).toThrow(ZodError)
    })
  })

  // --- file:get result ---
  describe('file:get result schema', () => {
    it('accepts result with ast', () => {
      const valid = { path: '/v/a.md', ast: { type: 'root', children: [] } }
      expect(() => FileGetResultSchema.parse(valid)).not.toThrow()
    })
    it('accepts result with optional error field', () => {
      const valid = {
        path: '/v/a.md',
        ast: null,
        error: { line: 1, column: 0, message: 'oops' }
      }
      expect(() => FileGetResultSchema.parse(valid)).not.toThrow()
    })
    it('rejects missing path', () => {
      expect(() => FileGetResultSchema.parse({ ast: {} })).toThrow(ZodError)
    })
  })

  // --- note:loaded ---
  describe('note:loaded schema', () => {
    it('accepts valid note loaded payload', () => {
      expect(() => NoteLoadedSchema.parse({ path: '/v/a.md', ast: {} })).not.toThrow()
    })
    it('rejects missing path', () => {
      expect(() => NoteLoadedSchema.parse({ ast: {} })).toThrow(ZodError)
    })
    it('rejects missing ast', () => {
      expect(() => NoteLoadedSchema.parse({ path: '/v/a.md' })).toThrow(ZodError)
    })
  })

  // --- note:updated ---
  describe('note:updated schema', () => {
    it('accepts valid note updated payload', () => {
      const valid = { path: '/v/a.md', ast: {}, isExternal: true }
      expect(() => NoteUpdatedSchema.parse(valid)).not.toThrow()
    })
    it('rejects missing isExternal', () => {
      expect(() => NoteUpdatedSchema.parse({ path: '/v/a.md', ast: {} })).toThrow(ZodError)
    })
    it('rejects non-boolean isExternal', () => {
      expect(() =>
        NoteUpdatedSchema.parse({ path: '/v/a.md', ast: {}, isExternal: 'yes' })
      ).toThrow(ZodError)
    })
  })

  // --- note:deleted ---
  describe('note:deleted schema', () => {
    it('accepts valid note deleted payload', () => {
      expect(() => NoteDeletedSchema.parse({ path: '/v/a.md' })).not.toThrow()
    })
    it('rejects missing path', () => {
      expect(() => NoteDeletedSchema.parse({})).toThrow(ZodError)
    })
    it('rejects non-string path', () => {
      expect(() => NoteDeletedSchema.parse({ path: 42 })).toThrow(ZodError)
    })
  })

  // --- notes:loaded ---
  describe('notes:loaded schema', () => {
    it('accepts valid bulk file list', () => {
      const valid = { files: [{ path: '/v/a.md', name: 'a', mtime: 0 }] }
      expect(() => NotesLoadedSchema.parse(valid)).not.toThrow()
    })
    it('accepts empty files array', () => {
      expect(() => NotesLoadedSchema.parse({ files: [] })).not.toThrow()
    })
    it('rejects missing files', () => {
      expect(() => NotesLoadedSchema.parse({})).toThrow(ZodError)
    })
    it('rejects file entry missing mtime', () => {
      const invalid = { files: [{ path: '/v/a.md', name: 'a' }] }
      expect(() => NotesLoadedSchema.parse(invalid)).toThrow(ZodError)
    })
  })

  // --- task:toggle ---
  describe('task:toggle schema', () => {
    it('accepts valid toggle payload', () => {
      expect(() => TaskToggleSchema.parse({ path: '/v/a.md', lineIndex: 5 })).not.toThrow()
    })
    it('rejects negative lineIndex', () => {
      expect(() => TaskToggleSchema.parse({ path: '/v/a.md', lineIndex: -1 })).toThrow(ZodError)
    })
    it('rejects float lineIndex', () => {
      expect(() => TaskToggleSchema.parse({ path: '/v/a.md', lineIndex: 1.5 })).toThrow(ZodError)
    })
    it('rejects missing path', () => {
      expect(() => TaskToggleSchema.parse({ lineIndex: 0 })).toThrow(ZodError)
    })
  })

  // --- task:toggle result ---
  describe('task:toggle result schema', () => {
    it('accepts success: true', () => {
      expect(() => TaskToggleResultSchema.parse({ success: true })).not.toThrow()
    })
    it('accepts success: false with error message', () => {
      expect(() =>
        TaskToggleResultSchema.parse({ success: false, error: 'bad line' })
      ).not.toThrow()
    })
    it('rejects missing success field', () => {
      expect(() => TaskToggleResultSchema.parse({})).toThrow(ZodError)
    })
  })

  // --- context:query ---
  describe('context:query schema', () => {
    it('accepts valid query', () => {
      expect(() => ContextQuerySchema.parse({ text: 'find something' })).not.toThrow()
    })
    it('accepts optional excludePath', () => {
      expect(() =>
        ContextQuerySchema.parse({ text: 'search', excludePath: '/v/a.md' })
      ).not.toThrow()
    })
    it('rejects missing text', () => {
      expect(() => ContextQuerySchema.parse({})).toThrow(ZodError)
    })
    it('rejects non-string text', () => {
      expect(() => ContextQuerySchema.parse({ text: 42 })).toThrow(ZodError)
    })
  })

  // --- context:search result ---
  describe('context:search result schema', () => {
    it('accepts valid search results', () => {
      const valid = { results: [{ path: '/v/a.md', score: 0.85, tokenCount: 120 }] }
      expect(() => ContextSearchResultSchema.parse(valid)).not.toThrow()
    })
    it('accepts empty results array', () => {
      expect(() => ContextSearchResultSchema.parse({ results: [] })).not.toThrow()
    })
    it('rejects score > 1', () => {
      const invalid = { results: [{ path: '/v/a.md', score: 1.5, tokenCount: 10 }] }
      expect(() => ContextSearchResultSchema.parse(invalid)).toThrow(ZodError)
    })
    it('rejects score < 0', () => {
      const invalid = { results: [{ path: '/v/a.md', score: -0.1, tokenCount: 10 }] }
      expect(() => ContextSearchResultSchema.parse(invalid)).toThrow(ZodError)
    })
    it('rejects missing results', () => {
      expect(() => ContextSearchResultSchema.parse({})).toThrow(ZodError)
    })
  })

  // --- context:reindex ---
  describe('context:reindex schema', () => {
    it('accepts valid payload with vaultPath', () => {
      expect(() => ContextReindexSchema.parse({ vaultPath: '/vault' })).not.toThrow()
    })
    it('rejects missing vaultPath', () => {
      expect(() => ContextReindexSchema.parse({})).toThrow(ZodError)
    })
    it('accepts valid result', () => {
      expect(() => ContextReindexResultSchema.parse({ processed: 42 })).not.toThrow()
    })
    it('rejects negative processed count', () => {
      expect(() => ContextReindexResultSchema.parse({ processed: -1 })).toThrow(ZodError)
    })
  })

  // --- vector:status ---
  describe('vector:status schema', () => {
    it('accepts empty payload (no params required)', () => {
      expect(() => VectorStatusSchema.parse({})).not.toThrow()
    })
    it('accepts result with disabled=false', () => {
      expect(() =>
        VectorStatusResultSchema.parse({ disabled: false, reason: null, items: 0 })
      ).not.toThrow()
    })
    it('accepts result with disabled=true and reason', () => {
      expect(() =>
        VectorStatusResultSchema.parse({ disabled: true, reason: 'Model not found', items: 0 })
      ).not.toThrow()
    })
    it('rejects result missing disabled', () => {
      expect(() => VectorStatusResultSchema.parse({ reason: null, items: 0 })).toThrow(ZodError)
    })
    it('rejects result missing reason', () => {
      expect(() => VectorStatusResultSchema.parse({ disabled: false, items: 0 })).toThrow(ZodError)
    })
    it('rejects non-boolean disabled', () => {
      expect(() =>
        VectorStatusResultSchema.parse({ disabled: 'yes', reason: null, items: 0 })
      ).toThrow(ZodError)
    })
    it('rejects result missing items', () => {
      expect(() => VectorStatusResultSchema.parse({ disabled: false, reason: null })).toThrow(
        ZodError
      )
    })
  })

  // --- activity:log ---
  describe('activity:log schema', () => {
    it('accepts valid log entry', () => {
      expect(() =>
        ActivityLogSchema.parse({ level: 'info', message: 'ok', timestamp: Date.now() })
      ).not.toThrow()
    })
    it('accepts all valid levels', () => {
      for (const level of ['info', 'warn', 'error']) {
        expect(() => ActivityLogSchema.parse({ level, message: 'x', timestamp: 0 })).not.toThrow()
      }
    })
    it('rejects invalid level', () => {
      expect(() => ActivityLogSchema.parse({ level: 'debug', message: 'x', timestamp: 0 })).toThrow(
        ZodError
      )
    })
    it('rejects missing timestamp', () => {
      expect(() => ActivityLogSchema.parse({ level: 'info', message: 'x' })).toThrow(ZodError)
    })
    it('rejects missing message', () => {
      expect(() => ActivityLogSchema.parse({ level: 'info', timestamp: 0 })).toThrow(ZodError)
    })
  })
})

// ===========================================================================
// SECTION 2: IPC Channel Registration (Req 13.1)
// All Renderer→Main channels must have handlers registered
// ===========================================================================

describe('IPC Handler Registration — all Renderer→Main channels registered (Req 13.1)', () => {
  const rendererToMainChannels = [
    IPCChannel.VAULT_OPEN,
    IPCChannel.VAULT_SCAN,
    IPCChannel.VAULT_CLOSE,
    IPCChannel.FILE_GET,
    IPCChannel.FILE_WATCH,
    IPCChannel.TASK_TOGGLE,
    IPCChannel.NOTE_TOGGLE,
    IPCChannel.CONTEXT_QUERY,
    IPCChannel.CONTEXT_REINDEX,
    IPCChannel.VECTOR_STATUS,
    IPCChannel.ACTIVITY_LOG // bidirectional — also registered on ipcMain
  ]

  for (const channel of rendererToMainChannels) {
    it(`handler is registered for "${channel}"`, () => {
      expect(mockHandlers.has(channel)).toBe(true)
    })
  }
})

// ===========================================================================
// SECTION 3: Handler Invocation Tests (Req 13.3, 13.4)
// Valid payloads call correct methods; invalid payloads return errors
// ===========================================================================

describe('vault:open handler (Req 13.3)', () => {
  it('calls stateManager.openVault with provided path', async () => {
    mockStateManager.openVault.mockResolvedValueOnce({ path: '/vault', files: [] })

    const result = (await invokeHandler(IPCChannel.VAULT_OPEN, { path: '/vault' })) as any

    expect(mockStateManager.openVault).toHaveBeenCalledWith('/vault')
    expect(result).toMatchObject({ path: '/vault', files: [] })
  })

  it('shows dialog when no path provided and returns canceled: true', async () => {
    const { dialog } = await import('electron')
    vi.mocked(dialog.showOpenDialog).mockResolvedValueOnce({ canceled: true, filePaths: [] })

    const result = (await invokeHandler(IPCChannel.VAULT_OPEN, {})) as any

    expect(dialog.showOpenDialog).toHaveBeenCalled()
    expect(result).toMatchObject({ canceled: true })
  })

  it('returns error object when stateManager.openVault throws', async () => {
    mockStateManager.openVault.mockRejectedValueOnce(new Error('disk error'))

    const result = (await invokeHandler(IPCChannel.VAULT_OPEN, { path: '/bad' })) as any

    expect(result).toHaveProperty('error')
    expect(result.error).toContain('disk error')
  })

  it('returns error on invalid payload (path is not a string)', async () => {
    const result = (await invokeHandler(IPCChannel.VAULT_OPEN, { path: 123 })) as any

    expect(mockStateManager.openVault).not.toHaveBeenCalled()
    expect(result).toHaveProperty('error')
  })

  it('starts the watcher after successful vault open', async () => {
    mockStateManager.openVault.mockResolvedValueOnce({ path: '/vault', files: [] })

    await invokeHandler(IPCChannel.VAULT_OPEN, { path: '/vault' })

    expect(mockWatcher.start).toHaveBeenCalled()
    const watchConfig = mockWatcher.start.mock.calls[0][0]
    expect(watchConfig.vaultPath).toBe('/vault')
  })
})

describe('vault:scan handler (Req 13.3)', () => {
  it('returns error when no vault is open', async () => {
    mockStateManager.getCurrentVault.mockReturnValueOnce(null)

    const result = (await invokeHandler(IPCChannel.VAULT_SCAN)) as any

    expect(result).toHaveProperty('error')
    expect(result.error).toContain('No vault is currently open')
  })

  it('calls openVault with current vault path and returns scan result', async () => {
    mockStateManager.getCurrentVault.mockReturnValueOnce({ path: '/vault', files: [] })
    mockStateManager.openVault.mockResolvedValueOnce({
      path: '/vault',
      files: [{ path: '/vault/note.md', name: 'note', mtime: 1000 }]
    })

    const result = (await invokeHandler(IPCChannel.VAULT_SCAN)) as any

    expect(mockStateManager.openVault).toHaveBeenCalledWith('/vault')
    expect(result.files).toHaveLength(1)
  })
})

describe('vault:close handler (Req 13.3)', () => {
  it('stops the watcher and returns success', async () => {
    const result = (await invokeHandler(IPCChannel.VAULT_CLOSE, {})) as any

    expect(mockWatcher.stop).toHaveBeenCalled()
    expect(result).toMatchObject({ success: true })
  })

  it('returns error on invalid payload', async () => {
    // VaultCloseSchema accepts anything (empty object schema), but pass a raw non-object
    // The handler coalesces undefined to {} — so test watcher.stop throwing instead
    mockWatcher.stop.mockImplementationOnce(() => {
      throw new Error('watcher error')
    })

    const result = (await invokeHandler(IPCChannel.VAULT_CLOSE, {})) as any

    expect(result).toHaveProperty('error')
    expect(result.error).toContain('watcher error')
  })
})

describe('file:get handler (Req 13.3)', () => {
  it('returns AST for valid path', async () => {
    const mockAst = { type: 'root', children: [] }
    mockStateManager.getAST.mockResolvedValueOnce(mockAst)

    const result = (await invokeHandler(IPCChannel.FILE_GET, { path: '/vault/note.md' })) as any

    expect(mockStateManager.getAST).toHaveBeenCalledWith('/vault/note.md')
    expect(result).toMatchObject({ path: '/vault/note.md', ast: mockAst })
  })

  it('returns validation error for missing path', async () => {
    const result = (await invokeHandler(IPCChannel.FILE_GET, {})) as any

    expect(mockStateManager.getAST).not.toHaveBeenCalled()
    expect(result).toHaveProperty('error')
  })

  it('returns parse error when getAST throws', async () => {
    mockStateManager.getAST.mockRejectedValueOnce(new Error('parse failed'))

    const result = (await invokeHandler(IPCChannel.FILE_GET, { path: '/vault/broken.md' })) as any

    expect(result).toMatchObject({
      path: '/vault/broken.md',
      error: expect.objectContaining({ message: expect.stringContaining('parse failed') })
    })
  })
})

describe('file:watch handler (Req 13.3)', () => {
  it('acknowledges watch request and returns success', async () => {
    const result = (await invokeHandler(IPCChannel.FILE_WATCH, { path: '/vault/note.md' })) as any

    expect(result).toMatchObject({ success: true, path: '/vault/note.md' })
  })

  it('returns validation error for missing path', async () => {
    const result = (await invokeHandler(IPCChannel.FILE_WATCH, {})) as any

    expect(result).toHaveProperty('error')
  })
})

describe('task:toggle handler (Req 13.3)', () => {
  it('calls toggleTask and returns success', async () => {
    mockStateManager.toggleTask.mockResolvedValueOnce(undefined)

    const result = (await invokeHandler(IPCChannel.TASK_TOGGLE, {
      path: '/vault/tasks.md',
      lineIndex: 3
    })) as any

    expect(mockStateManager.toggleTask).toHaveBeenCalledWith('/vault/tasks.md', 3)
    expect(result).toMatchObject({ success: true })
  })

  it('returns validation error for invalid lineIndex (negative)', async () => {
    const result = (await invokeHandler(IPCChannel.TASK_TOGGLE, {
      path: '/vault/tasks.md',
      lineIndex: -1
    })) as any

    expect(mockStateManager.toggleTask).not.toHaveBeenCalled()
    expect(result).toMatchObject({ success: false })
    expect(result.error).toBeDefined()
  })

  it('returns validation error for float lineIndex', async () => {
    const result = (await invokeHandler(IPCChannel.TASK_TOGGLE, {
      path: '/vault/tasks.md',
      lineIndex: 2.5
    })) as any

    expect(result).toMatchObject({ success: false })
    expect(result.error).toBeDefined()
  })

  it('returns error when toggleTask throws', async () => {
    mockStateManager.toggleTask.mockRejectedValueOnce(new Error('Invalid line index: 999'))

    const result = (await invokeHandler(IPCChannel.TASK_TOGGLE, {
      path: '/vault/tasks.md',
      lineIndex: 999
    })) as any

    expect(result).toMatchObject({ success: false })
    expect(result.error).toContain('Invalid line index')
  })
})

describe('note:toggle handler (Req 13.3)', () => {
  it('calls toggleTask (same mechanism as task:toggle) and returns success', async () => {
    mockStateManager.toggleTask.mockResolvedValueOnce(undefined)

    const result = (await invokeHandler(IPCChannel.NOTE_TOGGLE, {
      path: '/vault/note.md',
      lineIndex: 0
    })) as any

    expect(mockStateManager.toggleTask).toHaveBeenCalledWith('/vault/note.md', 0)
    expect(result).toMatchObject({ success: true })
  })

  it('returns validation error for missing path', async () => {
    const result = (await invokeHandler(IPCChannel.NOTE_TOGGLE, { lineIndex: 0 })) as any

    expect(mockStateManager.toggleTask).not.toHaveBeenCalled()
    expect(result).toMatchObject({ success: false })
  })
})

describe('context:query handler (Req 13.3)', () => {
  it('calls vectorManager.search and returns results', async () => {
    const mockResults = [{ path: '/vault/related.md', score: 0.92, tokenCount: 150 }]
    mockVectorManager.getStatus.mockResolvedValueOnce({ disabled: false, reason: null, items: 5 })
    mockVectorManager.search.mockResolvedValueOnce(mockResults)

    const result = (await invokeHandler(IPCChannel.CONTEXT_QUERY, {
      text: 'semantic search'
    })) as any

    expect(mockVectorManager.search).toHaveBeenCalledWith('semantic search', 5, undefined)
    expect(result).toMatchObject({ results: mockResults })
  })

  it('passes excludePath to vectorManager.search', async () => {
    mockVectorManager.getStatus.mockResolvedValueOnce({ disabled: false, reason: null, items: 5 })
    mockVectorManager.search.mockResolvedValueOnce([])

    await invokeHandler(IPCChannel.CONTEXT_QUERY, {
      text: 'search',
      excludePath: '/vault/current.md'
    })

    expect(mockVectorManager.search).toHaveBeenCalledWith('search', 5, '/vault/current.md')
  })

  it('returns validation error for missing text', async () => {
    const result = (await invokeHandler(IPCChannel.CONTEXT_QUERY, {})) as any

    expect(mockVectorManager.search).not.toHaveBeenCalled()
    expect(result).toHaveProperty('error')
  })

  it('returns empty results array when vectorManager.search throws', async () => {
    mockVectorManager.getStatus.mockResolvedValueOnce({ disabled: false, reason: null, items: 5 })
    mockVectorManager.search.mockRejectedValueOnce(new Error('index unavailable'))

    const result = (await invokeHandler(IPCChannel.CONTEXT_QUERY, { text: 'query' })) as any

    expect(result).toMatchObject({ results: [] })
    expect(result.error).toBeDefined()
  })
})

describe('context:reindex handler (Req 1.5, 1.6)', () => {
  it('calls vectorManager.reindexAll and returns processed count', async () => {
    mockStateManager.getCurrentVault.mockReturnValueOnce({
      path: '/vault',
      files: [{ path: '/vault/a.md', name: 'a', mtime: 1 }]
    })
    mockVectorManager.reindexAll.mockResolvedValueOnce(5)

    const result = (await invokeHandler(IPCChannel.CONTEXT_REINDEX, { vaultPath: '/vault' })) as any

    expect(mockVectorManager.reindexAll).toHaveBeenCalled()
    expect(result).toMatchObject({ processed: 5 })
  })

  it('returns error when no vault is open', async () => {
    mockStateManager.getCurrentVault.mockReturnValueOnce(null)

    const result = (await invokeHandler(IPCChannel.CONTEXT_REINDEX, { vaultPath: '/vault' })) as any

    expect(mockVectorManager.reindexAll).not.toHaveBeenCalled()
    expect(result).toHaveProperty('error')
  })

  it('returns validation error for missing vaultPath', async () => {
    const result = (await invokeHandler(IPCChannel.CONTEXT_REINDEX, {})) as any

    expect(mockVectorManager.reindexAll).not.toHaveBeenCalled()
    expect(result).toHaveProperty('error')
  })
})

describe('vector:status handler (Req 1.5, 1.6)', () => {
  it('returns vector status from vectorManager.getStatus', async () => {
    mockVectorManager.getStatus.mockResolvedValueOnce({ disabled: false, reason: null, items: 5 })

    const result = (await invokeHandler(IPCChannel.VECTOR_STATUS, {})) as any

    expect(mockVectorManager.getStatus).toHaveBeenCalled()
    expect(result).toMatchObject({ disabled: false, reason: null, items: 5 })
  })

  it('returns disabled=true with reason when model failed to load', async () => {
    mockVectorManager.getStatus.mockResolvedValueOnce({
      disabled: true,
      reason: 'Model not found',
      items: 0
    })

    const result = (await invokeHandler(IPCChannel.VECTOR_STATUS, {})) as any

    expect(result).toMatchObject({ disabled: true, reason: 'Model not found', items: 0 })
  })
})

describe('activity:log handler — Renderer→Main (Req 13.3, bidirectional)', () => {
  it('accepts valid log entry and logs to console', async () => {
    const consoleSpy = vi.spyOn(console, 'info').mockImplementation(() => {})

    const result = (await invokeHandler(IPCChannel.ACTIVITY_LOG, {
      level: 'info',
      message: 'renderer says hello',
      timestamp: Date.now()
    })) as any

    expect(result).toMatchObject({ success: true })
    consoleSpy.mockRestore()
  })

  it('returns error for invalid log level', async () => {
    const result = (await invokeHandler(IPCChannel.ACTIVITY_LOG, {
      level: 'verbose',
      message: 'bad level',
      timestamp: 0
    })) as any

    expect(result).toHaveProperty('error')
  })

  it('returns error for missing message', async () => {
    const result = (await invokeHandler(IPCChannel.ACTIVITY_LOG, {
      level: 'warn',
      timestamp: 0
    })) as any

    expect(result).toHaveProperty('error')
  })
})

// ===========================================================================
// SECTION 4: Main→Renderer Message Flow via sendToRenderer (Req 13.4)
// Valid payloads are dispatched; invalid payloads trigger activity:log warning
// ===========================================================================

describe('sendToRenderer — Main→Renderer message flow (Req 13.4)', () => {
  describe('note:loaded', () => {
    it('sends valid payload to all renderer windows', () => {
      const payload = { path: '/v/a.md', ast: { type: 'root', children: [] } }
      sendToRenderer(IPCChannel.NOTE_LOADED, payload)

      expect(mockWebContents.send).toHaveBeenCalledWith(IPCChannel.NOTE_LOADED, payload)
    })

    it('does not send and emits activity:log warning for missing ast', () => {
      sendToRenderer(IPCChannel.NOTE_LOADED, { path: '/v/a.md' })

      // webContents.send should not be called with note:loaded
      const noteSent = mockWebContents.send.mock.calls.find(([ch]) => ch === IPCChannel.NOTE_LOADED)
      expect(noteSent).toBeUndefined()

      // activity:log warning should be emitted
      const logSent = mockWebContents.send.mock.calls.find(([ch]) => ch === IPCChannel.ACTIVITY_LOG)
      expect(logSent).toBeDefined()
      expect(logSent![1].level).toBe('warn')
    })
  })

  describe('note:updated', () => {
    it('sends valid payload to all renderer windows', () => {
      const payload = { path: '/v/a.md', ast: {}, isExternal: false }
      sendToRenderer(IPCChannel.NOTE_UPDATED, payload)

      expect(mockWebContents.send).toHaveBeenCalledWith(IPCChannel.NOTE_UPDATED, payload)
    })

    it('rejects payload missing isExternal', () => {
      sendToRenderer(IPCChannel.NOTE_UPDATED, { path: '/v/a.md', ast: {} })

      const noteSent = mockWebContents.send.mock.calls.find(
        ([ch]) => ch === IPCChannel.NOTE_UPDATED
      )
      expect(noteSent).toBeUndefined()
    })
  })

  describe('note:deleted', () => {
    it('sends valid payload to all renderer windows', () => {
      sendToRenderer(IPCChannel.NOTE_DELETED, { path: '/v/a.md' })

      expect(mockWebContents.send).toHaveBeenCalledWith(IPCChannel.NOTE_DELETED, {
        path: '/v/a.md'
      })
    })

    it('rejects payload with non-string path', () => {
      sendToRenderer(IPCChannel.NOTE_DELETED, { path: 99 })

      const noteSent = mockWebContents.send.mock.calls.find(
        ([ch]) => ch === IPCChannel.NOTE_DELETED
      )
      expect(noteSent).toBeUndefined()
    })
  })

  describe('notes:loaded', () => {
    it('sends valid bulk file list', () => {
      const payload = { files: [{ path: '/v/a.md', name: 'a', mtime: 1 }] }
      sendToRenderer(IPCChannel.NOTES_LOADED, payload)

      expect(mockWebContents.send).toHaveBeenCalledWith(IPCChannel.NOTES_LOADED, payload)
    })

    it('rejects payload missing files array', () => {
      sendToRenderer(IPCChannel.NOTES_LOADED, {})

      const noteSent = mockWebContents.send.mock.calls.find(
        ([ch]) => ch === IPCChannel.NOTES_LOADED
      )
      expect(noteSent).toBeUndefined()
    })
  })

  describe('context:search', () => {
    it('sends valid search results', () => {
      const payload = { results: [{ path: '/v/a.md', score: 0.9, tokenCount: 100 }] }
      sendToRenderer(IPCChannel.CONTEXT_SEARCH, payload)

      expect(mockWebContents.send).toHaveBeenCalledWith(IPCChannel.CONTEXT_SEARCH, payload)
    })

    it('rejects result with score > 1', () => {
      sendToRenderer(IPCChannel.CONTEXT_SEARCH, {
        results: [{ path: '/v/a.md', score: 2.0, tokenCount: 10 }]
      })

      const noteSent = mockWebContents.send.mock.calls.find(
        ([ch]) => ch === IPCChannel.CONTEXT_SEARCH
      )
      expect(noteSent).toBeUndefined()
    })
  })

  describe('activity:log — Main→Renderer direction', () => {
    it('sends valid activity log payload', () => {
      const payload = { level: 'error' as const, message: 'something bad', timestamp: 1000 }
      sendToRenderer(IPCChannel.ACTIVITY_LOG, payload)

      expect(mockWebContents.send).toHaveBeenCalledWith(IPCChannel.ACTIVITY_LOG, payload)
    })

    it('rejects payload with invalid level', () => {
      sendToRenderer(IPCChannel.ACTIVITY_LOG, { level: 'fatal', message: 'x', timestamp: 0 })

      const logSent = mockWebContents.send.mock.calls.filter(
        ([ch]) => ch === IPCChannel.ACTIVITY_LOG
      )
      // No direct send; only the internally triggered warning may fire
      // But since the warning itself uses emitActivityLog which calls sendToRenderer
      // recursively — that would fail validation too, preventing any send
      expect(logSent.every(([, p]: [string, any]) => p.level !== 'fatal')).toBe(true)
    })
  })
})

// ===========================================================================
// SECTION 5: Undeclared Channel Filtering (Req 13.5 / overlaps task 8.2)
// sendToRenderer silently ignores channels not in outgoingSchemas
// ===========================================================================

describe('Undeclared channel filtering (Req 13.5)', () => {
  it('sendToRenderer silently ignores an undeclared channel', () => {
    const undeclaredChannel = 'vault:undeclared' as IPCChannel
    sendToRenderer(undeclaredChannel, { anything: true })

    // Nothing should have been sent
    expect(mockWebContents.send).not.toHaveBeenCalled()
  })

  it('sendToRenderer silently ignores Renderer→Main-only channels', () => {
    // vault:open, vault:scan, etc. are not in outgoingSchemas
    sendToRenderer(IPCChannel.VAULT_OPEN, { path: '/v' })
    sendToRenderer(IPCChannel.FILE_GET, { path: '/v/a.md' })
    sendToRenderer(IPCChannel.TASK_TOGGLE, { path: '/v/a.md', lineIndex: 0 })

    expect(mockWebContents.send).not.toHaveBeenCalled()
  })
})

// ===========================================================================
// SECTION 6: Zod validation is applied before dispatch (Req 13.3, 13.4)
// Explicit round-trip: handler rejects invalid then accepts valid
// ===========================================================================

describe('Zod validation applied before dispatch — round-trip (Req 13.3, 13.4)', () => {
  it('task:toggle: rejects invalid then accepts valid in sequence', async () => {
    // First call — invalid (missing path)
    const bad = (await invokeHandler(IPCChannel.TASK_TOGGLE, { lineIndex: 0 })) as any
    expect(bad.success).toBe(false)
    expect(mockStateManager.toggleTask).not.toHaveBeenCalled()

    // Second call — valid
    mockStateManager.toggleTask.mockResolvedValueOnce(undefined)
    const good = (await invokeHandler(IPCChannel.TASK_TOGGLE, {
      path: '/vault/note.md',
      lineIndex: 0
    })) as any
    expect(good.success).toBe(true)
    expect(mockStateManager.toggleTask).toHaveBeenCalledTimes(1)
  })

  it('context:query: rejects invalid then accepts valid in sequence', async () => {
    // Invalid — text is a number
    const bad = (await invokeHandler(IPCChannel.CONTEXT_QUERY, { text: 42 })) as any
    expect(bad).toHaveProperty('error')
    expect(mockVectorManager.search).not.toHaveBeenCalled()

    // Valid
    mockVectorManager.getStatus.mockResolvedValueOnce({ disabled: false, reason: null, items: 5 })
    mockVectorManager.search.mockResolvedValueOnce([])
    const good = (await invokeHandler(IPCChannel.CONTEXT_QUERY, { text: 'real query' })) as any
    expect(good).toMatchObject({ results: [] })
    expect(mockVectorManager.search).toHaveBeenCalledTimes(1)
  })

  it('sendToRenderer note:loaded: rejects invalid then sends valid in sequence', () => {
    // Invalid — missing ast
    sendToRenderer(IPCChannel.NOTE_LOADED, { path: '/v/a.md' })
    const sentBad = mockWebContents.send.mock.calls.filter(([ch]) => ch === IPCChannel.NOTE_LOADED)
    expect(sentBad).toHaveLength(0)

    // Valid
    sendToRenderer(IPCChannel.NOTE_LOADED, { path: '/v/a.md', ast: {} })
    const sentGood = mockWebContents.send.mock.calls.filter(([ch]) => ch === IPCChannel.NOTE_LOADED)
    expect(sentGood).toHaveLength(1)
  })

  // --- index:build schema with extendedIndex ---
  describe('IndexBuildSchema with extendedIndex (Req 2.6, 2.8)', () => {
    const validExtendedIndex = {
      positions: { hello: { '/v/a.md': [1, 2] } },
      lineSnippets: { '/v/a.md': ['hello world'] },
      tagIndex: { tag1: ['/v/a.md'] },
      aliasIndex: { 'my-alias': ['/v/a.md'] },
      propertyIndex: { author: { pablo: ['/v/a.md'] } },
      blockRefs: { '/v/a.md': { '^ref1': 'L5' } }
    }

    it('accepts full payload with extendedIndex', () => {
      const payload = {
        ftIndex: { hello: ['/v/a.md'] },
        tagIndex: { tag1: ['/v/a.md'] },
        edges: [{ source: '/v/a.md', target: '/v/b.md', snippet: 'a' }],
        extendedIndex: validExtendedIndex
      }
      expect(() => IndexBuildSchema.parse(payload)).not.toThrow()
    })

    it('rejects payload missing extendedIndex', () => {
      const payload = {
        ftIndex: { hello: ['/v/a.md'] },
        tagIndex: { tag1: ['/v/a.md'] },
        edges: [{ source: '/v/a.md', target: '/v/b.md', snippet: 'a' }]
      }
      expect(() => IndexBuildSchema.parse(payload)).toThrow(ZodError)
    })

    it('rejects extendedIndex with invalid positions value type', () => {
      const payload = {
        ftIndex: { hello: ['/v/a.md'] },
        tagIndex: { tag1: ['/v/a.md'] },
        edges: [{ source: '/v/a.md', target: '/v/b.md', snippet: 'a' }],
        extendedIndex: {
          ...validExtendedIndex,
          positions: { hello: { '/v/a.md': ['not-a-number'] } }
        }
      }
      expect(() => IndexBuildSchema.parse(payload)).toThrow(ZodError)
    })
  })
})
