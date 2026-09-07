import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const editorThemeCss = readFileSync('src/components/EditorTheme.css', 'utf8')
const editorCss = readFileSync('src/components/Editor.css', 'utf8')

describe('rich editor scroll-beyond-last-line space', () => {
  it('reserves half of the viewport after the BlockNote document', () => {
    expect(editorThemeCss).toMatch(
      /\.editor__blocknote-container \.bn-editor\s*\{[^}]*padding-bottom:\s*max\(var\(--editor-padding-vertical\),\s*50vh\)/s,
    )
  })

  it('removes the visual-only space from note PDF exports', () => {
    expect(editorCss).toMatch(
      /body\.nabu-note-pdf-exporting \.bn-editor\s*\{[^}]*padding:\s*0 !important/s,
    )
  })
})
