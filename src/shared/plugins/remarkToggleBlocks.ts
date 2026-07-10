/**
 * remarkToggleBlocks.ts
 *
 * Remark plugin that transforms headings with [toggle] prefix into toggleBlock nodes.
 * Validates Requirement 2.4: Transform headings whose text content begins with
 * case-insensitive `[toggle]` prefix into ToggleBlock AST nodes.
 */

import { visit } from 'unist-util-visit'
import type { Plugin } from 'unified'
import type { Root, Heading, Node } from 'mdast'

interface ToggleBlock extends Node {
  type: 'toggleBlock'
  heading: Heading
  children: Node[]
}

/**
 * Checks if a heading starts with [toggle] prefix (case-insensitive)
 */
function hasTogglePrefix(heading: Heading): boolean {
  if (!heading.children || heading.children.length === 0) {
    return false
  }

  const firstChild = heading.children[0]
  if (firstChild.type !== 'text') {
    return false
  }

  const text = firstChild.value.trimStart()
  return /^\[toggle\]/i.test(text)
}

/**
 * Removes the [toggle] prefix from the heading text node
 */
function removeTogglePrefix(heading: Heading): void {
  if (!heading.children || heading.children.length === 0) {
    return
  }

  const firstChild = heading.children[0]
  if (firstChild.type === 'text') {
    // Remove [toggle] prefix and any trailing whitespace
    firstChild.value = firstChild.value.replace(/^\s*\[toggle\]\s*/i, '')
  }
}

/**
 * Remark plugin that transforms [toggle] headings into toggleBlock nodes
 */
export const remarkToggleBlocks: Plugin<[], Root> = function () {
  return (tree: Root) => {
    const nodesToProcess: Array<{
      heading: Heading
      headingIndex: number
      parent: Root | Node
      childrenIndices: number[]
    }> = []

    // First pass: identify all toggle headings and determine their children
    visit(tree, 'heading', (node: Heading, index, parent) => {
      if (index === undefined || !parent) {
        return
      }

      if (!hasTogglePrefix(node)) {
        return
      }

      if (!('children' in parent)) {
        return
      }

      const parentChildren = parent.children as Node[]
      const headingDepth = node.depth

      // Collect indices of immediate child content
      const childrenIndices: number[] = []
      let currentIndex = index + 1

      while (currentIndex < parentChildren.length) {
        const childNode = parentChildren[currentIndex]

        // Stop if we encounter another heading at same or higher level
        if (childNode.type === 'heading') {
          const nextHeading = childNode as Heading
          if (nextHeading.depth <= headingDepth) {
            break
          }
        }

        childrenIndices.push(currentIndex)
        currentIndex++
      }

      nodesToProcess.push({
        heading: node,
        headingIndex: index,
        parent,
        childrenIndices
      })
    })

    // Second pass: transform in reverse order to maintain correct indices
    for (let i = nodesToProcess.length - 1; i >= 0; i--) {
      const { heading, headingIndex, parent, childrenIndices } = nodesToProcess[i]

      if (!('children' in parent)) {
        continue
      }

      const parentChildren = parent.children as Node[]

      // Collect children nodes
      const children: Node[] = childrenIndices.map((idx) => parentChildren[idx])

      // Create the toggleBlock node
      const clonedHeading = { ...heading }
      removeTogglePrefix(clonedHeading)

      const toggleBlock: ToggleBlock = {
        type: 'toggleBlock',
        heading: clonedHeading,
        children
      }

      // Replace the heading and its children with the toggleBlock
      const nodesToRemove = 1 + children.length // heading + children
      parentChildren.splice(headingIndex, nodesToRemove, toggleBlock)
    }
  }
}

export default remarkToggleBlocks
