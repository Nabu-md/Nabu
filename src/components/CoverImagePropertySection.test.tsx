import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { CoverImagePropertySection } from './CoverImagePropertySection'
import type { VaultEntry } from '../types'

vi.mock('../mock-tauri', () => ({
  isTauri: vi.fn(() => false),
}))

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
    properties: {},
    hasH1: false,
    fileKind: 'markdown',
    ...overrides,
  }
}

describe('CoverImagePropertySection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the label row and an add control when no cover is set', () => {
    render(<CoverImagePropertySection entry={makeEntry()} locale="en" onPickCover={() => {}} />)
    expect(screen.getByText('Cover image')).toBeInTheDocument()
    expect(screen.getByTestId('cover-property-add')).toBeInTheDocument()
  })

  it('renders thumbnail with change and remove controls when a cover is set', () => {
    render(
      <CoverImagePropertySection
        entry={makeEntry({ properties: { cover_image: 'attachments/photo.png' } })}
        locale="en"
        vaultPath="/v"
        onPickCover={() => {}}
        onRemoveCover={() => {}}
      />,
    )
    expect(screen.getByTestId('cover-property-thumbnail')).toHaveAttribute('src', 'attachments/photo.png')
    expect(screen.getByTestId('cover-property-change')).toBeInTheDocument()
    expect(screen.getByTestId('cover-property-remove')).toBeInTheDocument()
  })

  it('invokes onPickCover from the add control', () => {
    const onPickCover = vi.fn()
    render(<CoverImagePropertySection entry={makeEntry()} locale="en" onPickCover={onPickCover} />)
    fireEvent.click(screen.getByTestId('cover-property-add'))
    expect(onPickCover).toHaveBeenCalledOnce()
  })

  it('invokes onRemoveCover from the remove control', () => {
    const onRemoveCover = vi.fn()
    render(
      <CoverImagePropertySection
        entry={makeEntry({ properties: { cover_image: 'attachments/photo.png' } })}
        locale="en"
        onRemoveCover={onRemoveCover}
      />,
    )
    fireEvent.click(screen.getByTestId('cover-property-remove'))
    expect(onRemoveCover).toHaveBeenCalledOnce()
  })
})
