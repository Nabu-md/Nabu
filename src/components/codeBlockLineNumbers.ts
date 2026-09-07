import type { Node as ProsemirrorNode } from '@tiptap/pm/model'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet, type EditorView } from '@tiptap/pm/view'

const CODE_BLOCK_TYPE = 'codeBlock'
const LINE_NUMBER_CLASS = 'editor__code-line-number'
const lineNumberPluginKey = new PluginKey<DecorationSet>('nabuCodeBlockLineNumbers')

function lineStartOffsets(source: string): number[] {
  const offsets = [0]
  for (let offset = 0; offset < source.length; offset += 1) {
    if (source.charCodeAt(offset) === 10) offsets.push(offset + 1)
  }
  return offsets
}

function createLineNumberMarker(view: EditorView, lineNumber: number): HTMLElement {
  const marker = view.dom.ownerDocument.createElement('span')
  marker.className = LINE_NUMBER_CLASS
  marker.dataset.codeLineNumber = String(lineNumber)
  marker.setAttribute('aria-hidden', 'true')
  marker.setAttribute('contenteditable', 'false')
  return marker
}

function lineNumberWidget(position: number, lineNumber: number): Decoration {
  return Decoration.widget(position, (view) => createLineNumberMarker(view, lineNumber), {
    ignoreSelection: true,
    key: `code-line-${position}-${lineNumber}`,
    side: -1,
  })
}

function buildLineNumberDecorations(doc: ProsemirrorNode): DecorationSet {
  const decorations: Decoration[] = []
  doc.descendants((node, position) => {
    if (node.type.name !== CODE_BLOCK_TYPE) return true

    lineStartOffsets(node.textContent).forEach((offset, index) => {
      decorations.push(lineNumberWidget(position + 1 + offset, index + 1))
    })
    return false
  })
  return DecorationSet.create(doc, decorations)
}

export function createCodeBlockLineNumberPlugin(): Plugin<DecorationSet> {
  return new Plugin<DecorationSet>({
    key: lineNumberPluginKey,
    props: {
      decorations: (state) => lineNumberPluginKey.getState(state) ?? DecorationSet.empty,
    },
    state: {
      init: (_, state) => buildLineNumberDecorations(state.doc),
      apply: (transaction, decorations) => (
        transaction.docChanged
          ? buildLineNumberDecorations(transaction.doc)
          : decorations.map(transaction.mapping, transaction.doc)
      ),
    },
  })
}
