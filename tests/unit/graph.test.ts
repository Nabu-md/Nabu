/**
 * Property-based tests for buildGraph()
 *
 * Validates: Requirements 6.1, 6.11
 */

import { describe, it } from 'vitest'
import * as fc from 'fast-check'
import type { Root } from 'mdast'
import { buildGraph } from '@shared/graph'
import type { FileEntry } from '@shared/types'

// ---- Generators ----

const fileEntryArb = fc.record({
  path: fc.string({ minLength: 1, maxLength: 30 }).map((s) => `/vault/${s}.md`),
  name: fc.string({ minLength: 1, maxLength: 20 }),
  mtime: fc.nat()
})

// Builds a minimal Root AST optionally containing wikiLink nodes targeting some file names
function makeAST(targets: string[]): Root {
  return {
    type: 'root',
    children: targets.map(
      (t) =>
        ({
          type: 'wikiLink',
          target: t,
          resolved: false,
          position: undefined
        }) as any
    )
  } as Root
}

describe('buildGraph — property-based tests', () => {
  it('Property 1: no orphaned edges — every edge source and target is in files', () => {
    fc.assert(
      fc.property(fc.array(fileEntryArb, { minLength: 0, maxLength: 10 }), (files) => {
        // Deduplicate by path
        const uniqueFiles: FileEntry[] = []
        const seen = new Set<string>()
        for (const f of files) {
          if (!seen.has(f.path)) {
            seen.add(f.path)
            uniqueFiles.push(f)
          }
        }
        // Build ASTs with random cross-links
        const getAST = (path: string): Root | undefined => {
          const file = uniqueFiles.find((f) => f.path === path)
          if (!file) return undefined
          const otherNames = uniqueFiles.filter((f) => f.path !== path).map((f) => f.name)
          return makeAST(otherNames.slice(0, 2))
        }
        const edges = buildGraph(uniqueFiles, getAST)
        const pathSet = new Set(uniqueFiles.map((f) => f.path))
        return edges.every((e) => pathSet.has(e.source) && pathSet.has(e.target))
      })
    )
  })

  it('Property 2: wiki-link inclusion — a resolved wiki-link produces an edge', () => {
    fc.assert(
      fc.property(fc.array(fileEntryArb, { minLength: 2, maxLength: 8 }), (files) => {
        const unique: FileEntry[] = []
        const seen = new Set<string>()
        for (const f of files) {
          if (!seen.has(f.path) && !seen.has(f.name)) {
            seen.add(f.path)
            seen.add(f.name)
            unique.push(f)
          }
        }
        if (unique.length < 2) return true
        const [source, target] = unique
        const getAST = (path: string): Root | undefined => {
          if (path === source.path) return makeAST([target.name])
          return makeAST([])
        }
        const edges = buildGraph(unique, getAST)
        return edges.some((e) => e.source === source.path && e.target === target.path)
      })
    )
  })

  it('Property 3: idempotence — calling buildGraph twice returns same edges', () => {
    fc.assert(
      fc.property(fc.array(fileEntryArb, { minLength: 0, maxLength: 8 }), (files) => {
        const unique: FileEntry[] = []
        const seen = new Set<string>()
        for (const f of files) {
          if (!seen.has(f.path) && !seen.has(f.name)) {
            seen.add(f.path)
            seen.add(f.name)
            unique.push(f)
          }
        }
        const getAST = (path: string): Root | undefined => {
          const idx = unique.findIndex((f) => f.path === path)
          if (idx < 0) return undefined
          const targets = unique
            .filter((_, i) => i !== idx)
            .slice(0, 1)
            .map((f) => f.name)
          return makeAST(targets)
        }
        const sort = (arr: typeof edges) =>
          [...arr].sort((a, b) => `${a.source}${a.target}`.localeCompare(`${b.source}${b.target}`))
        const edges = buildGraph(unique, getAST)
        const edges2 = buildGraph(unique, getAST)
        const s1 = sort(edges).map((e) => `${e.source}->${e.target}`)
        const s2 = sort(edges2).map((e) => `${e.source}->${e.target}`)
        return JSON.stringify(s1) === JSON.stringify(s2)
      })
    )
  })

  it('Property 4.5: alias resolution — wiki-link target matching an alias creates an edge', () => {
    fc.assert(
      fc.property(fc.array(fileEntryArb, { minLength: 2, maxLength: 4 }), (files) => {
        const unique: FileEntry[] = []
        const seen = new Set<string>()
        for (const f of files) {
          if (!seen.has(f.path) && !seen.has(f.name)) {
            seen.add(f.path)
            seen.add(f.name)
            unique.push(f)
          }
        }
        if (unique.length < 2) return true

        const [source, target] = unique
        // Create an alias for the target file
        const aliasName = `alias-for-${target.name}`
        const aliasIndex = new Map<string, string[]>()
        aliasIndex.set(aliasName.toLowerCase(), [target.path])

        // Source links to the alias name, not the file name
        const getAST = (path: string): Root | undefined => {
          if (path === source.path) return makeAST([aliasName])
          return makeAST([])
        }

        const edges = buildGraph(unique, getAST, aliasIndex)
        return edges.some((e) => e.source === source.path && e.target === target.path)
      })
    )
  })

  it('Property 4: subset on file removal — removing a file removes its edges', () => {
    fc.assert(
      fc.property(fc.array(fileEntryArb, { minLength: 2, maxLength: 8 }), (files) => {
        const unique: FileEntry[] = []
        const seen = new Set<string>()
        for (const f of files) {
          if (!seen.has(f.path) && !seen.has(f.name)) {
            seen.add(f.path)
            seen.add(f.name)
            unique.push(f)
          }
        }
        if (unique.length < 2) return true
        const removed = unique[0]
        const reduced = unique.slice(1)
        const getAST = (path: string): Root | undefined => {
          const idx = unique.findIndex((f) => f.path === path)
          if (idx < 0) return undefined
          const targets = unique
            .filter((_, i) => i !== idx)
            .slice(0, 1)
            .map((f) => f.name)
          return makeAST(targets)
        }
        const edgesReduced = buildGraph(reduced, getAST)
        return edgesReduced.every((e) => e.source !== removed.path && e.target !== removed.path)
      })
    )
  })
})
