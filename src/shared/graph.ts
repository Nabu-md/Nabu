/**
 * graph.ts — pure graph-building utilities
 *
 * Builds a list of directed edges from wiki-link relationships found in a set
 * of markdown files.  All AST access is provided via callback so the function
 * performs no file I/O and is fully deterministic / testable.
 */

import { visit } from 'unist-util-visit';
import type { Node } from 'unist';
import type { Root } from 'mdast';
import type { FileEntry, WikiLink, Edge } from './types';

/**
 * Build the directed-edge list for the knowledge graph.
 *
 * @param files       - Full list of vault files (used for name → path resolution).
 * @param getAST      - Callback that returns the parsed AST for a given file path,
 *                      or `undefined` if the file has not been parsed yet.
 * @param aliasIndex  - Optional alias index (lowercase alias → file paths).
 *                      When provided, wiki-link targets that don't match a file
 *                      name are resolved against aliases (Req 15.2).
 * @returns           Array of unique, resolvable edges.  Unresolvable wiki-links
 *                    (targets that don't match any vault file or alias) are
 *                    silently skipped.  The `snippet` field is left empty (`''`)
 *                    and is populated later by `buildIndexes()`.
 */
export function buildGraph(
  files: FileEntry[],
  getAST: (path: string) => Root | undefined,
  aliasIndex?: Map<string, string[]>
): Edge[] {
  const edges: Edge[] = [];

  for (const file of files) {
    const ast = getAST(file.path);
    if (ast === undefined) continue;

    visit(ast as Node, (node: Node) => {
      if (node.type !== 'wikiLink') return;

      const wikiLink = node as unknown as WikiLink;
      const target = wikiLink.target;

      // Case-insensitive basename comparison — resolve target name to a FileEntry
      let resolvedFile: FileEntry | undefined = files.find(
        (f) => f.name.toLowerCase() === target.toLowerCase()
      );

      // Fall back to alias resolution if no file name matches (Req 15.2)
      if (resolvedFile === undefined && aliasIndex) {
        const aliasPaths = aliasIndex.get(target.toLowerCase());
        if (aliasPaths && aliasPaths.length > 0) {
          // Shortest-path tie-break (closest to vault root wins)
          const sorted = [...aliasPaths].sort((a, b) => a.length - b.length);
          resolvedFile = files.find((f) => f.path === sorted[0]);
        }
      }

      if (resolvedFile === undefined) return; // skip unresolvable links

      edges.push({
        source: file.path,
        target: resolvedFile.path,
        snippet: '', // filled later by buildIndexes()
      });
    });
  }

  return edges;
}

// Re-export Edge from ./types for consumers who import only from this module
export type { Edge } from './types';
