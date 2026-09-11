import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { FolderNoteRow } from './FolderNoteRow'
import type { VaultEntry } from '../../types'

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

describe('FolderNoteRow', () => {
  it('renders the folder note title with an open toggle', () => {
    const entry = makeEntry({ isA: 'Folder', title: 'My Project' })
    render(<FolderNoteRow entry={entry} depth={0} childrenCount={2} onToggle={() => {}} />)
    expect(screen.getByTestId('folder-note-row:My Project')).toBeInTheDocument()
    expect(screen.getByText('My Project')).toBeInTheDocument()
  })

  it('exposes aria-expanded matching the expansion state', () => {
    const entry = makeEntry({ isA: 'Folder', title: 'My Project' })
    render(
      <FolderNoteRow entry={entry} depth={0} childrenCount={2} isExpanded={true} onToggle={() => {}} />,
    )
    expect(screen.getByTestId('folder-note-row:My Project')).toHaveAttribute('aria-expanded', 'true')
  })

  it('invokes onToggle when clicked', () => {
    const onToggle = vi.fn()
    const entry = makeEntry({ isA: 'Folder', title: 'My Project' })
    render(<FolderNoteRow entry={entry} depth={0} childrenCount={2} onToggle={onToggle} />)
    fireEvent.click(screen.getByTestId('folder-note-row:My Project'))
    expect(onToggle).toHaveBeenCalledOnce()
  })

  it('invokes onOpen on double click', () => {
    const onOpen = vi.fn()
    const entry = makeEntry({ isA: 'Folder', title: 'My Project' })
    render(
      <FolderNoteRow entry={entry} depth={0} childrenCount={0} onToggle={() => {}} onOpen={onOpen} />,
    )
    fireEvent.doubleClick(screen.getByTestId('folder-note-row:My Project'))
    expect(onOpen).toHaveBeenCalledWith(entry)
  })

  it('shows the child count when expanded children exist', () => {
    const entry = makeEntry({ isA: 'Folder', title: 'My Project' })
    render(<FolderNoteRow entry={entry} depth={0} childrenCount={3} onToggle={() => {}} />)
    expect(screen.getByText('3')).toBeInTheDocument()
  })
})
