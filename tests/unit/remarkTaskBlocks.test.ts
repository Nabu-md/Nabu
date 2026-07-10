/**
 * Unit tests for remarkTaskBlocks plugin
 * Validates Requirement 2.5
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'
import { remarkTaskBlocks } from '@main/plugins/remarkTaskBlocks'

describe('remarkTaskBlocks', () => {
  const createProcessor = () => {
    return unified().use(remarkParse).use(remarkGfm).use(remarkTaskBlocks)
  }

  it('should transform unchecked task list item into taskItem node', () => {
    const markdown = `- [ ] Unchecked task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    expect(taskList.type).toBe('taskList')
    expect(taskList.items).toBeDefined()
    expect(taskList.items.length).toBe(1)

    const taskItem = taskList.items[0]
    expect(taskItem.type).toBe('taskItem')
    expect(taskItem.checked).toBe(false)
    expect(taskItem).toHaveProperty('lineIndex')
  })

  it('should transform checked task list item into taskItem node', () => {
    const markdown = `- [x] Checked task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    expect(taskList.type).toBe('taskList')

    const taskItem = taskList.items[0]
    expect(taskItem.type).toBe('taskItem')
    expect(taskItem.checked).toBe(true)
  })

  it('should include 0-based lineIndex property', () => {
    const markdown = `- [ ] First task
- [x] Second task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    expect(taskList.items.length).toBe(2)

    const firstTask = taskList.items[0]
    const secondTask = taskList.items[1]

    expect(typeof firstTask.lineIndex).toBe('number')
    expect(typeof secondTask.lineIndex).toBe('number')
    expect(firstTask.lineIndex).toBeGreaterThanOrEqual(0)
    expect(secondTask.lineIndex).toBeGreaterThanOrEqual(0)
  })

  it('should extract children as PhrasingContent', () => {
    const markdown = `- [ ] Task with **bold** and *italic*`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    const taskItem = taskList.items[0]

    expect(taskItem.children).toBeDefined()
    expect(Array.isArray(taskItem.children)).toBe(true)
    expect(taskItem.children.length).toBeGreaterThan(0)

    // Check for emphasis and strong nodes
    const hasFormatting = taskItem.children.some(
      (child: any) => child.type === 'strong' || child.type === 'emphasis'
    )
    expect(hasFormatting).toBe(true)
  })

  it('should handle multiple task items in same list', () => {
    const markdown = `- [ ] First task
- [x] Second task
- [ ] Third task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    expect(taskList.items.length).toBe(3)

    expect(taskList.items[0].checked).toBe(false)
    expect(taskList.items[1].checked).toBe(true)
    expect(taskList.items[2].checked).toBe(false)
  })

  it('should not transform regular list items', () => {
    const markdown = `- Regular item
- Another item`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const firstNode = result.children[0]
    expect(firstNode.type).toBe('list')
    expect(firstNode.type).not.toBe('taskList')
  })

  it('should handle mixed list with both task and regular items', () => {
    const markdown = `- [ ] Task item
- Regular item
- [x] Another task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    // Since the list contains task items, it should be transformed
    expect(taskList.type).toBe('taskList')

    // Only task items should be extracted
    expect(taskList.items.length).toBe(2)
    expect(taskList.items[0].checked).toBe(false)
    expect(taskList.items[1].checked).toBe(true)
  })

  it('should handle task with inline code', () => {
    const markdown = `- [ ] Run \`npm install\``

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    const taskItem = taskList.items[0]

    const hasInlineCode = taskItem.children.some((child: any) => child.type === 'inlineCode')
    expect(hasInlineCode).toBe(true)
  })

  it('should handle task with link', () => {
    const markdown = `- [ ] Check [documentation](https://example.com)`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    const taskItem = taskList.items[0]

    const hasLink = taskItem.children.some((child: any) => child.type === 'link')
    expect(hasLink).toBe(true)
  })

  it('should handle single task item with minimal text', () => {
    const markdown = `- [ ] Task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    expect(taskList.items.length).toBe(1)

    const taskItem = taskList.items[0]
    expect(taskItem.checked).toBe(false)
  })

  it('should handle nested lists with tasks', () => {
    const markdown = `- [ ] Parent task
  - [ ] Child task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    expect(taskList.type).toBe('taskList')
    // Should extract parent task items
    expect(taskList.items.length).toBeGreaterThan(0)
  })

  it('should handle uppercase X in checkbox', () => {
    const markdown = `- [X] Task with uppercase X`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    const taskItem = taskList.items[0]

    expect(taskItem.checked).toBe(true)
  })

  it('should handle task with strikethrough', () => {
    const markdown = `- [x] ~~Completed~~ task`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    const taskItem = taskList.items[0]

    const hasDelete = taskItem.children.some((child: any) => child.type === 'delete')
    expect(hasDelete).toBe(true)
  })

  it('should preserve task item children order', () => {
    const markdown = `- [ ] First **bold** then *italic*`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const taskList = result.children[0] as any
    const taskItem = taskList.items[0]

    // Verify children exist and maintain order
    expect(taskItem.children.length).toBeGreaterThan(0)

    // Find strong and emphasis nodes
    const strongIndex = taskItem.children.findIndex((child: any) => child.type === 'strong')
    const emphasisIndex = taskItem.children.findIndex((child: any) => child.type === 'emphasis')

    if (strongIndex !== -1 && emphasisIndex !== -1) {
      expect(strongIndex).toBeLessThan(emphasisIndex)
    }
  })
})
