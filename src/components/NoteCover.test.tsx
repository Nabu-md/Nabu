import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { NoteCover } from './NoteCover'
import type { VaultEntry } from '../types'
import { isTauri } from '../mock-tauri'

vi.mock('../mock-tauri', () => ({
  isTauri: vi.fn(() => false),
}))

const isTauriMock = isTauri as unknown as { mockReturnValue: (value: boolean) => void }

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

describe('NoteCover', () => {
  beforeEach(() => {
    isTauriMock.mockReturnValue(false)
  })

  it('renders nothing without a cover source or picker', () => {
    const { container } = render(
      <NoteCover entry={makeEntry()} locale="en" vaultPath="/v" />,
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('renders the cover image from a cover_image property', () => {
    render(
      <NoteCover
        entry={makeEntry({ properties: { cover_image: 'https://example.com/cover.png' } })}
        locale="en"
        vaultPath="/v"
      />,
    )
    const image = screen.getByTestId('note-cover-image')
    expect(image).toHaveAttribute('src', 'https://example.com/cover.png')
  })

  it('offers an add-cover control when no cover is set', () => {
    render(<NoteCover entry={makeEntry()} locale="en" vaultPath="/v" onPickCover={() => {}} />)
    expect(screen.getByRole('button', { name: 'Add cover' })).toBeInTheDocument()
  })

  it('does not offer the add-cover control without a picker callback', () => {
    render(<NoteCover entry={makeEntry()} locale="en" vaultPath="/v" />)
    expect(screen.queryByRole('button', { name: 'Add cover' })).not.toBeInTheDocument()
  })

  it('calls onPickCover when the add control is activated', () => {
    const onPickCover = vi.fn()
    render(<NoteCover entry={makeEntry()} locale="en" vaultPath="/v" onPickCover={onPickCover} />)
    fireEvent.click(screen.getByRole('button', { name: 'Add cover' }))
    expect(onPickCover).toHaveBeenCalledOnce()
  })

  it('calls onRemoveCover when the remove control is activated', () => {
    const onRemoveCover = vi.fn()
    render(
      <NoteCover
        entry={makeEntry({ properties: { cover_image: 'https://example.com/cover.png' } })}
        locale="en"
        vaultPath="/v"
        onRemoveCover={onRemoveCover}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Remove cover' }))
    expect(onRemoveCover).toHaveBeenCalledOnce()
  })

  it('hides the remove control without a remove callback', () => {
    render(
      <NoteCover
        entry={makeEntry({ properties: { cover_image: 'https://example.com/cover.png' } })}
        locale="en"
        vaultPath="/v"
      />,
    )
    expect(screen.queryByRole('button', { name: 'Remove cover' })).not.toBeInTheDocument()
  })

  it('renders nothing for an invalid cover value', () => {
    const { container } = render(
      <NoteCover entry={makeEntry({ properties: { cover_image: 'not-a-cover' } })} locale="en" vaultPath="/v" />,
    )
    expect(container).toBeEmptyDOMElement()
  })
})
