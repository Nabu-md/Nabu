import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '../mock-tauri'
import type { VaultEntry } from '../types'
import { RAPID_CREATE_NOTE_SETTLE_MS, useNoteCreation } from './useNoteCreation'
import type { NoteCreationConfig } from './useNoteCreation'
import { requestCreateNoteInFolder } from './noteCreationRequests'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('../mock-tauri', () => ({
  isTauri: vi.fn(() => false),
  addMockEntry: vi.fn(),
  updateMockContent: vi.fn(),
  trackMockChange: vi.fn(),
  mockInvoke: vi.fn().mockResolvedValue(''),
}))

const makeEntry = (overrides: Partial<VaultEntry> = {}): VaultEntry => ({
  path: '/vault/test.md', filename: 'test.md', title: 'Test Note', isA: 'Note',
  aliases: [], belongsTo: [], relatedTo: [], status: 'Active', archived: false,
  modifiedAt: 1700000000, createdAt: 1700000000, fileSize: 100, snippet: '',
  wordCount: 0, relationships: {}, icon: null, color: null, order: null,
  outgoingLinks: [], template: null, sort: null, sidebarLabel: null,
  view: null, visible: null, properties: {}, organized: false, favorite: false,
  favoriteIndex: null, listPropertiesDisplay: [], hasH1: false,
  ...overrides,
})

describe('useNoteCreation hook', () => {
  const addEntry = vi.fn()
  const removeEntry = vi.fn()
  const setToastMessage = vi.fn()
  const openTabWithContent = vi.fn()
  const makeConfig = (entries: VaultEntry[] = []): NoteCreationConfig => ({
    addEntry, removeEntry, entries, setToastMessage, vaultPath: '/test/vault',
  })

  const tabDeps = { openTabWithContent }
  const flushImmediateCreate = async () => {
    await Promise.resolve()
    await Promise.resolve()
  }

  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(isTauri).mockReturnValue(false)
    vi.useRealTimers()
  })

  it('handleCreateNote creates entry and opens tab', () => {
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    act(() => { result.current.handleCreateNote('Test Note', 'Note') })
    expect(addEntry).toHaveBeenCalledTimes(1)
    expect(openTabWithContent).toHaveBeenCalledTimes(1)
    const [createdEntry] = addEntry.mock.calls[0]
    expect(createdEntry.title).toBe('Test Note')
    expect(createdEntry.isA).toBe('Note')
    expect(createdEntry.status).toBeNull()
    expect(openTabWithContent.mock.calls[0][1]).toBe('---\ntitle: Test Note\ntype: Note\n---\n')
  })

  it('handleCreateNoteImmediate generates timestamp-based title', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    await act(async () => {
      result.current.handleCreateNoteImmediate()
      await flushImmediateCreate()
    })
    expect(addEntry).toHaveBeenCalledTimes(1)
    expect(addEntry.mock.calls[0][0].title).toBe('Untitled Note 1700000000')
    expect(addEntry.mock.calls[0][0].filename).toBe('untitled-note-1700000000.md')
    expect(addEntry.mock.calls[0][0].status).toBeNull()
    expect(openTabWithContent.mock.calls[0][1]).toBe('---\ntype: Note\n---\n\n# \n\n')
    vi.restoreAllMocks()
  })

  it('handleCreateNoteImmediate can target a nested folder in a mounted vault', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke).mockResolvedValueOnce(undefined)
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate(undefined, {
        creationPath: 'folder_header',
        folderPath: 'Projects/2026 Planning',
        vaultPath: '/Users/demo/Team',
      })
      await flushImmediateCreate()
    })

    const createdPath = '/Users/demo/Team/Projects/2026 Planning/untitled-note-1700000000.md'
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('create_note_content', {
      path: createdPath,
      content: '---\ntype: Note\n---\n\n# \n\n',
      vaultPath: '/Users/demo/Team',
    })
    expect(addEntry).toHaveBeenCalledWith(expect.objectContaining({
      path: createdPath,
      workspace: expect.objectContaining({ path: '/Users/demo/Team' }),
    }))
    vi.restoreAllMocks()
  })

  it('handles folder create requests from the folder tree event bridge', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke).mockResolvedValueOnce(undefined)
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      requestCreateNoteInFolder('Projects/2026 Planning', '/Users/demo/Team')
      await flushImmediateCreate()
    })

    const createdPath = '/Users/demo/Team/Projects/2026 Planning/untitled-note-1700000000.md'
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('create_note_content', {
      path: createdPath,
      content: '---\ntype: Note\n---\n\n# \n\n',
      vaultPath: '/Users/demo/Team',
    })
    expect(openTabWithContent).toHaveBeenCalledWith(
      expect.objectContaining({ path: createdPath }),
      '---\ntype: Note\n---\n\n# \n\n',
    )
    expect(addEntry).toHaveBeenCalledWith(expect.objectContaining({ path: createdPath }))
    vi.restoreAllMocks()
  })

  it('handleCreateNoteImmediate generates unique names on rapid calls via timestamp', async () => {
    vi.useFakeTimers()
    let ts = 1700000000000
    vi.spyOn(Date, 'now').mockImplementation(() => { ts += 1000; return ts })
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    await act(async () => {
      result.current.handleCreateNoteImmediate()
      result.current.handleCreateNoteImmediate()
      result.current.handleCreateNoteImmediate()
      await flushImmediateCreate()
    })
    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS)
      await flushImmediateCreate()
    })
    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS)
      await flushImmediateCreate()
    })
    const filenames = addEntry.mock.calls.map(([e]: [VaultEntry]) => e.filename)
    // Each call consumes Date.now() multiple times (filename + buildNewEntry), so just verify uniqueness
    expect(new Set(filenames).size).toBe(3)
    for (const fn of filenames) {
      expect(fn).toMatch(/^untitled-note-\d+\.md$/)
    }
    vi.restoreAllMocks()
  })

  it('handleCreateNoteImmediate avoids filename collisions when called twice in the same second', async () => {
    vi.useFakeTimers()
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate()
      result.current.handleCreateNoteImmediate()
      await flushImmediateCreate()
    })
    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS)
      await flushImmediateCreate()
    })

    const filenames = addEntry.mock.calls.map(([entry]: [VaultEntry]) => entry.filename)
    expect(filenames).toEqual([
      'untitled-note-1700000000.md',
      'untitled-note-1700000000-2.md',
    ])

    vi.restoreAllMocks()
  })

  it('serializes rapid immediate-create bursts after the first note', async () => {
    vi.useFakeTimers()
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate()
      result.current.handleCreateNoteImmediate()
      result.current.handleCreateNoteImmediate()
      await flushImmediateCreate()
    })

    expect(addEntry).toHaveBeenCalledTimes(1)

    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS)
      await flushImmediateCreate()
    })
    expect(addEntry).toHaveBeenCalledTimes(2)

    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS)
      await flushImmediateCreate()
    })
    expect(addEntry).toHaveBeenCalledTimes(3)

    vi.restoreAllMocks()
  })

  it('waits for slow immediate note persistence before starting the queued create', async () => {
    vi.useFakeTimers()
    vi.mocked(isTauri).mockReturnValue(true)
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    let resolveFirstWrite: () => void
    const firstWrite = new Promise<void>((resolve) => {
      resolveFirstWrite = resolve
    })
    vi.mocked(invoke)
      .mockImplementationOnce(() => firstWrite)
      .mockResolvedValue(undefined)
    const createCalls = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === 'create_note_content')
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate()
      result.current.handleCreateNoteImmediate()
      await flushImmediateCreate()
    })

    expect(createCalls()).toHaveLength(1)

    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS * 3)
      await flushImmediateCreate()
    })

    expect(createCalls()).toHaveLength(1)

    await act(async () => {
      resolveFirstWrite()
      await flushImmediateCreate()
    })
    await act(async () => {
      vi.advanceTimersByTime(RAPID_CREATE_NOTE_SETTLE_MS)
      await flushImmediateCreate()
    })

    expect(createCalls()).toHaveLength(2)
    expect(addEntry).toHaveBeenCalledTimes(2)
    vi.restoreAllMocks()
  })

  it('handleCreateNoteImmediate accepts custom type', async () => {
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    await act(async () => {
      result.current.handleCreateNoteImmediate('Project')
      await flushImmediateCreate()
    })
    expect(addEntry.mock.calls[0][0].isA).toBe('Project')
    expect(addEntry.mock.calls[0][0].status).toBeNull()
    expect(openTabWithContent.mock.calls[0][1]).toBe('---\ntype: Project\n---\n\n# \n\n')
  })

  it('handleCreateNoteImmediate creates sheet-display notes without markdown body scaffolding', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate(undefined, { creationPath: 'cmd_sheet', format: 'sheet' })
      await flushImmediateCreate()
    })

    expect(addEntry.mock.calls[0][0]).toMatchObject({
      filename: 'untitled-sheet-1700000000.md',
      title: 'Untitled Sheet 1700000000',
      isA: 'Note',
    })
    expect(openTabWithContent.mock.calls[0][1]).toBe('---\ntype: Note\n_display: sheet\n---\n')
    vi.restoreAllMocks()
  })

  it('handleCreateNoteImmediate persists typed notes under Windows verbatim vault roots', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke).mockResolvedValueOnce(undefined)
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const windowsVaultPath = String.raw`\\?\C:\Users\alex\Documents\Nabu`
    const createdPath = String.raw`\\?\C:\Users\alex\Documents\Nabu/untitled-project-1700000000.md`
    const { result } = renderHook(() => useNoteCreation({
      ...makeConfig(),
      vaultPath: windowsVaultPath,
    }, tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate('Project')
      await flushImmediateCreate()
    })

    expect(vi.mocked(invoke)).toHaveBeenCalledWith('create_note_content', {
      path: createdPath,
      content: expect.stringContaining('type: Project'),
      vaultPath: windowsVaultPath,
    })
    expect(addEntry).toHaveBeenCalledWith(expect.objectContaining({
      path: createdPath,
      filename: 'untitled-project-1700000000.md',
      isA: 'Project',
    }))
    expect(setToastMessage).not.toHaveBeenCalled()
    vi.restoreAllMocks()
  })

  it('handleCreateNoteImmediate slugifies custom type names for filenames', async () => {
    vi.spyOn(Date, 'now').mockReturnValue(1700000000000)
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    await act(async () => {
      result.current.handleCreateNoteImmediate('Q&A / Ops')
      await flushImmediateCreate()
    })

    expect(addEntry.mock.calls[0][0].filename).toBe('untitled-q-a-ops-1700000000.md')
    vi.restoreAllMocks()
  })

  it('handleCreateType creates type entry', async () => {
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    let created = false
    await act(async () => {
      created = await result.current.handleCreateType('Recipe')
    })

    expect(created).toBe(true)
    expect(addEntry.mock.calls[0][0].isA).toBe('Type')
    expect(addEntry.mock.calls[0][0].title).toBe('Recipe')
  })

  it('handleCreateType persists type files under Windows verbatim vault roots', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error('not found'))
      .mockResolvedValueOnce(undefined)
    const onTypeStateChanged = vi.fn()
    const windowsVaultPath = String.raw`\\?\C:\Users\alex\Documents\Nabu`
    const createdPath = String.raw`\\?\C:\Users\alex\Documents\Nabu/recipe.md`
    const { result } = renderHook(() => useNoteCreation({
      ...makeConfig(),
      vaultPath: windowsVaultPath,
      onTypeStateChanged,
    }, tabDeps))

    let created = false
    await act(async () => {
      created = await result.current.handleCreateType('Recipe')
    })

    expect(created).toBe(true)
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_note_content', {
      path: createdPath,
      vaultPath: windowsVaultPath,
    })
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('create_note_content', {
      path: createdPath,
      content: '---\ntype: Type\n---\n\n# Recipe\n',
      vaultPath: windowsVaultPath,
    })
    expect(openTabWithContent).toHaveBeenCalledWith(expect.objectContaining({
      path: createdPath,
      filename: 'recipe.md',
      title: 'Recipe',
      isA: 'Type',
    }), expect.stringContaining('type: Type'))
    expect(onTypeStateChanged).toHaveBeenCalledOnce()
    expect(setToastMessage).not.toHaveBeenCalled()
  })

  it('handleCreateType blocks when the target type file already exists', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke).mockResolvedValueOnce('---\ntype: Note\n---\n# Existing Briefing\n')
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    let created = true
    await act(async () => {
      created = await result.current.handleCreateType('Briefing')
    })

    expect(created).toBe(false)
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_note_content', {
      path: '/test/vault/briefing.md',
      vaultPath: '/test/vault',
    })
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'create_note_content')).toBe(false)
    expect(addEntry).not.toHaveBeenCalled()
    expect(openTabWithContent).not.toHaveBeenCalled()
    expect(removeEntry).not.toHaveBeenCalled()
    expect(setToastMessage).toHaveBeenCalledWith('Cannot create type "Briefing" because briefing.md already exists')
  })

  it('handleCreateType blocks the built-in Note type when stale entries omit existing note.md', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke).mockResolvedValueOnce('---\ntype: Type\n---\n# Note\n')
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))

    let created = true
    await act(async () => {
      created = await result.current.handleCreateType('Note')
    })

    expect(created).toBe(false)
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_note_content', {
      path: '/test/vault/note.md',
      vaultPath: '/test/vault',
    })
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'create_note_content')).toBe(false)
    expect(addEntry).not.toHaveBeenCalled()
    expect(openTabWithContent).not.toHaveBeenCalled()
    expect(setToastMessage).toHaveBeenCalledWith('Cannot create type "Note" because note.md already exists')
  })

  it('handleCreateType creates the built-in Note type when notes.md is an existing ordinary note', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error('not found'))
      .mockResolvedValueOnce(undefined)
    const existingNotes = makeEntry({
      path: '/test/vault/notes.md',
      filename: 'notes.md',
      title: 'Meeting Notes',
      isA: 'Note',
    })
    const { result } = renderHook(() => useNoteCreation(makeConfig([existingNotes]), tabDeps))

    let created = false
    await act(async () => {
      created = await result.current.handleCreateType('Notes')
    })

    expect(created).toBe(true)
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_note_content', {
      path: '/test/vault/note.md',
      vaultPath: '/test/vault',
    })
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('create_note_content', {
      path: '/test/vault/note.md',
      content: '---\ntype: Type\n---\n\n# Note\n',
      vaultPath: '/test/vault',
    })
    expect(addEntry).toHaveBeenCalledWith(expect.objectContaining({
      path: '/test/vault/note.md',
      filename: 'note.md',
      title: 'Note',
      isA: 'Type',
    }))
    expect(openTabWithContent).toHaveBeenCalledWith(expect.objectContaining({
      path: '/test/vault/note.md',
      title: 'Note',
      isA: 'Type',
    }), expect.stringContaining('# Note'))
    expect(setToastMessage).not.toHaveBeenCalled()
  })

  it('handleCreateType blocks when a loaded non-Type entry collides with the target type path', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    const staleEntry = makeEntry({
      path: '/test/vault/pttep.md',
      filename: 'pttep.md',
      title: 'Stale cache entry',
      isA: 'Note',
    })
    const { result } = renderHook(() => useNoteCreation(makeConfig([staleEntry]), tabDeps))

    let created = true
    await act(async () => {
      created = await result.current.handleCreateType('PTTEP')
    })

    expect(created).toBe(false)
    expect(vi.mocked(invoke)).not.toHaveBeenCalled()
    expect(addEntry).not.toHaveBeenCalled()
    expect(openTabWithContent).not.toHaveBeenCalled()
    expect(setToastMessage).toHaveBeenCalledWith('Cannot create type "PTTEP" because pttep.md already exists')
  })

  it('handleCreateType writes new type entries to the vault root even when older type entries live in a folder', async () => {
    const existingType = makeEntry({
      path: '/test/vault/types/project.md',
      filename: 'project.md',
      title: 'Project',
      isA: 'Type',
    })
    const { result } = renderHook(() => useNoteCreation(makeConfig([existingType]), tabDeps))

    await act(async () => {
      await result.current.handleCreateType('Hotel')
    })

    expect(addEntry).toHaveBeenCalledWith(expect.objectContaining({
      path: '/test/vault/hotel.md',
      filename: 'hotel.md',
      title: 'Hotel',
      isA: 'Type',
    }))
    expect(setToastMessage).not.toHaveBeenCalled()
  })

  it('handleCreateType ignores an existing untitled draft when a unicode filename is unique', async () => {
    const existing = makeEntry({ path: '/test/vault/untitled.md', filename: 'untitled.md', title: 'Untitled', isA: 'Note' })
    const { result } = renderHook(() => useNoteCreation(makeConfig([existing]), tabDeps))

    let created = false
    await act(async () => {
      created = await result.current.handleCreateType('停智慧')
    })

    expect(created).toBe(true)
    expect(addEntry).toHaveBeenCalledWith(expect.objectContaining({
      path: '/test/vault/停智慧.md',
      filename: '停智慧.md',
      title: '停智慧',
      isA: 'Type',
    }))
    expect(setToastMessage).not.toHaveBeenCalled()
  })

  it('createTypeEntrySilent persists without opening tab', async () => {
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    const entry = await act(async () => result.current.createTypeEntrySilent('Recipe'))
    expect(addEntry).toHaveBeenCalledTimes(1)
    expect(openTabWithContent).not.toHaveBeenCalled()
    expect(entry.isA).toBe('Type')
  })

  it('createTypeEntrySilent reuses an existing slug-equivalent type', async () => {
    const existing = makeEntry({ path: '/test/vault/briefing.md', filename: 'briefing.md', title: 'Briefing', isA: 'Type' })
    const { result } = renderHook(() => useNoteCreation(makeConfig([existing]), tabDeps))

    const entry = await act(async () => result.current.createTypeEntrySilent('briefing'))

    expect(entry).toBe(existing)
    expect(addEntry).not.toHaveBeenCalled()
    expect(openTabWithContent).not.toHaveBeenCalled()
  })

  it('handleCreateNoteForRelationship blocks when the generated filename already exists', async () => {
    const existing = makeEntry({ path: '/test/vault/briefing.md', filename: 'briefing.md', title: 'Existing Briefing', isA: 'Note' })
    const { result } = renderHook(() => useNoteCreation(makeConfig([existing]), tabDeps))

    let created = true
    await act(async () => {
      created = await result.current.handleCreateNoteForRelationship('Briefing')
    })

    expect(created).toBe(false)
    expect(addEntry).not.toHaveBeenCalled()
    expect(openTabWithContent).not.toHaveBeenCalled()
    expect(setToastMessage).toHaveBeenCalledWith('Cannot create note "Briefing" because briefing.md already exists')
  })

  it('reverts optimistic creation when disk write fails (Tauri)', async () => {
    vi.mocked(isTauri).mockReturnValue(true)
    vi.mocked(invoke).mockRejectedValueOnce(new Error('disk full'))
    const { result } = renderHook(() => useNoteCreation(makeConfig(), tabDeps))
    await act(async () => {
      result.current.handleCreateNote('Failing Note', 'Note')
      await new Promise(r => setTimeout(r, 0))
    })
    expect(removeEntry).toHaveBeenCalled()
    expect(setToastMessage).toHaveBeenCalledWith('Failed to create note — disk write error')
  })

})
