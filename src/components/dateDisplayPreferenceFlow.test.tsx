import type { ReactElement } from 'react'
import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AppPreferencesProvider } from '../hooks/useAppPreferences'
import { NoteItem } from './NoteItem'
import { SmartPropertyValueCell } from './PropertyValueCells'
import {
  makeEntry,
  makeTypeDefinition,
} from '../test-utils/noteListTestUtils'

function renderWithPreferences(ui: ReactElement) {
  return render(
    <TooltipProvider>
      <AppPreferencesProvider dateDisplayFormat="european">
        {ui}
      </AppPreferencesProvider>
    </TooltipProvider>,
  )
}

describe('date display preference flow', () => {
  it('formats note date rows from the shared preference provider', () => {
    const entry = makeEntry({
      path: '/vault/book.md',
      filename: 'book.md',
      title: 'Book Note',
      isA: 'Book',
      modifiedAt: 1746921600,
      properties: { Due: '2026-05-11' },
    })

    renderWithPreferences(<NoteItem entry={entry} isSelected={false} typeEntryMap={{}} allEntries={[entry]} displayPropsOverride={[]} onClickNote={() => {}} />)

    expect(screen.getByTestId('note-title-row')).toHaveTextContent('Book Note')
  })

  it('keeps date editor input ISO while display text follows the shared preference', () => {
    renderWithPreferences(
      <SmartPropertyValueCell
        propKey="Due"
        value="2026-04-20"
        displayMode="date"
        isEditing={true}
        vaultStatuses={[]}
        vaultTags={[]}
        onStartEdit={vi.fn()}
        onSave={vi.fn()}
        onSaveList={vi.fn()}
      />,
    )

    expect(screen.getByTestId('date-display')).toHaveTextContent('20/4/2026')
    expect(screen.getByTestId('date-picker-input')).toHaveValue('2026-04-20')
  })
})
