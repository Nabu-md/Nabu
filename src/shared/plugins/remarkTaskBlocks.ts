/**
 * remarkTaskBlocks.ts
 *
 * Remark plugin that transforms GFM task list items into taskItem nodes within taskList parent.
 * Validates Requirement 2.5: Transform GFM task list items (- [ ] / - [x]) into taskItem nodes
 * with checked boolean and 0-based lineIndex properties.
 */

import { visit } from 'unist-util-visit'
import type { Plugin } from 'unified'
import type { Root, List, ListItem, PhrasingContent, Node } from 'mdast'

interface TaskItem extends Node {
  type: 'taskItem'
  checked: boolean
  lineIndex: number
  children: PhrasingContent[]
}

interface TaskList extends Node {
  type: 'taskList'
  items: TaskItem[]
}

/**
 * Checks if a list item is a GFM task list item
 */
function isTaskListItem(node: ListItem): boolean {
  return typeof node.checked === 'boolean'
}

/**
 * Remark plugin that transforms GFM task lists into taskList nodes
 */
export const remarkTaskBlocks: Plugin<[], Root> = function () {
  return (tree: Root) => {
    const nodesToProcess: Array<{
      list: List
      listIndex: number
      parent: Root | Node
    }> = []

    // First pass: identify all lists containing task items
    visit(tree, 'list', (node: List, index, parent) => {
      if (index === undefined || !parent) {
        return
      }

      // Check if any child is a task list item
      const hasTaskItems = node.children.some((child) => isTaskListItem(child))

      if (hasTaskItems) {
        nodesToProcess.push({
          list: node,
          listIndex: index,
          parent
        })
      }
    })

    // Second pass: transform in reverse order to maintain correct indices
    for (let i = nodesToProcess.length - 1; i >= 0; i--) {
      const { list, listIndex, parent } = nodesToProcess[i]

      if (!('children' in parent)) {
        continue
      }

      const parentChildren = parent.children as Node[]

      // Convert list items to taskItem nodes
      const taskItems: TaskItem[] = list.children
        .filter((item) => isTaskListItem(item))
        .map((item, itemIndex) => {
          // Calculate 0-based line index
          // In a real implementation, this would come from the position information
          // For now, we use the item's position if available
          const lineIndex = item.position?.start.line ? item.position.start.line - 1 : itemIndex

          // Extract children (phrasing content) by flattening paragraphs
          const children: PhrasingContent[] = []
          for (const child of item.children) {
            if (child.type === 'paragraph') {
              // Flatten paragraph children into task item children
              children.push(...(child.children as PhrasingContent[]))
            }
            // Other block-level children (nested lists, etc.) are excluded from phrasing content
          }

          return {
            type: 'taskItem',
            checked: item.checked ?? false,
            lineIndex,
            children
          } as TaskItem
        })

      // Create the taskList node
      const taskList: TaskList = {
        type: 'taskList',
        items: taskItems
      }

      // Replace the list with the taskList
      parentChildren.splice(listIndex, 1, taskList)
    }
  }
}

export default remarkTaskBlocks
