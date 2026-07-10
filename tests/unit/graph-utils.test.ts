/**
 * graph-utils.test.ts — Unit tests for graph-utils module
 *
 * Validates: Requirements 38.1, 38.2, 38.3, 38.4, 38.5, 38.6
 */

import { describe, it, expect } from 'vitest'
import {
  computeTagGraph,
  computeTagNodeRadius,
  getTagNodeColor,
  getTagDisplayLabel
} from '@shared/graph-utils'
import type { ExtendedSearchIndex } from '@shared/extended-indexing'
import type { FileEntry } from '@shared/types'

describe('computeTagGraph', () => {
  it('returns empty arrays when tagIndex is empty', () => {
    const emptyIndex: ExtendedSearchIndex = {
      positions: new Map(),
      lineSnippets: new Map(),
      tagIndex: new Map(),
      aliasIndex: new Map(),
      propertyIndex: new Map(),
      blockRefs: new Map()
    }
    const files: FileEntry[] = [
      { path: '/vault/note1.md', name: 'note1.md', mtime: 0 },
      { path: '/vault/note2.md', name: 'note2.md', mtime: 0 }
    ]

    const result = computeTagGraph(emptyIndex, files)

    expect(result.nodes).toEqual([])
    expect(result.edges).toEqual([])
  })

  it('creates nodes for each tag with correct count (Req 38.2)', () => {
    const index: ExtendedSearchIndex = {
      positions: new Map(),
      lineSnippets: new Map(),
      tagIndex: new Map([
        ['project/nabu', new Set(['/vault/note1.md', '/vault/note2.md'])],
        ['project', new Set(['/vault/note1.md'])]
      ]),
      aliasIndex: new Map(),
      propertyIndex: new Map(),
      blockRefs: new Map()
    }
    const files: FileEntry[] = [
      { path: '/vault/note1.md', name: 'note1.md', mtime: 0 },
      { path: '/vault/note2.md', name: 'note2.md', mtime: 0 }
    ]

    const result = computeTagGraph(index, files)

    // Should have 2 nodes: project/nabu and project (parent tag)
    expect(result.nodes.length).toBe(2)

    const node1 = result.nodes.find((n) => n.label === 'project/nabu')
    expect(node1?.count).toBe(2)

    const node2 = result.nodes.find((n) => n.label === 'project')
    expect(node2?.count).toBe(1)
  })

  it('creates co-occurrence edges between tags on same file (Req 38.3)', () => {
    const index: ExtendedSearchIndex = {
      positions: new Map(),
      lineSnippets: new Map(),
      tagIndex: new Map([
        ['tag-a', new Set(['/vault/note1.md'])],
        ['tag-b', new Set(['/vault/note1.md'])],
        ['tag-c', new Set(['/vault/note2.md'])]
      ]),
      aliasIndex: new Map(),
      propertyIndex: new Map(),
      blockRefs: new Map()
    }
    const files: FileEntry[] = [
      { path: '/vault/note1.md', name: 'note1.md', mtime: 0 },
      { path: '/vault/note2.md', name: 'note2.md', mtime: 0 }
    ]

    const result = computeTagGraph(index, files)

    // Should have 3 nodes
    expect(result.nodes.length).toBe(3)

    // Should have 1 edge between tag-a and tag-b
    expect(result.edges.length).toBe(1)
    expect(
      result.edges.some(
        (e) =>
          (e.source === 'tag-a' && e.target === 'tag-b') ||
          (e.source === 'tag-b' && e.target === 'tag-a')
      )
    ).toBe(true)

    // Edge should have cooccurrence = 1
    const edge = result.edges[0]
    expect(edge.cooccurrence).toBe(1)
  })

  it('creates symmetric edges - co-occurrence is bidirectional (Req 38.3)', () => {
    const index: ExtendedSearchIndex = {
      positions: new Map(),
      lineSnippets: new Map(),
      tagIndex: new Map([
        ['alpha', new Set(['/vault/note1.md'])],
        ['beta', new Set(['/vault/note1.md'])]
      ]),
      aliasIndex: new Map(),
      propertyIndex: new Map(),
      blockRefs: new Map()
    }
    const files: FileEntry[] = [{ path: '/vault/note1.md', name: 'note1.md', mtime: 0 }]

    const result = computeTagGraph(index, files)

    // Edge should be symmetric regardless of sort order
    expect(result.edges.length).toBe(1)
    const edge = result.edges[0]
    expect(edge.source).toBe('alpha')
    expect(edge.target).toBe('beta')
    expect(edge.cooccurrence).toBe(1)
  })
})

describe('computeTagNodeRadius', () => {
  it('returns minimum radius for count of 0', () => {
    expect(computeTagNodeRadius(0, 100)).toBe(4)
  })

  it('returns minimum radius when maxFiles is 0', () => {
    expect(computeTagNodeRadius(10, 0)).toBe(8)
  })

  it('scales radius logarithmically with count', () => {
    const r1 = computeTagNodeRadius(1, 100)
    const r10 = computeTagNodeRadius(10, 100)
    const r50 = computeTagNodeRadius(50, 100)
    const r100 = computeTagNodeRadius(100, 100)

    // Radius should increase with count
    expect(r1).toBeGreaterThan(4)
    expect(r10).toBeGreaterThan(r1)
    expect(r50).toBeGreaterThan(r10)
    expect(r100).toBeGreaterThan(r50)

    // Max radius should be capped
    expect(r100).toBeLessThanOrEqual(20)
  })
})

describe('getTagNodeColor', () => {
  it('returns deterministic color for same tag', () => {
    const color1 = getTagNodeColor('project/nabu')
    const color2 = getTagNodeColor('project/nabu')

    expect(color1).toBe(color2)
  })

  it('returns valid color from palette', () => {
    const validColors = ['blue', 'red', 'green', 'yellow', 'purple', 'orange', 'cyan', 'pink']
    const color = getTagNodeColor('any-tag')

    expect(validColors).toContain(color)
  })

  it('returns different colors for different tags typically', () => {
    const colors = new Set([
      getTagNodeColor('tag-a'),
      getTagNodeColor('tag-b'),
      getTagNodeColor('tag-c')
    ])

    // At least some should differ (probabilistic but very likely)
    expect(colors.size).toBeGreaterThan(1)
  })
})

describe('getTagDisplayLabel', () => {
  it('returns full tag when no slash present', () => {
    expect(getTagDisplayLabel('notnested')).toBe('notnested')
  })

  it('returns last segment for namespaced tags (Req 38.4)', () => {
    expect(getTagDisplayLabel('parent/child')).toBe('child')
    expect(getTagDisplayLabel('parent/child/grandchild')).toBe('grandchild')
  })
})
