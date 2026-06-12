/**
 * indexing.ts
 *
 * Pure index-building utilities for full-text search and tag-based filtering.
 * Both functions are side-effect free: they receive file metadata and an AST
 * accessor callback, and return plain Maps without performing any I/O.
 *
 * Requirements: 7.1, 8.1
 */

import { visit, SKIP } from 'unist-util-visit';
import type { VisitorResult } from 'unist-util-visit';
import type { Root, Yaml } from 'mdast';
import type { FileEntry } from './types';

// ---------------------------------------------------------------------------
// Full-text index
// ---------------------------------------------------------------------------

/**
 * Build an inverted full-text index from the given files.
 *
 * For each file the AST is retrieved via `getAST`; if `undefined` is returned
 * the file is silently skipped.  The walker visits every node but skips the
 * *children* of `yaml` and `toml` front-matter nodes so that frontmatter
 * content is not included in the full-text index (tags are handled separately
 * by `buildTagIndex`).
 *
 * Tokens are produced by lower-casing the node value and splitting on
 * whitespace and Unicode punctuation; empty strings are discarded.
 *
 * Returns `Map<word, Set<filePath>>`.
 *
 * Requirements: 7.1
 */
export function buildFullTextIndex(
  files: FileEntry[],
  getAST: (path: string) => Root | undefined,
): Map<string, Set<string>> {
  const ftIndex = new Map<string, Set<string>>();

  for (const file of files) {
    const root = getAST(file.path);
    if (root === undefined) continue;

    visit(root, (node): VisitorResult => {
      // For yaml/toml nodes: don't descend into children (frontmatter is
      // excluded from the full-text index; tags are handled by buildTagIndex)
      const nodeType = (node as { type: string }).type;
      if (nodeType === 'yaml' || nodeType === 'toml') {
        return SKIP;
      }

      // Extract textual value from content-bearing leaf nodes
      if (node.type === 'text' || node.type === 'inlineCode' || node.type === 'code') {
        const value = (node as { value: string }).value;
        if (!value) return;

        const words = value
          .toLowerCase()
          .split(/[\s\p{P}]+/u)
          .filter((w) => w.length > 0);

        for (const word of words) {
          let paths = ftIndex.get(word);
          if (paths === undefined) {
            paths = new Set<string>();
            ftIndex.set(word, paths);
          }
          paths.add(file.path);
        }
      }
    });
  }

  return ftIndex;
}

// ---------------------------------------------------------------------------
// Tag index
// ---------------------------------------------------------------------------

/**
 * Build a tag-to-files index from YAML frontmatter.
 *
 * For each file the function locates the first `yaml` node in the AST and
 * extracts the `tags:` field.  Both inline-array syntax
 * (`tags: [t1, t2]`) and block-list syntax (`tags:\n  - t1`) are supported.
 *
 * Tags are trimmed of surrounding whitespace; empty strings are discarded.
 *
 * Returns `Map<tag, Set<filePath>>`.
 *
 * Requirements: 8.1
 */
export function buildTagIndex(
  files: FileEntry[],
  getAST: (path: string) => Root | undefined,
): Map<string, Set<string>> {
  const tagIndex = new Map<string, Set<string>>();

  for (const file of files) {
    const root = getAST(file.path);
    if (root === undefined) continue;

    // Find the first yaml frontmatter node
    let yamlNode: Yaml | undefined;
    visit(root, 'yaml', (node: Yaml) => {
      yamlNode = node;
      return SKIP; // stop after the first one
    });

    if (yamlNode === undefined) continue;

    const tags = extractTagsFromYaml(yamlNode.value);

    for (const tag of tags) {
      let paths = tagIndex.get(tag);
      if (paths === undefined) {
        paths = new Set<string>();
        tagIndex.set(tag, paths);
      }
      paths.add(file.path);
    }
  }

  return tagIndex;
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Extract the list of tags from a raw YAML frontmatter string.
 *
 * Supports two formats:
 *   - Inline array : `tags: [t1, t2, t3]`
 *   - Block list   : `tags:\n  - t1\n  - t2`
 *
 * Returns a deduplicated array of non-empty trimmed tag strings.
 */
function extractTagsFromYaml(yaml: string): string[] {
  const tags: string[] = [];

  // Try to find the tags line
  const lines = yaml.split('\n');
  let tagsLineIndex = -1;

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trimStart();
    if (/^tags\s*:/i.test(trimmed)) {
      tagsLineIndex = i;
      break;
    }
  }

  if (tagsLineIndex === -1) return tags;

  const tagsLine = lines[tagsLineIndex];

  // Check for inline array format: tags: [t1, t2]
  const inlineMatch = tagsLine.match(/^[^:]*:\s*\[([^\]]*)\]/);
  if (inlineMatch) {
    const items = inlineMatch[1].split(',');
    for (const item of items) {
      const tag = item.trim();
      if (tag.length > 0) tags.push(tag);
    }
    return tags;
  }

  // Check if there's a value after the colon (non-array scalar — skip block mode check)
  const afterColon = tagsLine.replace(/^[^:]*:/, '').trim();
  if (afterColon.length > 0 && !afterColon.startsWith('-')) {
    // Single scalar value on same line (not a standard tags format, skip)
    return tags;
  }

  // Block list format: subsequent lines starting with `  - `
  for (let i = tagsLineIndex + 1; i < lines.length; i++) {
    const line = lines[i];
    // A block list item has leading whitespace followed by `- `
    const blockItemMatch = line.match(/^\s+-\s+(.*)/);
    if (blockItemMatch) {
      const tag = blockItemMatch[1].trim();
      if (tag.length > 0) tags.push(tag);
    } else {
      // Non-list line encountered — block list is finished
      // Only stop if the line is non-empty and not a continuation
      const trimmedLine = line.trim();
      if (trimmedLine.length > 0 && !trimmedLine.startsWith('#')) {
        break;
      }
    }
  }

  return tags;
}
