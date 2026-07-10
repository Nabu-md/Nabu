/**
 * outgoing-links.test.ts
 *
 * Tests for the OutgoingLinksPanel filtering and dedup logic.
 *
 * Requirements: 6.1, 6.2, 6.3, 6.4, 6.5
 */

import { describe, it, expect } from 'vitest'
import type { Edge } from '../../src/shared/types'

// ---------------------------------------------------------------------------
// Pure logic helpers (mirrors what OutgoingLinksPanel does internally)
// ---------------------------------------------------------------------------

interface OutgoingLink {
  targetPath: string
  name: string
  snippet: string
}

function computeOutgoingLinks(
  currentFile: string | null,
  edges: Edge[],
  vaultFilePaths: string[]
): OutgoingLink[] {
  if (!currentFile) return []

  const seen = new Set<string>()
  return edges
    .filter((e) => e.source === currentFile)
    .filter((e) => {
      if (seen.has(e.target)) return false
      seen.add(e.target)
      return true
    })
    .map((e) => ({
      targetPath: e.target,
      name:
        vaultFilePaths
          .find((fp) => fp === e.target)
          ?.split('/')
          .pop()
          ?.replace('.md', '') ??
        e.target.split('/').pop()?.replace('.md', '') ??
        e.target,
      snippet: e.snippet
    }))
}

function isLinkBroken(targetPath: string, vaultFilePaths: string[]): boolean {
  return !vaultFilePaths.some((fp) => fp === targetPath)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('OutgoingLinksPanel logic', () => {
  const edges: Edge[] = [
    { source: '/vault/a.md', target: '/vault/b.md', snippet: 'link to b' },
    { source: '/vault/a.md', target: '/vault/c.md', snippet: 'link to c' },
    { source: '/vault/a.md', target: '/vault/b.md', snippet: 'duplicate' },
    { source: '/vault/b.md', target: '/vault/d.md', snippet: 'link to d' }
  ]

  const vaultFiles = ['/vault/b.md', '/vault/c.md', '/vault/d.md']

  it('returns outgoing links for the current file', () => {
    const links = computeOutgoingLinks('/vault/a.md', edges, vaultFiles)
    expect(links).toHaveLength(2)
    expect(links.map((l) => l.targetPath)).toEqual(
      expect.arrayContaining(['/vault/b.md', '/vault/c.md'])
    )
  })

  it('deduplicates by target', () => {
    const links = computeOutgoingLinks('/vault/a.md', edges, vaultFiles)
    expect(links).toHaveLength(2)
    // b.md should appear only once despite two edges
    const bLinks = links.filter((l) => l.targetPath === '/vault/b.md')
    expect(bLinks).toHaveLength(1)
  })

  it('returns empty array when currentFile is null', () => {
    expect(computeOutgoingLinks(null, edges, vaultFiles)).toHaveLength(0)
  })

  it('returns empty array when no outgoing edges exist', () => {
    expect(computeOutgoingLinks('/vault/none.md', edges, vaultFiles)).toHaveLength(0)
  })

  it('identifies broken links', () => {
    expect(isLinkBroken('/vault/b.md', vaultFiles)).toBe(false)
    expect(isLinkBroken('/vault/missing.md', vaultFiles)).toBe(true)
  })

  it('uses the first edges snippet after dedup', () => {
    const links = computeOutgoingLinks('/vault/a.md', edges, vaultFiles)
    const bLink = links.find((l) => l.targetPath === '/vault/b.md')
    // Should use the first occurrence's snippet
    expect(bLink?.snippet).toBe('link to b')
  })
})
