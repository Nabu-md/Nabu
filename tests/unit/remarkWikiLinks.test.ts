/**
 * Unit tests for remarkWikiLinks plugin
 * Validates Requirement 2.6
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import { remarkWikiLinks } from '@main/plugins/remarkWikiLinks'

describe('remarkWikiLinks', () => {
  const createProcessor = () => {
    return unified().use(remarkParse).use(remarkWikiLinks)
  }

  it('should transform [[Page Name]] into wikiLink node', () => {
    const markdown = `This is a link to [[Page Name]].`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    expect(paragraph.type).toBe('paragraph')

    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')
    expect(wikiLink).toBeDefined()
    expect(wikiLink.target).toBe('Page Name')
  })

  it('should set target property to inner text without brackets', () => {
    const markdown = `Link: [[My Note]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink.target).toBe('My Note')
    expect(wikiLink.target).not.toContain('[[')
    expect(wikiLink.target).not.toContain(']]')
  })

  it('should set resolved property to false by default', () => {
    const markdown = `[[Test Link]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink.resolved).toBe(false)
  })

  it('should handle multiple wiki links in same paragraph', () => {
    const markdown = `Link to [[First Note]] and [[Second Note]].`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLinks = paragraph.children.filter((child: any) => child.type === 'wikiLink')

    expect(wikiLinks.length).toBe(2)
    expect(wikiLinks[0].target).toBe('First Note')
    expect(wikiLinks[1].target).toBe('Second Note')
  })

  it('should preserve text before and after wiki links', () => {
    const markdown = `Text before [[Link]] and text after.`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any

    // Should have text nodes before and after the wiki link
    const textNodes = paragraph.children.filter((child: any) => child.type === 'text')
    expect(textNodes.length).toBeGreaterThan(0)

    const hasTextBefore = textNodes.some((node: any) => node.value.includes('Text before'))
    const hasTextAfter = textNodes.some((node: any) => node.value.includes('and text after'))

    expect(hasTextBefore).toBe(true)
    expect(hasTextAfter).toBe(true)
  })

  it('should handle wiki links with spaces', () => {
    const markdown = `[[Page With Multiple Words]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink.target).toBe('Page With Multiple Words')
  })

  it('should handle wiki links with special characters', () => {
    const markdown = `[[Page-Name_123]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink.target).toBe('Page-Name_123')
  })

  it('should not transform single brackets', () => {
    const markdown = `This is [not a wiki link]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink).toBeUndefined()
  })

  it('should handle consecutive wiki links', () => {
    const markdown = `[[First]][[Second]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLinks = paragraph.children.filter((child: any) => child.type === 'wikiLink')

    expect(wikiLinks.length).toBe(2)
    expect(wikiLinks[0].target).toBe('First')
    expect(wikiLinks[1].target).toBe('Second')
  })

  it('should handle wiki link at start of text', () => {
    const markdown = `[[Start Link]] followed by text`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const firstChild = paragraph.children[0]

    expect(firstChild.type).toBe('wikiLink')
    expect(firstChild.target).toBe('Start Link')
  })

  it('should handle wiki link at end of text', () => {
    const markdown = `Text followed by [[End Link]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const lastChild = paragraph.children[paragraph.children.length - 1]

    expect(lastChild.type).toBe('wikiLink')
    expect(lastChild.target).toBe('End Link')
  })

  it('should trim whitespace from target', () => {
    const markdown = `[[  Padded Link  ]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink.target).toBe('Padded Link')
  })

  it('should handle wiki links in headings', () => {
    const markdown = `# Heading with [[Wiki Link]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const heading = result.children[0] as any
    expect(heading.type).toBe('heading')

    const wikiLink = heading.children.find((child: any) => child.type === 'wikiLink')
    expect(wikiLink).toBeDefined()
    expect(wikiLink.target).toBe('Wiki Link')
  })

  it('should handle wiki links in list items', () => {
    const markdown = `- Item with [[Link]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const list = result.children[0] as any
    const listItem = list.children[0]
    const paragraph = listItem.children[0] as any

    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')
    expect(wikiLink).toBeDefined()
    expect(wikiLink.target).toBe('Link')
  })

  it('should not transform empty wiki links [[]]', () => {
    const markdown = `[[]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    // Empty brackets are not a valid wiki link
    expect(wikiLink).toBeUndefined()
  })

  it('should handle nested brackets in content', () => {
    const markdown = `Text before and after`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any

    // Should be just text, no wiki links
    const wikiLinks = paragraph.children.filter((child: any) => child.type === 'wikiLink')
    expect(wikiLinks.length).toBe(0)
  })

  it('should preserve node order with text and wiki links', () => {
    const markdown = `First [[Link1]] middle [[Link2]] last`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const childTypes = paragraph.children.map((child: any) => child.type)

    expect(childTypes).toEqual(['text', 'wikiLink', 'text', 'wikiLink', 'text'])
  })

  it('should handle wiki links with numbers', () => {
    const markdown = `[[Note 123]]`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const wikiLink = paragraph.children.find((child: any) => child.type === 'wikiLink')

    expect(wikiLink.target).toBe('Note 123')
  })

  it('should handle wiki links in emphasis', () => {
    const markdown = `*This is [[emphasized link]]*`

    const result = createProcessor().parse(markdown)
    createProcessor().runSync(result)

    const paragraph = result.children[0] as any
    const emphasis = paragraph.children.find((child: any) => child.type === 'emphasis')

    expect(emphasis).toBeDefined()

    // Wiki links inside emphasis should still be transformed
    const wikiLink = emphasis.children.find((child: any) => child.type === 'wikiLink')
    expect(wikiLink).toBeDefined()
    expect(wikiLink.target).toBe('emphasized link')
  })
})
