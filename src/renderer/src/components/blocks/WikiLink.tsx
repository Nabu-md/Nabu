import React, { useMemo, useState } from 'react'
import { WikiLink as WikiLinkNode, FileEntry } from '@shared/types'

// ---------------------------------------------------------------------------
// Resolution types
// ---------------------------------------------------------------------------

interface ResolvedLink {
  /** The single best-match file path (shortest path wins when multiple match). */
  path: string
  /** All matching file paths (for tooltip when ambiguous). */
  matches: string[]
}

// ---------------------------------------------------------------------------
// Resolution logic
// ---------------------------------------------------------------------------

/**
 * Build a lookup index from lowercase basename (without `.md`) → list of absolute
 * paths.  Optionally incorporates aliases from the alias index so that wiki-link
 * targets pointing to aliases resolve to the owning note (Req 15.2).
 *
 * This runs in O(n) on vault size and is memoised by the component so it is
 * effectively O(1) per lookup after the first render with a given `vaultFiles`
 * reference.
 */
function buildIndex(
  vaultFiles: FileEntry[],
  aliasIndex?: Map<string, string[]>
): Map<string, string[]> {
  const index = new Map<string, string[]>()

  // 1. Index by file name
  for (const entry of vaultFiles) {
    const key = entry.name.replace(/\.md$/i, '').toLowerCase()
    const existing = index.get(key)
    if (existing) {
      existing.push(entry.path)
    } else {
      index.set(key, [entry.path])
    }
  }

  // 2. Merge aliases into the same index (Req 15.2)
  if (aliasIndex) {
    for (const [alias, paths] of aliasIndex) {
      const existing = index.get(alias)
      if (existing) {
        // Append any paths not already present
        for (const p of paths) {
          if (!existing.includes(p)) existing.push(p)
        }
      } else {
        index.set(alias, [...paths])
      }
    }
  }

  return index
}

/**
 * Resolve a wiki-link target to a file path (or `null` if not found).
 *
 * Resolution rules (from design.md):
 *  1. Strip `.md` extension from target before lookup.
 *  2. Lowercase both target and index key.
 *  3. If multiple matches, select the one with the shortest path (closest to
 *     vault root).  All matches are returned so the caller can show a tooltip.
 */
function resolveWikiLink(target: string, index: Map<string, string[]>): ResolvedLink | null {
  const key = target.replace(/\.md$/i, '').toLowerCase()
  const matches = index.get(key)
  if (!matches || matches.length === 0) return null

  // Sort ascending by path length; shortest = closest to vault root
  const sorted = [...matches].sort((a, b) => a.length - b.length)
  return { path: sorted[0], matches: sorted }
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface WikiLinkProps {
  node: WikiLinkNode
  vaultFiles: FileEntry[]
  onNavigate: (filePath: string, blockRef?: string, pageRef?: number) => void
  /** Optional alias index for resolving wiki-link targets that match aliases (Req 15.2). */
  aliasIndex?: Map<string, string[]>
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function WikiLink({
  node,
  vaultFiles,
  onNavigate,
  aliasIndex
}: WikiLinkProps): React.JSX.Element {
  const [tooltipVisible, setTooltipVisible] = useState(false)

  // Build the index once per unique `vaultFiles` reference (aliasIndex is stable across renders)
  const index = useMemo(() => buildIndex(vaultFiles, aliasIndex), [vaultFiles, aliasIndex])

  const resolved = resolveWikiLink(node.target, index)

  // Compute display suffix for block reference links
  const displaySuffix = node.blockRef
    ? `#^${node.blockRef}`
    : node.pageRef
      ? `#page=${node.pageRef}`
      : ''

  // ---- Broken / unresolved link ----
  if (!resolved) {
    return (
      <span
        className="wiki-link broken inline-flex items-center gap-0.5 text-white/40 cursor-default"
        title={`"${node.target}" not found`}
        aria-label={`Broken wiki link: ${node.target} not found`}
      >
        <span aria-hidden="true">⚠</span> [[{node.target}
        {displaySuffix}]]
      </span>
    )
  }

  const isAmbiguous = resolved.matches.length > 1

  const handleClick = (e: React.MouseEvent): void => {
    e.preventDefault()
    onNavigate(resolved.path, node.blockRef, node.pageRef)
  }

  const linkClasses =
    'wiki-link relative inline-flex items-center gap-0.5 cursor-pointer ' +
    'text-[#8B5CF6] underline underline-offset-2 hover:text-[#A78BFA] ' +
    'transition-colors duration-150'

  // ---- Resolved — single match ----
  if (!isAmbiguous) {
    return (
      <a
        role="link"
        tabIndex={0}
        className={linkClasses}
        onClick={handleClick}
        onKeyDown={(e) => e.key === 'Enter' && handleClick(e as unknown as React.MouseEvent)}
        title={resolved.path}
        aria-label={`Wiki link: ${node.target}`}
      >
        [[{node.target}
        {displaySuffix}]]
      </a>
    )
  }

  // ---- Resolved — multiple matches (ambiguous) ----
  return (
    <span
      className="wiki-link-wrapper relative inline-flex items-center"
      onMouseEnter={() => setTooltipVisible(true)}
      onMouseLeave={() => setTooltipVisible(false)}
      onFocus={() => setTooltipVisible(true)}
      onBlur={() => setTooltipVisible(false)}
    >
      <a
        role="link"
        tabIndex={0}
        className={
          linkClasses + ' after:ml-0.5 after:text-[#8B5CF6]/60 after:text-xs after:content-["↕"]'
        }
        onClick={handleClick}
        onKeyDown={(e) => e.key === 'Enter' && handleClick(e as unknown as React.MouseEvent)}
        aria-label={`Wiki link: ${node.target}${displaySuffix} (${resolved.matches.length} matches)`}
        aria-describedby={tooltipVisible ? `wl-tooltip-${node.target}` : undefined}
      >
        [[{node.target}
        {displaySuffix}]]
      </a>

      {/* CSS tooltip listing all matching paths */}
      {tooltipVisible && (
        <div
          id={`wl-tooltip-${node.target}`}
          role="tooltip"
          className={
            'absolute z-50 bottom-full left-0 mb-1.5 min-w-max max-w-xs ' +
            'rounded-md border border-white/10 bg-[#1E1E2E] px-3 py-2 ' +
            'shadow-lg text-xs text-white/80 space-y-1'
          }
        >
          <p className="font-semibold text-white/50 mb-1.5 uppercase tracking-wide text-[10px]">
            Multiple matches
          </p>
          {resolved.matches.map((m) => (
            <div
              key={m}
              className={m === resolved.path ? 'font-medium text-[#A78BFA]' : 'text-white/70'}
            >
              {m === resolved.path ? '▶ ' : '\u00a0\u00a0'}
              {m}
            </div>
          ))}
        </div>
      )}
    </span>
  )
}
