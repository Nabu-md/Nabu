/**
 * Unit tests for remarkToggleBlocks plugin
 * Validates Requirement 2.4
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import { remarkToggleBlocks } from '@main/plugins/remarkToggleBlocks'
import type { Root, Heading } from 'mdast'

describe('remarkToggleBlocks', () => {
  const createProcessor = () => {
    return unified().use(remarkParse).use(remarkToggleBlocks)
  }

  it('should transform heading with [toggle] prefix into toggleBlock', () => {
    const markdown = `# [toggle] My Section

This is content under the toggle.`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0]
    expect(toggleBlock.type).toBe('toggleBlock')
    expect(toggleBlock).toHaveProperty('heading')
    expect(toggleBlock).toHaveProperty('children')
  })

  it('should be case-insensitive for [toggle] prefix', () => {
    const testCases = [
      '# [TOGGLE] Uppercase',
      '# [Toggle] Titlecase',
      '# [toggle] Lowercase',
      '# [ToGgLe] Mixed case'
    ]

    for (const markdown of testCases) {
      const result = createProcessor().parse(markdown)
      createProcessor().runSync(result)

      const toggleBlock = result.children[0]
      expect(toggleBlock.type).toBe('toggleBlock')
    }
  })

  it('should remove [toggle] prefix from heading text', () => {
    const markdown = `# [toggle] My Heading`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    const heading = toggleBlock.heading as Heading
    const firstChild = heading.children[0]

    expect(firstChild.type).toBe('text')
    expect((firstChild as any).value).toBe('My Heading')
    expect((firstChild as any).value).not.toContain('[toggle]')
  })

  it('should include immediate child content (one level deep)', () => {
    const markdown = `# [toggle] Section

Paragraph 1

Paragraph 2

## Subsection

Content in subsection`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    expect(toggleBlock.type).toBe('toggleBlock')
    expect(toggleBlock.children.length).toBeGreaterThan(0)

    // Should include paragraphs and subsection
    const childTypes = toggleBlock.children.map((child: any) => child.type)
    expect(childTypes).toContain('paragraph')
    expect(childTypes).toContain('heading')
  })

  it('should stop at same-level heading', () => {
    const markdown = `# [toggle] Section One

Content for section one

# Section Two

Content for section two`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    expect(toggleBlock.type).toBe('toggleBlock')

    // Should NOT include "Section Two" heading
    const hasSecondHeading = toggleBlock.children.some(
      (child: any) => child.type === 'heading' && child.depth === 1
    )
    expect(hasSecondHeading).toBe(false)

    // Second heading should be a sibling, not a child
    const secondChild = result.children[1]
    expect(secondChild.type).toBe('heading')
  })

  it('should stop at higher-level heading', () => {
    const markdown = `## [toggle] Subsection

Content

# Main Section

Other content`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    expect(toggleBlock.type).toBe('toggleBlock')

    // Should NOT include main section heading
    const hasMainHeading = toggleBlock.children.some(
      (child: any) => child.type === 'heading' && child.depth === 1
    )
    expect(hasMainHeading).toBe(false)
  })

  it('should not transform heading without [toggle] prefix', () => {
    const markdown = `# Normal Heading

Content`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const firstNode = result.children[0]
    expect(firstNode.type).toBe('heading')
    expect(firstNode.type).not.toBe('toggleBlock')
  })

  it('should not transform [toggle] in middle of heading', () => {
    const markdown = `# This has [toggle] in the middle`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const firstNode = result.children[0]
    expect(firstNode.type).toBe('heading')
    expect(firstNode.type).not.toBe('toggleBlock')
  })

  it('should handle multiple toggle blocks in same document', () => {
    const markdown = `# [toggle] First Section

Content 1

# [toggle] Second Section

Content 2`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    expect(result.children.length).toBe(2)
    expect(result.children[0].type).toBe('toggleBlock')
    expect(result.children[1].type).toBe('toggleBlock')
  })

  it('should handle toggle block with no content', () => {
    const markdown = `# [toggle] Empty Section

# Next Section`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    expect(toggleBlock.type).toBe('toggleBlock')
    expect(toggleBlock.children).toEqual([])
  })

  it('should handle whitespace after [toggle]', () => {
    const markdown = `# [toggle]   Heading with spaces`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    expect(toggleBlock.type).toBe('toggleBlock')

    const heading = toggleBlock.heading as Heading
    const firstChild = heading.children[0]
    expect((firstChild as any).value).toBe('Heading with spaces')
  })

  it('should handle nested headings within toggle block', () => {
    const markdown = `# [toggle] Main

## Subsection

### Sub-subsection

# Next Main`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const toggleBlock = result.children[0] as any
    expect(toggleBlock.type).toBe('toggleBlock')

    // Should include nested headings of lower level
    const headings = toggleBlock.children.filter((child: any) => child.type === 'heading')
    expect(headings.length).toBeGreaterThan(0)
  })
})
