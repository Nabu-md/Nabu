import { render } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createEditor,
  getSingleEditorViewTestState,
  makeEntry,
} from './SingleEditorView.testUtils'
import { SingleEditorView } from './SingleEditorView'

const state = getSingleEditorViewTestState()

describe('SingleEditorView wikilink creation suggestions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    state.capturedSuggestionProps = {}
    state.wikilinkCandidates = []
  })

  it('appends creation after partial matches but omits it for an exact match', async () => {
    const editor = createEditor()
    const onNavigateWikilink = vi.fn()
    const exactEntry = makeEntry({
      path: '/vault/project/alpha.md',
      filename: 'alpha.md',
      title: 'Alpha',
    })
    state.wikilinkCandidates = [{
      title: 'Alpha Roadmap',
      aliases: [],
      group: 'Note',
      entryTitle: 'Alpha Roadmap',
      path: '/vault/project/alpha-roadmap.md',
      onItemClick: vi.fn(),
    }]

    render(
      <SingleEditorView
        editor={editor as never}
        entries={[exactEntry]}
        onNavigateWikilink={onNavigateWikilink}
        sourceEntry={makeEntry({ path: '/vault/project/source.md', title: 'Source' })}
        vaultPath="/vault"
      />,
    )

    const getItems = state.capturedSuggestionProps['[['].getItems as (
      query: string
    ) => Promise<Array<{ title: string; onItemClick: () => void }>>
    const partialItems = await getItems('alpha plan')

    expect(partialItems.map((item) => item.title)).toEqual([
      'Alpha Roadmap',
      'Create a new note called “alpha plan”',
    ])

    partialItems.at(-1)?.onItemClick()

    expect(editor.insertInlineContent).toHaveBeenCalledWith(
      [{ type: 'wikilink', props: { target: 'alpha plan' } }, ' '],
      { updateSelection: true },
    )
    expect(onNavigateWikilink).toHaveBeenCalledWith('alpha plan')
    expect(await getItems('Alpha')).toHaveLength(1)
  })

  it('shows existing notes for empty and one-character bracket queries', async () => {
    const exactEntry = makeEntry({
      path: String.raw`C:\Users\alex\Documents\Nabu\project\alpha.md`,
      filename: 'alpha.md',
      title: 'Alpha',
    })

    render(
      <SingleEditorView
        editor={createEditor() as never}
        entries={[exactEntry]}
        onNavigateWikilink={vi.fn()}
        sourceEntry={makeEntry({ path: String.raw`C:\Users\alex\Documents\Nabu\source.md`, title: 'Source' })}
        vaultPath={String.raw`C:\Users\alex\Documents\Nabu`}
      />,
    )

    const getItems = state.capturedSuggestionProps['[['].getItems as (
      query: string
    ) => Promise<Array<{ title: string }>>

    expect((await getItems('')).map((item) => item.title)).toEqual(['Alpha'])
    expect((await getItems('A')).at(0)?.title).toBe('Alpha')
  })
})
