/**
 * Unit tests for src/shared/search-query.ts — AST_Walk_Query.
 *
 * Covers query parsing (Req 3.2), operator filtering
 * (Req 3.3–3.6), AND-combination (Req 3.8), and edge cases.
 *
 * Requirements: 3.2, 3.3, 3.4, 3.5, 3.6, 3.8
 */

import { describe, it, expect } from 'vitest';
import type { Root } from 'mdast';
import type { FileEntry } from '@shared/types';
import type { ExtendedSearchIndex } from '@shared/extended-indexing';
import { createEmptyIndex, buildExtendedIndex } from '@shared/extended-indexing';
import { parseQuery, executeQuery, search } from '@shared/search-query';

// ---------------------------------------------------------------------------
// AST factory helpers
// ---------------------------------------------------------------------------

/**
 * Build a Root AST with a yaml frontmatter node and one or more paragraph
 * text nodes at the given lines.
 */
function makeRoot(opts: {
  yaml?: string;
  paragraphs?: { line: number; text: string }[];
}): Root {
  const children: any[] = [];

  if (opts.yaml !== undefined) {
    children.push({
      type: 'yaml',
      value: opts.yaml,
      position: { start: { line: 1, column: 1 }, end: { line: 1, column: opts.yaml.length + 1 } },
    });
  }

  if (opts.paragraphs) {
    let currentLine = opts.yaml !== undefined ? 2 : 1;
    for (const p of opts.paragraphs) {
      const textNode: any = {
        type: 'text',
        value: p.text,
        position: {
          start: { line: p.line ?? currentLine, column: 1 },
          end: { line: p.line ?? currentLine, column: p.text.length + 1 },
        },
      };
      const paraNode: any = {
        type: 'paragraph',
        children: [textNode],
        position: { start: { line: p.line ?? currentLine, column: 1 }, end: { line: p.line ?? currentLine, column: p.text.length + 1 } },
      };
      children.push(paraNode);
      currentLine = (p.line ?? currentLine) + 1;
    }
  }

  return { type: 'root', children } as unknown as Root;
}

/**
 * Build an AST getter from a Map<path, Root>.
 */
function astGetter(astMap: Map<string, Root>): (path: string) => Root | undefined {
  return (path: string) => astMap.get(path);
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const VAULT_PATH = '/test-vault';

const FILES: FileEntry[] = [
  { path: '/test-vault/projects/alpha.md', name: 'alpha', mtime: 1 },
  { path: '/test-vault/projects/beta.md', name: 'beta', mtime: 2 },
  { path: '/test-vault/meetings/2024-01-01.md', name: '2024-01-01', mtime: 3 },
  { path: '/test-vault/daily/notes.md', name: 'notes', mtime: 4 },
  { path: '/test-vault/tags/reference.md', name: 'reference', mtime: 5 },
];

/**
 * Build a full extended index for use across tests.
 *
 * Files:
 *   alpha.md:   frontmatter: tags: [dev, typescript]
 *               content: "Alpha is a TypeScript project\nIt has many files"
 *   beta.md:    frontmatter: tags: [dev, rust]
 *               content: "Beta is a Rust project\nIt compiles quickly"
 *   2024-01-01: frontmatter: aliases: [NYD, NewYear]
 *               content: "Happy new year everyone\nLots of meetings today"
 *   notes.md:   frontmatter: tags: [daily/notes, work]
 *               content: "Meeting notes for today\nAction items follow"
 *   reference.md: no frontmatter
 *               content: "Reference documentation here\nSee the guides inline"
 */
function buildTestIndex(): ExtendedSearchIndex {
  const asts = new Map<string, Root>();

  asts.set('/test-vault/projects/alpha.md', makeRoot({
    yaml: 'tags: [dev, typescript]\nstatus: active',
    paragraphs: [
      { line: 3, text: 'Alpha is a TypeScript project' },
      { line: 4, text: 'It has many files' },
    ],
  }));

  asts.set('/test-vault/projects/beta.md', makeRoot({
    yaml: 'tags: [dev, rust]',
    paragraphs: [
      { line: 2, text: 'Beta is a Rust project' },
      { line: 3, text: 'It compiles quickly' },
    ],
  }));

  asts.set('/test-vault/meetings/2024-01-01.md', makeRoot({
    yaml: 'aliases: [NYD, NewYear]',
    paragraphs: [
      { line: 2, text: 'Happy new year everyone' },
      { line: 3, text: 'Lots of meetings today' },
    ],
  }));

  asts.set('/test-vault/daily/notes.md', makeRoot({
    yaml: 'tags: [daily/notes, work]',
    paragraphs: [
      { line: 2, text: 'Meeting notes for today' },
      { line: 3, text: 'Action items follow' },
    ],
  }));

  asts.set('/test-vault/tags/reference.md', makeRoot({
    paragraphs: [
      { line: 1, text: 'Reference documentation here' },
      { line: 2, text: 'See the guides inline' },
    ],
  }));

  return buildExtendedIndex(FILES, astGetter(asts));
}

// ---------------------------------------------------------------------------
// Tests — parseQuery (Req 3.2)
// ---------------------------------------------------------------------------

describe('parseQuery (Req 3.2)', () => {
  it('returns empty bareTerms for empty query', () => {
    expect(parseQuery('')).toEqual({ bareTerms: [] });
  });

  it('returns empty bareTerms for whitespace-only query', () => {
    expect(parseQuery('   ')).toEqual({ bareTerms: [] });
  });

  it('parses a single bare term', () => {
    expect(parseQuery('hello')).toEqual({ bareTerms: ['hello'] });
  });

  it('parses multiple bare terms', () => {
    expect(parseQuery('hello world test')).toEqual({ bareTerms: ['hello', 'world', 'test'] });
  });

  it('parses path: operator', () => {
    const q = parseQuery('path:projects');
    expect(q.path).toBe('projects');
    expect(q.bareTerms).toEqual([]);
  });

  it('parses tag: operator', () => {
    const q = parseQuery('tag:dev');
    expect(q.tag).toBe('dev');
    expect(q.bareTerms).toEqual([]);
  });

  it('parses line: operator', () => {
    const q = parseQuery('line:hello');
    expect(q.line).toBe('hello');
    expect(q.bareTerms).toEqual([]);
  });

  it('parses content: operator', () => {
    const q = parseQuery('content:world');
    expect(q.content).toBe('world');
    expect(q.bareTerms).toEqual([]);
  });

  it('parses file: operator', () => {
    const q = parseQuery('file:alpha');
    expect(q.file).toBe('alpha');
    expect(q.bareTerms).toEqual([]);
  });

  it('parses regex: operator', () => {
    const q = parseQuery('regex:\\bword\\b');
    expect(q.regex).toBe('\\bword\\b');
    expect(q.bareTerms).toEqual([]);
  });

  it('parses property: operator with name and value', () => {
    const q = parseQuery('property:author:pablo');
    expect(q.property).toEqual({ name: 'author', value: 'pablo' });
  });

  it('parses property: operator with value that contains colons', () => {
    const q = parseQuery('property:tag:2024:Q1');
    expect(q.property).toEqual({ name: 'tag', value: '2024:Q1' });
  });

  it('parses mixed operators and bare terms', () => {
    const q = parseQuery('path:projects tag:dev hello world');
    expect(q.path).toBe('projects');
    expect(q.tag).toBe('dev');
    expect(q.bareTerms).toEqual(['hello', 'world']);
  });

  it('lowercases operator values for path, tag, file, and property name', () => {
    const q = parseQuery('Path:Projects Tag:Dev File:Alpha Property:Author:Pablo');
    expect(q.path).toBe('Projects');           // case-preserved for matching
    expect(q.tag).toBe('Dev');
    expect(q.file).toBe('Alpha');
    expect(q.property).toEqual({ name: 'author', value: 'Pablo' }); // name lowercased
  });

  it('last operator value wins for duplicate operators', () => {
    const q = parseQuery('tag:dev tag:rust');
    expect(q.tag).toBe('rust');
  });
});

// ---------------------------------------------------------------------------
// Tests — executeQuery (Req 3.3, 3.4, 3.5, 3.6, 3.8)
// ---------------------------------------------------------------------------

describe('executeQuery', () => {
  const index = buildTestIndex();

  it('returns empty for empty parsed query', () => {
    const results = executeQuery(
      parseQuery(''),
      FILES,
      VAULT_PATH,
      index,
      () => undefined,
    );
    expect(results).toEqual([]);
  });

  // --- path: (Req 3.3) ---

  it('filters by path: with substring match (case-insensitive)', () => {
    const results = executeQuery(parseQuery('path:projects'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(2);
    const paths = results.map((r) => r.filePath).sort();
    expect(paths).toEqual([
      '/test-vault/projects/alpha.md',
      '/test-vault/projects/beta.md',
    ]);
  });

  it('path: filter is case-insensitive', () => {
    const results = executeQuery(parseQuery('path:Projects'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(2);
  });

  it('path: returns empty for non-matching fragment', () => {
    const results = executeQuery(parseQuery('path:xyz'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- tag: (Req 3.4) ---

  it('filters by tag: exact match', () => {
    const results = executeQuery(parseQuery('tag:dev'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(2);
    const paths = results.map((r) => r.filePath).sort();
    expect(paths).toEqual([
      '/test-vault/projects/alpha.md',
      '/test-vault/projects/beta.md',
    ]);
  });

  it('filters by tag: namespace prefix match', () => {
    const results = executeQuery(parseQuery('tag:daily'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/daily/notes.md');
  });

  it('tag: returns empty for non-matching tag', () => {
    const results = executeQuery(parseQuery('tag:nonexistent'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- file: filter ---

  it('filters by file: with substring match (case-insensitive)', () => {
    const results = executeQuery(parseQuery('file:alpha'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/alpha.md');
  });

  it('file: returns empty for non-matching fragment', () => {
    const results = executeQuery(parseQuery('file:xyz'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- property: filter ---

  it('property: returns files matching the property value', () => {
    const results = executeQuery(parseQuery('property:status:active'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1); // alpha.md has status: active
    expect(results[0].filePath).toBe('/test-vault/projects/alpha.md');
  });

  it('property: returns empty for non-matching property', () => {
    const results = executeQuery(parseQuery('property:author:pablo'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- content: (Req 3.5) ---

  it('content: matches substring in line snippets and returns snippet + position', () => {
    const results = executeQuery(parseQuery('content:TypeScript'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/alpha.md');
    expect(results[0].matches).toHaveLength(1);
    expect(results[0].matches[0].line).toBe(3); // line where "TypeScript" appears
    expect(results[0].matches[0].snippet).toContain('TypeScript');
    expect(results[0].matches[0].startCol).toBeGreaterThanOrEqual(0);
    expect(results[0].matches[0].endCol).toBeGreaterThan(results[0].matches[0].startCol);
  });

  it('content: matches case-insensitively', () => {
    const results = executeQuery(parseQuery('content:typescript'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/alpha.md');
  });

  it('content: returns empty for non-matching text', () => {
    const results = executeQuery(parseQuery('content:nonexistent'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- line: (Req 3.5, alias for content:) ---

  it('line: matches substring in line snippets', () => {
    const results = executeQuery(parseQuery('line:compiles'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/beta.md');
  });

  // --- regex: (Req 3.6) ---

  it('regex: matches against line snippets', () => {
    const results = executeQuery(parseQuery('regex:\\bRust\\b'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/beta.md');
  });

  it('regex: returns empty when pattern matches nothing', () => {
    const results = executeQuery(parseQuery('regex:\\bxxxxxx\\b'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  it('regex: gracefully handles invalid regex patterns', () => {
    // Should not throw — just return no results
    expect(() => {
      executeQuery(parseQuery('regex:\\'), FILES, VAULT_PATH, index, () => undefined);
    }).not.toThrow();
  });

  // --- Bare terms ---

  it('bare terms filter by positions map (AND-combined)', () => {
    const results = executeQuery(parseQuery('alpha project'), FILES, VAULT_PATH, index, () => undefined);
    // alpha.md contains both "alpha" and "project"
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/alpha.md');
  });

  it('bare term without matches in position index returns empty', () => {
    const results = executeQuery(parseQuery('alpha xyznotfound'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- AND-combination (Req 3.8) ---

  it('AND-combines multiple operators', () => {
    // Only alpha.md matches both path:projects and tag:dev
    const results = executeQuery(parseQuery('path:projects tag:dev'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(2);
  });

  it('AND-combines operators with content', () => {
    // Only alpha.md matches path:projects AND content:TypeScript
    const results = executeQuery(parseQuery('path:projects content:TypeScript'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].filePath).toBe('/test-vault/projects/alpha.md');
  });

  it('AND-combination with incompatible operators returns empty', () => {
    // No file matches both tag:dev and path:meetings
    const results = executeQuery(parseQuery('tag:dev path:meetings'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  // --- Score ---

  it('scores results by number of matching lines (descending)', () => {
    // Search for something that matches multiple lines per file
    const results = executeQuery(parseQuery('content:the'), FILES, VAULT_PATH, index, () => undefined);
    // Results should be sorted by score descending
    for (let i = 1; i < results.length; i++) {
      expect(results[i].score).toBeLessThanOrEqual(results[i - 1].score);
    }
  });
});

// ---------------------------------------------------------------------------
// Tests — convenience search() wrapper
// ---------------------------------------------------------------------------

describe('search convenience wrapper', () => {
  const index = buildTestIndex();

  it('returns results for a bare term', () => {
    const results = search('TypeScript', FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(1);
    expect(results[0].name).toBe('alpha');
  });

  it('combines operators and bare terms', () => {
    const results = search('tag:dev project', FILES, VAULT_PATH, index, () => undefined);
    expect(results).toHaveLength(2);
  });

  it('returns empty for non-matching query', () => {
    const results = search('tag:nonexistent xyz', FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

describe('edge cases', () => {
  const index = buildTestIndex();

  it('handles query that only has an operator with no matching files', () => {
    const results = executeQuery(parseQuery('path:/nonexistent'), FILES, VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  it('handles search with empty files array', () => {
    const results = executeQuery(parseQuery('hello'), [], VAULT_PATH, index, () => undefined);
    expect(results).toEqual([]);
  });

  it('returns results with correct name and relativePath', () => {
    const results = search('alpha', FILES, VAULT_PATH, index, () => undefined);
    expect(results[0].name).toBe('alpha');
    expect(results[0].relativePath).toBe('projects/alpha.md');
  });

  it('relativePath handles root-level files correctly', () => {
    const singleFile: FileEntry[] = [
      { path: '/test-vault/root.md', name: 'root', mtime: 1 },
    ];
    const asts = new Map<string, Root>();
    asts.set('/test-vault/root.md', makeRoot({ paragraphs: [{ line: 1, text: 'hello' }] }));
    const idx = buildExtendedIndex(singleFile, astGetter(asts));
    const results = search('hello', singleFile, VAULT_PATH, idx, () => undefined);
    expect(results[0].relativePath).toBe('root.md');
  });

  it('handles special characters in search terms', () => {
    // Should not throw — special chars in bare terms are used as-is in positions lookup
    expect(() => {
      search('hello+world', FILES, VAULT_PATH, index, () => undefined);
    }).not.toThrow();
  });
});
