/**
 * roundtrip.test.ts
 *
 * Round-trip fidelity test suite for the Nabu parser.
 *
 * For every fixture file in tests/fixtures/markdown/:
 *   1. Parse the fixture  →  AST₁
 *   2. Serialize AST₁     →  markdown string
 *   3. Re-parse the string →  AST₂
 *   4. Assert that the normalised AST₁ and AST₂ are structurally identical
 *      (same node types, same tree shape, same leaf text / values).
 *   5. If a YAML front-matter node is present, assert the parsed YAML object
 *      from AST₁ and AST₂ are deeply equal (no key reordering / value coercion).
 *
 * Validates: Requirements 10.4
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest'
import { readdirSync } from 'fs'
import { writeFile, mkdir, rm } from 'fs/promises'
import { join, resolve } from 'path'
import { tmpdir } from 'os'
import { parseFile, serializeAST } from '@main/parser'
import type { Root } from 'mdast'

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const FIXTURES_DIR = resolve(__dirname, '../fixtures/markdown')

// ---------------------------------------------------------------------------
// Temporary directory for serialised round-trip files
// ---------------------------------------------------------------------------

let tmpDir: string

beforeAll(async () => {
  tmpDir = join(tmpdir(), `nabu-roundtrip-${Date.now()}`)
  await mkdir(tmpDir, { recursive: true })
})

afterAll(async () => {
  try {
    await rm(tmpDir, { recursive: true, force: true })
  } catch {
    // best-effort cleanup
  }
})

// ---------------------------------------------------------------------------
// AST normalisation
// ---------------------------------------------------------------------------

/**
 * A lightweight, comparable representation of an AST node.
 * Strips all position / offset data and keeps only type, optional scalar
 * values, and the children/items hierarchy.
 */
interface NormNode {
  type: string
  /** text / code content for leaf nodes */
  value?: string
  /** heading depth */
  depth?: number
  /** code block language */
  lang?: string | null
  /** wikiLink target */
  target?: string
  /** taskItem checked state */
  checked?: boolean
  /** table cell alignment */
  align?: Array<'left' | 'right' | 'center' | null> | null
  /** ordered list start */
  ordered?: boolean
  children?: NormNode[]
  items?: NormNode[]
  heading?: NormNode
}

/**
 * Recursively normalise an mdast node (or custom plugin node) into a
 * structure that is safe for deep equality comparison.
 */
function normalizeNode(node: Record<string, unknown>): NormNode {
  const norm: NormNode = { type: node.type as string }

  // Scalar values present on leaf / semi-leaf nodes
  if (typeof node.value === 'string') norm.value = node.value
  if (typeof node.depth === 'number') norm.depth = node.depth
  if (node.lang !== undefined) norm.lang = node.lang as string | null
  if (typeof node.target === 'string') norm.target = node.target
  if (typeof node.checked === 'boolean') norm.checked = node.checked
  if (node.align !== undefined) norm.align = node.align as NormNode['align']
  if (typeof node.ordered === 'boolean') norm.ordered = node.ordered

  // Recurse into children (standard mdast)
  if (Array.isArray(node.children)) {
    norm.children = (node.children as Record<string, unknown>[]).map(normalizeNode)
  }

  // Recurse into items (taskList custom node)
  if (Array.isArray(node.items)) {
    norm.items = (node.items as Record<string, unknown>[]).map(normalizeNode)
  }

  // Recurse into heading (toggleBlock custom node)
  if (node.heading && typeof node.heading === 'object') {
    norm.heading = normalizeNode(node.heading as Record<string, unknown>)
  }

  return norm
}

function normalizeAST(ast: Root): NormNode {
  return normalizeNode(ast as unknown as Record<string, unknown>)
}

// ---------------------------------------------------------------------------
// YAML front matter helpers
// ---------------------------------------------------------------------------

/**
 * Extract the raw YAML string from a parsed AST (the value of the first
 * `yaml` node, if present).
 */
function extractYamlValue(ast: Root): string | null {
  const yamlNode = ast.children.find((n) => n.type === 'yaml') as
    { type: 'yaml'; value: string } | undefined
  return yamlNode?.value ?? null
}

/**
 * Parse a YAML string into a plain object without pulling in an external
 * library.  We rely on the fact that remark-frontmatter already validated
 * the YAML, and we use a simple key=value line scan to produce a comparable
 * structure.
 *
 * This does not need to be a full YAML parser — it just needs to be
 * deterministic so that AST₁ and AST₂ produce equal outputs for the same
 * canonical YAML string.
 */
function parseYamlToObject(yaml: string): Record<string, string> {
  const result: Record<string, string> = {}
  for (const line of yaml.split('\n')) {
    const colonIdx = line.indexOf(':')
    if (colonIdx === -1) continue
    const key = line.slice(0, colonIdx).trim()
    const val = line.slice(colonIdx + 1).trim()
    if (key) result[key] = val
  }
  return result
}

// ---------------------------------------------------------------------------
// Fixtures enumeration
// ---------------------------------------------------------------------------

const fixtureFiles = readdirSync(FIXTURES_DIR)
  .filter((f) => f.endsWith('.md'))
  .sort()

// ---------------------------------------------------------------------------
// Round-trip test suite
// ---------------------------------------------------------------------------

describe('Round-trip fidelity (Req 10.4)', () => {
  for (const filename of fixtureFiles) {
    it(`round-trips: ${filename}`, async () => {
      const fixturePath = join(FIXTURES_DIR, filename)

      // ── Step 1: Parse fixture ──────────────────────────────────────────
      const { ast: ast1, error: err1 } = await parseFile(fixturePath)

      // Even if the parser returned a soft error, we still expect an AST.
      expect(ast1, `[${filename}] parseFile must return an AST`).toBeDefined()
      expect(ast1.type, `[${filename}] AST root must have type 'root'`).toBe('root')

      if (err1) {
        // A soft parse error is acceptable (parser uses error recovery);
        // log it so failures are diagnosable.
        console.warn(`[${filename}] soft parse error on first parse: ${err1.message}`)
      }

      // ── Step 2: Serialize AST₁ → markdown string ──────────────────────
      const serialized = await serializeAST(ast1)
      expect(typeof serialized, `[${filename}] serializeAST must return a string`).toBe('string')

      // ── Step 3: Write serialized string to temp file and re-parse ─────
      const tmpFile = join(tmpDir, filename)
      await writeFile(tmpFile, serialized, 'utf8')
      const { ast: ast2, error: err2 } = await parseFile(tmpFile)

      expect(ast2, `[${filename}] second parseFile must return an AST`).toBeDefined()

      if (err2) {
        console.warn(`[${filename}] soft parse error on second parse: ${err2.message}`)
      }

      // ── Step 4: Compare normalised ASTs ───────────────────────────────
      const norm1 = normalizeAST(ast1)
      const norm2 = normalizeAST(ast2)

      expect(norm2, `[${filename}] normalised AST after round-trip must match original`).toEqual(
        norm1
      )

      // ── Step 5: YAML front-matter preservation ────────────────────────
      const yaml1 = extractYamlValue(ast1)
      const yaml2 = extractYamlValue(ast2)

      if (yaml1 !== null) {
        expect(
          yaml2,
          `[${filename}] front matter must be preserved after round-trip`
        ).not.toBeNull()

        // Compare the parsed YAML objects so key order differences don't cause
        // false negatives, but value coercion (e.g. number → string) does.
        const obj1 = parseYamlToObject(yaml1)
        const obj2 = parseYamlToObject(yaml2!)

        expect(obj2, `[${filename}] YAML front matter keys/values must be preserved`).toEqual(obj1)
      } else {
        // No front matter in original → should have none after round-trip either
        expect(
          yaml2,
          `[${filename}] no front matter in original, should have none after round-trip`
        ).toBeNull()
      }
    })
  }
})
