import type {
  BlockNoteEditor,
  BlockSchema,
  InlineContentSchema,
  StyleSchema,
} from '@blocknote/core'

export type NabuBlockNoteEditor = BlockNoteEditor<BlockSchema, InlineContentSchema, StyleSchema>
export type DropPlacement = 'before' | 'after'

export const BLOCK_CONTAINER_SELECTOR = '[data-node-type="blockContainer"][data-id]'
export const BLOCK_OUTER_SELECTOR = '[data-node-type="blockOuter"][data-id], .bn-block-outer[data-id]'

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}

export function editorBlockElement(editor: NabuBlockNoteEditor): HTMLElement | null {
  const element = editor.domElement
  if (!(element instanceof HTMLElement)) return null
  return element.matches('.bn-editor')
    ? element
    : element.querySelector('.bn-editor')
}

export function blockElementFromPoint({
  editorElement,
  ownerDocument,
  x,
  y,
}: {
  editorElement: HTMLElement
  ownerDocument: Document
  x: number
  y: number
}): HTMLElement | null {
  if (typeof ownerDocument.elementsFromPoint !== 'function') return null

  const editorRect = editorElement.getBoundingClientRect()
  if (editorRect.width <= 0 || editorRect.height <= 0) return null

  const hitX = clamp(x, editorRect.left + 10, editorRect.right - 10)
  const hitY = clamp(y, editorRect.top + 1, editorRect.bottom - 1)

  for (const element of ownerDocument.elementsFromPoint(hitX, hitY)) {
    if (!editorElement.contains(element)) continue

    const blockElement = element.closest(BLOCK_CONTAINER_SELECTOR)
    if (blockElement instanceof HTMLElement && editorElement.contains(blockElement)) {
      return blockElement
    }
  }

  return null
}

export function dropPlacementForPoint(blockElement: HTMLElement, y: number): DropPlacement {
  const rect = blockElement.getBoundingClientRect()
  return y < rect.top + rect.height / 2 ? 'before' : 'after'
}

export function blockIdFromElement(blockElement: HTMLElement): string | null {
  return blockElement.dataset.id ?? null
}

export function blockElementById(editorElement: HTMLElement, blockId: string): HTMLElement | null {
  for (const element of editorElement.querySelectorAll(BLOCK_CONTAINER_SELECTOR)) {
    if (element instanceof HTMLElement && element.dataset.id === blockId) return element
  }

  return null
}

export function renderedSectionBlockElements(editorElement: HTMLElement): HTMLElement[] {
  const outerBlocks = Array.from(editorElement.querySelectorAll(BLOCK_OUTER_SELECTOR))
    .filter((element): element is HTMLElement => element instanceof HTMLElement)
  if (outerBlocks.length > 0) return outerBlocks

  return Array.from(editorElement.querySelectorAll(BLOCK_CONTAINER_SELECTOR))
    .filter((element): element is HTMLElement => element instanceof HTMLElement)
}
