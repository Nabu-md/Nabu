import { describe, expect, it } from 'vitest'
import {
  NABU_BLOCK_CLIPBOARD_MIME,
  blocksWithoutIds,
  parseClipboardBlocks,
  writeSelectedBlocksToClipboard,
} from './richEditorBlockSelectionClipboard'
import type { ClipboardDataLike, RichEditorBlockSelectionEditor } from './richEditorBlockSelectionTypes'

class TestClipboardData implements ClipboardDataLike {
  private readonly data = new Map<string, string>()

  clearData() {
    this.data.clear()
  }

  getData(type: string) {
    return this.data.get(type) ?? ''
  }

  setData(type: string, value: string) {
    this.data.set(type, value)
  }
}

function parserEditor(): RichEditorBlockSelectionEditor {
  return {
    tryParseHTMLToBlocks: () => [{ id: 'html', type: 'paragraph' }],
    tryParseMarkdownToBlocks: () => [{ id: 'markdown', type: 'paragraph' }],
  }
}

function clipboardWithBlockNoteHTML(nabuData: string): TestClipboardData {
  const clipboardData = new TestClipboardData()
  clipboardData.setData(NABU_BLOCK_CLIPBOARD_MIME, nabuData)
  clipboardData.setData('blocknote/html', '<p>HTML</p>')
  return clipboardData
}

describe('rich editor block-selection clipboard helpers', () => {
  it('writes Nabu JSON, rich HTML, external HTML, and markdown formats', () => {
    const clipboardData = new TestClipboardData()
    const editor: RichEditorBlockSelectionEditor = {
      document: [
        { id: 'one', content: 'One', type: 'paragraph' },
        { id: 'two', content: 'Two', type: 'paragraph' },
      ],
      blocksToFullHTML: () => '<div data-content-type="paragraph">Two</div>',
      blocksToHTMLLossy: () => '<p>Two</p>',
      blocksToMarkdownLossy: () => 'Two',
    }

    expect(writeSelectedBlocksToClipboard(editor, clipboardData, ['two'])).toBe(true)
    expect(clipboardData.getData(NABU_BLOCK_CLIPBOARD_MIME)).toContain('"id":"two"')
    expect(clipboardData.getData('blocknote/html')).toContain('data-content-type')
    expect(clipboardData.getData('text/html')).toBe('<p>Two</p>')
    expect(clipboardData.getData('text/plain')).toBe('Two')
  })

  it('parses Nabu blocks before falling back to HTML or markdown', () => {
    const clipboardData = clipboardWithBlockNoteHTML(JSON.stringify([{ id: 'nabu', type: 'paragraph' }]))

    expect(parseClipboardBlocks(parserEditor(), clipboardData)).toEqual([{ id: 'nabu', type: 'paragraph' }])
  })

  it('falls back from invalid Nabu data to BlockNote HTML', () => {
    const clipboardData = clipboardWithBlockNoteHTML('{')

    expect(parseClipboardBlocks(parserEditor(), clipboardData)).toEqual([{ id: 'html', type: 'paragraph' }])
  })

  it('strips ids from pasted blocks recursively', () => {
    expect(blocksWithoutIds([
      {
        id: 'parent',
        type: 'bulletListItem',
        children: [{ id: 'child', type: 'bulletListItem' }],
      },
    ])).toEqual([
      {
        type: 'bulletListItem',
        children: [{ type: 'bulletListItem' }],
      },
    ])
  })
})
