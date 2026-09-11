import { beforeEach, describe, expect, it, vi } from 'vitest'
import { describeFolderNote, isFolderNote } from './folderNotes'
import type { VaultEntry } from '../types'

function makeEntry(overrides: Partial<VaultEntry> = {}): VaultEntry {
  return {
    path: 'notes/note.md',
    filename: 'note.md',
    title: 'Note',
    isA: 'Note',
    aliases: [],
    belongsTo: [],
    relatedTo: [],
    status: null,
    archived: false,
    modifiedAt: null,
    createdAt: null,
    fileSize: 0,
    snippet: '',
    wordCount: 0,
    properties: {},
    relationships: {},
    icon: null,
    color: null,
    order: null,
    sidebarLabel: null,
    template: null,
    sort: null,
    view: null,
    visible: true,
    organized: false,
    favorite: false,
    favoriteIndex: null,
    listPropertiesDisplay: [],
    outgoingLinks: [],
    hasH1: false,
    fileKind: 'markdown',
    ...overrides,
  }
}

describe('folderNotes', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('isFolderNote', () => {
    it('is true for a note whose type is Folder', () => {
      expect(isFolderNote(makeEntry({ isA: 'Folder' }))).toBe(true)
    })

    it('is case-insensitive on the type', () => {
      expect(isFolderNote(makeEntry({ isA: 'folder' }))).toBe(true)
    })

    it('is false for other types and untyped notes', () => {
      expect(isFolderNote(makeEntry({ isA: 'Note' }))).toBe(false)
      expect(isFolderNote(makeEntry({ isA: null }))).toBe(false)
    })
  })

  describe('describeFolderNote', () => {
    it('collects the linked child entries in link order, deduplicated', () => {
      const task1 = makeEntry({ path: 'My Project/Task 1.md', title: 'Task 1', isA: 'Note' })
      const task2 = makeEntry({ path: 'My Project/Task 2.md', title: 'Task 2', isA: 'Note' })
      const folderNote = makeEntry({
        isA: 'Folder',
        path: 'My Project/My Project.md',
        title: 'My Project',
        outgoingLinks: ['Task 1', 'Task 2', 'Task 1'],
      })
      const entries = [folderNote, task1, task2]

      expect(describeFolderNote({ entry: folderNote, entries })).toEqual({
        entry: folderNote,
        children: [task1, task2],
      })
    })

    it('resolves path-style wikilink targets', () => {
      const task = makeEntry({ path: 'My Project/task-1.md', title: 'Task 1' })
      const folderNote = makeEntry({
        isA: 'Folder',
        path: 'My Project/My Project.md',
        outgoingLinks: ['My Project/task-1'],
      })

      expect(describeFolderNote({ entry: folderNote, entries: [folderNote, task] }).children).toEqual([task])
    })

    it('excludes the folder note itself', () => {
      const folderNote = makeEntry({
        isA: 'Folder',
        path: 'My Project/My Project.md',
        outgoingLinks: ['My Project'],
      })
      expect(describeFolderNote({ entry: folderNote, entries: [folderNote] }).children).toEqual([])
    })

    it('is not a folder note when the type is not Folder', () => {
      const note = makeEntry({ isA: 'Note', outgoingLinks: ['Task 1'] })
      const task = makeEntry({ path: 'Task 1.md', title: 'Task 1' })
      expect(describeFolderNote({ entry: note, entries: [note, task] })).toBeNull()
    })
  })
})
