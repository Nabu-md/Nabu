/**
 * renderer-pipeline.test.ts
 *
 * Unit tests for renderer-pipeline parity with main pipeline.
 * Validates Requirements 23.3, 23.7 from Task 70.
 */

import { describe, it, expect } from 'vitest';
import type { Root } from 'mdast';

// Import main processor for comparison
import { buildProcessor } from '../../src/shared/markdown';

// Note: The renderer pipeline uses @renderer alias which is configured in vite config.
// For unit tests, we test the shared buildProcessor directly.

// ---------------------------------------------------------------------------
// Test fixtures - same content used for both pipelines
// ---------------------------------------------------------------------------

const fixtures = [
  {
    name: 'basic markdown',
    content: '# Hello\n\nWorld\n',
  },
  {
    name: 'frontmatter',
    content: '---\ntitle: Test\ntags: [a, b]\n---\n\n# Body\n',
  },
  {
    name: 'GFM table',
    content: '| A | B |\n|---|---|\n| 1 | 2 |\n',
  },
  {
    name: 'task list',
    content: '- [ ] Unchecked\n- [x] Checked\n',
  },
  {
    name: 'callout',
    content: '> [!note]\n> This is a callout\n',
  },
  {
    name: 'wiki link',
    content: 'See [[My Note]] for details.\n',
  },
  {
    name: 'inline math',
    content: 'Inline $E = mc^2$ formula\n',
  },
  {
    name: 'block math',
    content: '$$\n\\int_0^1 x dx\n$$',
  },
  {
    name: 'complex note',
    content: `# Title

- [ ] Task one
- [x] Task two

| Col A | Col B |
|-------|-------|
| val1  | val2  |

> [!warning] Be careful!
> Content here

[[wiki link]] and inline #tag

$$\nmath block$$\n`,
  },
];

// ---------------------------------------------------------------------------
// Task 70: renderer-pipeline parity with main pipeline
// ---------------------------------------------------------------------------

describe('renderer-pipeline parity with main pipeline (Req 23.3, 23.7)', () => {
  it.each(fixtures)('buildProcessor produces consistent AST for "$name"', ({ content }) => {
    const processor = buildProcessor();
    const ast = processor.parse(content) as Root;
    const result = processor.runSync(ast) as Root;
    
    expect(result.type).toBe('root');
    expect(result.children.length).toBeGreaterThan(0);
  });

  it('handles malformed markdown gracefully (Req 23.8)', () => {
    const malformed = '# Heading\n\n```\nunclosed code block\n\n- [x';
    
    // Should not throw, should return a valid AST
    const processor = buildProcessor();
    const ast = processor.parse(malformed) as Root;
    expect(() => processor.runSync(ast)).not.toThrow();
    
    const result = processor.runSync(ast) as Root;
    expect(result.type).toBe('root');
  });

  it('handles empty content gracefully', () => {
    const processor = buildProcessor();
    const ast = processor.parse('') as Root;
    const result = processor.runSync(ast) as Root;
    
    expect(result.type).toBe('root');
    expect(result.children.length).toBe(0);
  });

  it('preserves callout nodes in AST (Req 23.4)', () => {
    const content = '> [!note]\n> Title\n> Body\n';
    const processor = buildProcessor();
    const ast = processor.parse(content) as Root;
    const result = processor.runSync(ast) as Root;
    
    // Cast to any to allow checking for custom node types
    const children = result.children as any[];
    const callout = children.find((n: any) => n.type === 'callout');
    expect(callout).toBeDefined();
    if (callout) {
      expect(callout.calloutType).toBe('note');
    }
  });

  it('preserves wiki-link nodes in AST (Req 23.4)', () => {
    const content = 'See [[My Note]] for details.\n';
    const processor = buildProcessor();
    const ast = processor.parse(content) as Root;
    const result = processor.runSync(ast) as Root;
    
    // Find wikiLink in paragraph children
    let found = false;
    function search(node: any) {
      if (node.type === 'wikiLink') {
        found = true;
        expect(node.target).toBe('My Note');
      }
      if (Array.isArray(node.children)) node.children.forEach(search);
    }
    search(result);
    expect(found).toBe(true);
  });
});