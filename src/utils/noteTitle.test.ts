import { describe, expect, it } from 'vitest'
import {
  contentDefinesDisplayTitle,
  deriveDisplayTitleState,
  extractFrontmatterTitleFromContent,
  extractH1TitleFromContent,
  filenameStemToTitle,
} from './noteTitle'

describe('filenameStemToTitle', () => {
  it('converts kebab-case filenames into title case', () => {
    expect(filenameStemToTitle('renamed-note.md')).toBe('Renamed Note')
  })
})

describe('extractH1TitleFromContent', () => {
  it('extracts the first H1 after frontmatter', () => {
    const content = '---\ntitle: Legacy Title\n---\n# Updated Title\n\nBody'
    expect(extractH1TitleFromContent(content)).toBe('Updated Title')
  })

  it('strips markdown formatting from the H1', () => {
    const content = '# **Bold** [Link](https://example.com) and `code`'
    expect(extractH1TitleFromContent(content)).toBe('Bold Link and code')
  })

  it('preserves plain square brackets in the H1', () => {
    const content = '# [26Q2] Nabu MVP'
    expect(extractH1TitleFromContent(content)).toBe('[26Q2] Nabu MVP')
  })

  it('returns null when the first non-empty line is not an H1', () => {
    expect(extractH1TitleFromContent('Body first\n# Not the title')).toBeNull()
  })
})

describe('extractFrontmatterTitleFromContent', () => {
  it('extracts the frontmatter title when present', () => {
    const content = '---\ntitle: Legacy Title\nstatus: Active\n---\n## Body'
    expect(extractFrontmatterTitleFromContent(content)).toBe('Legacy Title')
  })

  it('returns null when the frontmatter title is missing', () => {
    expect(extractFrontmatterTitleFromContent('---\nstatus: Active\n---\n## Body')).toBeNull()
  })
})

describe('contentDefinesDisplayTitle', () => {
  it('returns true when the document title comes from frontmatter', () => {
    const content = '---\ntitle: Spring 2026\n---\n## Goals'
    expect(contentDefinesDisplayTitle(content)).toBe(true)
  })

  it('returns false when title still comes from the filename', () => {
    expect(contentDefinesDisplayTitle('Body only')).toBe(false)
  })
})

describe('deriveDisplayTitleState', () => {
  it('prefers H1 over frontmatter title and filename', () => {
    const content = '---\ntitle: Legacy Title\n---\n# Updated Title\n\nBody'
    expect(deriveDisplayTitleState({ content, filename: 'legacy-title.md', frontmatterTitle: 'Legacy Title' })).toEqual({
      title: 'Updated Title',
      hasH1: true,
    })
  })

  it('falls back to frontmatter title when no H1 is present', () => {
    const content = '---\ntitle: Legacy Title\n---\nBody'
    expect(deriveDisplayTitleState({ content, filename: 'legacy-title.md', frontmatterTitle: 'Legacy Title' })).toEqual({
      title: 'Legacy Title',
      hasH1: false,
    })
  })

  it('reads the frontmatter title from content when no explicit title is passed', () => {
    const content = '---\ntitle: Spring 2026\n---\n## Goals'
    expect(deriveDisplayTitleState({ content, filename: 'spring-2026.md' })).toEqual({
      title: 'Spring 2026',
      hasH1: false,
    })
  })

  it('falls back to filename title when there is no H1 or frontmatter title', () => {
    expect(deriveDisplayTitleState({ content: 'Body only', filename: 'renamed-note.md' })).toEqual({
      title: 'Renamed Note',
      hasH1: false,
    })
  })

  it('keeps plain square brackets when deriving the display title from H1', () => {
    const content = '# [26Q2] Nabu MVP\n\nBody'
    expect(deriveDisplayTitleState({ content, filename: 'nabu-mvp.md' })).toEqual({
      title: '[26Q2] Nabu MVP',
      hasH1: true,
    })
  })
})
