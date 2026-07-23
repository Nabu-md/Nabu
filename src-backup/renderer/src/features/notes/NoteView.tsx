import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Root,
  Node,
  Parent,
  Heading,
  Paragraph,
  List,
  ListItem,
  Code,
  Table,
  TableRow,
  TableCell,
  Text,
  Strong,
  Emphasis,
  InlineCode,
  Delete,
  Link,
  Image,
  Blockquote,
  Html
} from 'mdast'
import {
  ToggleBlock as ToggleBlockNode,
  TaskList as TaskListNode,
  WikiLink as WikiLinkNode,
  Callout
} from '@shared/types'
import { useAppContext, type Tab } from '../../shared/store'
import { ToggleBlock } from './blocks/ToggleBlock'
import { TaskList } from './blocks/TaskList'
import { WikiLink } from './blocks/WikiLink'
import { CodeBlock } from './blocks/CodeBlock'
import { MermaidBlock } from './blocks/MermaidBlock'
import { EmbedBlock } from './blocks/EmbedBlock'
import { SandboxedHtml } from '../../shared/components/SandboxedHtml'
import { PropertiesView } from './blocks/PropertiesView'
import { KanbanBlock } from './blocks/KanbanBlock'
import { renderInlineTagText } from './blocks/InlineTagChip'
import { FavoriteToggle } from '../../shared/components/FavoriteToggle'
import { SaveIcon, DownloadIcon, EditIcon, EyeIcon } from '../../shared/components/icons'
import { MarkdownEditor } from './MarkdownEditor'
import katex from 'katex'
// parseMarkdown imported but used via IPC for Live Preview mode
// import { parseMarkdown } from './markdown/pipeline'
import {
  loadNoteFile as cmdLoadNoteFile,
  saveNote as cmdSaveNote,
  enterEditMode as cmdEnterEditMode,
  exitEditMode as cmdExitEditMode,
  exitLivePreviewMode as cmdExitLivePreviewMode,
  navigateToNote as cmdNavigateToNote,
  writeProperties as cmdWriteProperties,
  persistHeadingFold as cmdPersistHeadingFold,
  exportNoteHtml as cmdExportNoteHtml,
  retryLoadNote as cmdRetryLoadNote
} from './noteCommands'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Replace the YAML frontmatter section in raw markdown content.
 *
 * If the content starts with `---\n...\n---`, that section is replaced with
 * the new YAML string.  If no frontmatter exists, the new YAML is prepended.
 * Passing an empty YAML string removes the frontmatter section entirely.
 */
/** Note: replaceFrontmatter has moved to src/main/ipc.ts as replaceFrontmatterRaw */

// ---------------------------------------------------------------------------
// Skeleton placeholder
// ---------------------------------------------------------------------------

function NoteSkeleton(): React.JSX.Element {
  return (
    <div
      className="note-skeleton w-full px-8 py-6 animate-pulse"
      aria-busy="true"
      aria-label="Loading note…"
    >
      {/* Title */}
      <div className="h-7 w-2/3 rounded bg-white/10 mb-6" />
      {/* Paragraph lines */}
      <div className="space-y-3 mb-6">
        <div className="h-4 w-full rounded bg-white/8" />
        <div className="h-4 w-5/6 rounded bg-white/8" />
        <div className="h-4 w-4/6 rounded bg-white/8" />
      </div>
      {/* Sub-heading */}
      <div className="h-5 w-1/3 rounded bg-white/10 mb-4" />
      {/* More lines */}
      <div className="space-y-3 mb-6">
        <div className="h-4 w-full rounded bg-white/8" />
        <div className="h-4 w-3/4 rounded bg-white/8" />
        <div className="h-4 w-5/6 rounded bg-white/8" />
        <div className="h-4 w-2/3 rounded bg-white/8" />
      </div>
      {/* Code block */}
      <div className="h-20 w-full rounded bg-white/6 mb-6" />
      {/* More lines */}
      <div className="space-y-3">
        <div className="h-4 w-full rounded bg-white/8" />
        <div className="h-4 w-4/5 rounded bg-white/8" />
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Error state
// ---------------------------------------------------------------------------

interface NoteErrorProps {
  filePath: string
  message: string
  onRetry: () => void
}

function NoteError({ filePath, message, onRetry }: NoteErrorProps): React.JSX.Element {
  return (
    <div
      className="note-error flex flex-col items-center justify-center gap-4 px-8 py-16 text-center"
      role="alert"
      aria-live="assertive"
    >
      <div className="text-red-400 text-4xl" aria-hidden="true">
        ⚠
      </div>
      <div>
        <p className="text-sm font-semibold text-white/80 mb-1">Failed to load note</p>
        <p className="text-xs text-white/50 font-mono break-all mb-1">{filePath}</p>
        <p className="text-xs text-red-400/80">{message}</p>
      </div>
      <button
        type="button"
        onClick={onRetry}
        className="px-4 py-2 text-sm rounded bg-white/10 hover:bg-white/20 text-white/80 transition-colors"
      >
        Retry
      </button>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Empty state (no file selected)
// ---------------------------------------------------------------------------

function NoteEmpty(): React.JSX.Element {
  return (
    <div
      className="note-empty flex items-center justify-center h-full text-white/30 text-sm select-none"
      aria-label="No note selected"
    >
      Select a note to view
    </div>
  )
}

// ---------------------------------------------------------------------------
// Callout configuration — type → icon + colour
// ---------------------------------------------------------------------------

interface CalloutStyle {
  border: string
  bg: string
  text: string
  icon: string
}

const CALLOUT_CONFIG: Record<string, CalloutStyle> = {
  note: { border: 'border-l-blue-500', bg: 'bg-blue-950/20', text: 'text-blue-400', icon: 'ℹ️' },
  info: { border: 'border-l-sky-500', bg: 'bg-sky-950/20', text: 'text-sky-400', icon: 'ℹ️' },
  tip: {
    border: 'border-l-emerald-500',
    bg: 'bg-emerald-950/20',
    text: 'text-emerald-400',
    icon: '💡'
  },
  success: {
    border: 'border-l-green-500',
    bg: 'bg-green-950/20',
    text: 'text-green-400',
    icon: '✅'
  },
  warning: {
    border: 'border-l-amber-500',
    bg: 'bg-amber-950/20',
    text: 'text-amber-400',
    icon: '⚠️'
  },
  danger: { border: 'border-l-red-500', bg: 'bg-red-950/20', text: 'text-red-400', icon: '🔴' },
  error: { border: 'border-l-rose-500', bg: 'bg-rose-950/20', text: 'text-rose-400', icon: '✖️' },
  question: {
    border: 'border-l-violet-500',
    bg: 'bg-violet-950/20',
    text: 'text-violet-400',
    icon: '❓'
  },
  example: {
    border: 'border-l-purple-500',
    bg: 'bg-purple-950/20',
    text: 'text-purple-400',
    icon: '📋'
  },
  quote: { border: 'border-l-gray-500', bg: 'bg-white/5', text: 'text-gray-400', icon: '💬' },
  abstract: { border: 'border-l-teal-500', bg: 'bg-teal-950/20', text: 'text-teal-400', icon: '📄' }
}

// ---------------------------------------------------------------------------
// Recursive AST renderer
// ---------------------------------------------------------------------------

interface RenderContext {
  filePath: string
  optimisticToggles: Record<number, boolean>
  onToggle: (lineIndex: number) => void
  onNavigate: (filePath: string, blockRef?: string, pageRef?: number) => void
  vaultFiles: import('@shared/types').FileEntry[]
  embedDepth: number
  aliasIndex?: Map<string, string[]>
  // Collapsible heading state (Phase 2)
  headingFoldStates: Record<string, boolean>
  onHeadingToggle: (headingId: string) => void
}

/** Extract a block identifier from a node's data, if present. */
function blockIdFrom(node: Node): string | undefined {
  const data = (node as unknown as Record<string, unknown>).data as
    Record<string, unknown> | undefined
  return data?.blockId as string | undefined
}

function renderNode(node: Node, ctx: RenderContext, key: string | number): React.ReactNode {
  const type = node.type

  // ---- Custom node types ----

  if (type === 'toggleBlock') {
    const n = node as ToggleBlockNode
    return (
      <ToggleBlock
        key={key}
        node={n}
        filePath={ctx.filePath}
        renderNodes={(nodes, fp) =>
          nodes.map((child, i) => renderNode(child, { ...ctx, filePath: fp }, i))
        }
      />
    )
  }

  if (type === 'taskList') {
    const n = node as TaskListNode
    return (
      <TaskList
        key={key}
        node={n}
        optimisticToggles={ctx.optimisticToggles}
        onToggle={ctx.onToggle}
      />
    )
  }

  if (type === 'wikiLink') {
    const n = node as WikiLinkNode
    return (
      <WikiLink
        key={key}
        node={n}
        vaultFiles={ctx.vaultFiles}
        onNavigate={ctx.onNavigate}
        aliasIndex={ctx.aliasIndex}
      />
    )
  }

  if (type === 'callout') {
    const n = node as Callout
    const style = CALLOUT_CONFIG[n.calloutType] ?? CALLOUT_CONFIG.note
    const isCollapsible = n.toggle != null
    const expandedByDefault = n.toggle === '+'

    const header = (
      <div className={`flex items-center gap-2 text-sm font-semibold ${style.text} select-none`}>
        <span className="text-base leading-none">{style.icon}</span>
        {n.title && <span>{n.title}</span>}
      </div>
    )

    const body = n.children.map((child, i) => renderNode(child, ctx, `${key}-body-${i}`))

    if (isCollapsible) {
      return (
        <details
          key={key}
          open={expandedByDefault}
          className={`my-3 rounded-lg border-l-4 ${style.border} ${style.bg} ${style.text}`}
        >
          <summary className="cursor-pointer px-4 py-2 rounded-r-lg hover:bg-white/[0.03]">
            {header}
          </summary>
          <div className="px-4 pb-3 text-white/80 text-sm leading-relaxed">{body}</div>
        </details>
      )
    }

    return (
      <div key={key} className={`my-3 rounded-lg border-l-4 ${style.border} ${style.bg}`}>
        <div className="px-4 pt-2">{header}</div>
        <div className="px-4 pb-3 text-white/80 text-sm leading-relaxed">{body}</div>
      </div>
    )
  }

  if (type === 'embed') {
    const n = node as unknown as { target: string }
    return (
      <EmbedBlock
        key={key}
        target={n.target}
        embedDepth={ctx.embedDepth}
        renderNodes={(nodes, fp) =>
          nodes.map((child, i) =>
            renderNode(child, { ...ctx, embedDepth: ctx.embedDepth + 1, filePath: fp }, i)
          )
        }
      />
    )
  }

  // ---- Standard mdast types ----

  if (type === 'root') {
    const n = node as Root

    // Group children into sections for collapsible headings (Phase 2)
    const sections: Array<{ heading: Node | null; children: Node[] }> = []
    let currentSection: { heading: Node | null; children: Node[] } = { heading: null, children: [] }

    for (const child of n.children) {
      if (child.type === 'heading') {
        if (currentSection.children.length > 0 || currentSection.heading !== null) {
          sections.push(currentSection)
        }
        currentSection = { heading: child, children: [] }
      } else {
        currentSection.children.push(child)
      }
    }
    // Push the last section
    if (currentSection.heading !== null || currentSection.children.length > 0) {
      sections.push(currentSection)
    }

    return (
      <React.Fragment key={key}>
        {sections.map((section, i) => {
          if (section.heading === null) {
            // Content before first heading — render as-is
            return (
              <React.Fragment key={i}>
                {section.children.map((child, ci) => renderNode(child, ctx, ci))}
              </React.Fragment>
            )
          }

          const heading = section.heading as Heading
          const headingText = heading.children
            .filter((c): c is Text => c.type === 'text')
            .map((c) => c.value)
            .join('')
          const headingId = `${heading.depth}-${headingText
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, '-')
            .replace(/^-+|-+$/g, '')}-${i}`

          const isFolded = !ctx.headingFoldStates[headingId]

          return (
            <div key={i} className="heading-section" data-heading-id={headingId}>
              {renderNode(heading, ctx, i)}
              {isFolded ? null : (
                <div className="heading-section-content">
                  {section.children.map((child, ci) => renderNode(child, ctx, ci))}
                </div>
              )}
            </div>
          )
        })}
      </React.Fragment>
    )
  }

  if (type === 'heading') {
    const n = node as Heading
    const depth = n.depth
    const Tag = `h${depth}` as keyof React.JSX.IntrinsicElements
    const classMap: Record<number, string> = {
      1: 'text-2xl font-bold mt-6 mb-3 text-white/90',
      2: 'text-xl font-semibold mt-5 mb-2 text-white/85',
      3: 'text-lg font-semibold mt-4 mb-2 text-white/80',
      4: 'text-base font-semibold mt-3 mb-1 text-white/75',
      5: 'text-sm font-semibold mt-3 mb-1 text-white/70',
      6: 'text-xs font-semibold mt-2 mb-1 text-white/65'
    }
    const bid = blockIdFrom(node)

    // Generate heading ID for fold state tracking (Phase 2)
    const headingText = n.children
      .filter((c): c is Text => c.type === 'text')
      .map((c) => c.value)
      .join('')
    const headingId = `${depth}-${headingText
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')}-${key}`

    // Check if this heading is folded (default open)
    const isFolded = !ctx.headingFoldStates[headingId]

    return (
      <div
        key={key}
        className="heading-wrapper"
        data-heading-id={headingId}
        data-heading-depth={depth}
        data-block-id={bid}
      >
        <Tag
          id={`outline-heading-${key}`}
          className={`${classMap[depth] ?? 'font-semibold mt-3 mb-1'} flex items-center gap-1 cursor-pointer`}
          onClick={() => ctx.onHeadingToggle(headingId)}
        >
          <span
            className={`fold-indicator text-xs transition-transform ${isFolded ? '' : 'rotate-90'}`}
            aria-label={isFolded ? 'Expand section' : 'Collapse section'}
          >
            ▶
          </span>
          {(n as Parent).children.map((child, i) => renderNode(child, ctx, i))}
        </Tag>
      </div>
    )
  }

  if (type === 'paragraph') {
    const n = node as Paragraph
    const bid = blockIdFrom(node)

    // Check for %%kanban%% directive (Phase 6)
    const textContent = n.children
      .filter((c): c is Text => c.type === 'text')
      .map((c) => c.value)
      .join('')
    if (textContent.startsWith('%%kanban%%')) {
      const noteFolder = ctx.filePath ? ctx.filePath.split('/').slice(0, -1).join('/') : ''
      return <KanbanBlock key={key} folderPath={noteFolder} />
    }

    return (
      <p key={key} className="my-2 leading-relaxed text-white/75 text-sm" data-block-id={bid}>
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </p>
    )
  }

  if (type === 'list') {
    const n = node as List
    const Tag = n.ordered ? 'ol' : 'ul'
    const listClass = n.ordered
      ? 'list-decimal pl-6 my-2 space-y-1 text-sm text-white/75'
      : 'list-disc pl-6 my-2 space-y-1 text-sm text-white/75'
    return (
      <Tag key={key} className={listClass}>
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </Tag>
    )
  }

  if (type === 'listItem') {
    const n = node as ListItem
    const bid = blockIdFrom(node)
    return (
      <li key={key} className="leading-relaxed" data-block-id={bid}>
        {(n as Parent).children.map((child, i) => renderNode(child, ctx, i))}
      </li>
    )
  }

  if (type === 'code') {
    const n = node as Code
    const bid = blockIdFrom(node)
    // Route mermaid diagrams to the dedicated MermaidBlock
    if (n.lang === 'mermaid') {
      return (
        <div key={key} data-block-id={bid}>
          <MermaidBlock value={n.value} />
        </div>
      )
    }
    return (
      <div key={key} data-block-id={bid}>
        <CodeBlock node={n} />
      </div>
    )
  }

  if (type === 'table') {
    const n = node as Table
    const bid = blockIdFrom(node)
    const [headerRow, ...bodyRows] = n.children as TableRow[]
    return (
      <div key={key} className="overflow-x-auto my-4" data-block-id={bid}>
        <table className="min-w-full text-sm text-white/75 border-collapse">
          <thead>
            <tr>
              {(headerRow?.children ?? []).map((cell: TableCell, i: number) => (
                <th
                  key={i}
                  className="border border-white/15 px-3 py-1.5 text-left font-semibold text-white/85 bg-white/5"
                  align={n.align?.[i] ?? undefined}
                >
                  {(cell as Parent).children.map((child, ci) => renderNode(child, ctx, ci))}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {bodyRows.map((row: TableRow, ri: number) => (
              <tr key={ri} className="odd:bg-white/[0.02]">
                {row.children.map((cell: TableCell, ci: number) => (
                  <td
                    key={ci}
                    className="border border-white/10 px-3 py-1.5"
                    align={n.align?.[ci] ?? undefined}
                  >
                    {(cell as Parent).children.map((child, k) => renderNode(child, ctx, k))}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )
  }

  if (type === 'blockquote') {
    const n = node as Blockquote
    const bid = blockIdFrom(node)
    return (
      <blockquote
        key={key}
        className="border-l-2 border-white/25 pl-4 my-3 text-white/55 italic text-sm"
        data-block-id={bid}
      >
        {(n as Parent).children.map((child, i) => renderNode(child, ctx, i))}
      </blockquote>
    )
  }

  if (type === 'thematicBreak') {
    return <hr key={key} className="border-white/15 my-6" />
  }

  if (type === 'html') {
    const n = node as Html
    // Render HTML content inside a sandboxed iframe with allow-scripts only.
    // See SandboxedHtml.tsx for the full security model.
    return <SandboxedHtml key={key} html={n.value} />
  }

  // ---- Inline nodes ----

  if (type === 'text') {
    return renderInlineTagText(node as Text)
  }

  if (type === 'strong') {
    const n = node as Strong
    return (
      <strong key={key} className="font-semibold text-white/90">
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </strong>
    )
  }

  if (type === 'emphasis') {
    const n = node as Emphasis
    return (
      <em key={key} className="italic">
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </em>
    )
  }

  if (type === 'inlineCode') {
    const n = node as InlineCode
    return (
      <code key={key} className="font-mono text-xs bg-white/10 rounded px-1 py-0.5 text-white/80">
        {n.value}
      </code>
    )
  }

  // ---- Math (KaTeX) ----

  if (type === 'inlineMath') {
    const n = node as unknown as { value: string }
    try {
      const html = katex.renderToString(n.value, { throwOnError: false })
      return (
        <span key={key} className="math math-inline" dangerouslySetInnerHTML={{ __html: html }} />
      )
    } catch {
      return (
        <span key={key} className="text-red-400/80 text-sm italic">
          ${n.value}$
        </span>
      )
    }
  }

  if (type === 'math') {
    const n = node as unknown as { value: string; meta?: string | null }
    try {
      const html = katex.renderToString(n.value, { throwOnError: false, displayMode: true })
      return (
        <div
          key={key}
          className="math math-block my-4 overflow-x-auto text-center"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      )
    } catch {
      return (
        <pre
          key={key}
          className="text-red-400/80 text-sm p-3 rounded bg-white/5 my-3 overflow-x-auto font-mono"
        >
          {'$$\n'}
          {n.value}
          {'\n$$'}
        </pre>
      )
    }
  }

  if (type === 'delete') {
    const n = node as Delete
    return (
      <del key={key} className="opacity-50 line-through">
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </del>
    )
  }

  if (type === 'link') {
    const n = node as Link
    return (
      <a
        key={key}
        href={n.url}
        title={n.title ?? undefined}
        className="text-blue-400 hover:underline"
        target="_blank"
        rel="noreferrer noopener"
      >
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </a>
    )
  }

  if (type === 'image') {
    const n = node as Image
    return (
      <img
        key={key}
        src={n.url}
        alt={n.alt ?? ''}
        title={n.title ?? undefined}
        className="max-w-full rounded my-2"
      />
    )
  }

  if (type === 'break') {
    return <br key={key} />
  }

  // ---- YAML frontmatter (remark-frontmatter) — handled by PropertiesView ----
  if (type === 'yaml' || type === 'toml') {
    return null
  }

  // ---- Unrecognized node types: render raw text content as plain paragraph ----
  const unknown = node as unknown as Record<string, unknown>
  const rawText =
    typeof unknown['value'] === 'string'
      ? unknown['value']
      : typeof unknown['alt'] === 'string'
        ? unknown['alt']
        : null

  if (rawText !== null) {
    return (
      <p key={key} className="my-2 text-sm text-white/50">
        {rawText}
      </p>
    )
  }

  // Has children — recurse
  if ('children' in unknown && Array.isArray(unknown['children'])) {
    return (
      <React.Fragment key={key}>
        {(unknown['children'] as Node[]).map((child, i) => renderNode(child, ctx, i))}
      </React.Fragment>
    )
  }

  return null
}

// ---------------------------------------------------------------------------
// OutgoingLinksPanel
// ---------------------------------------------------------------------------

function OutgoingLinksPanel(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const [isExpanded, setIsExpanded] = useState(true)

  const outgoingLinks = useMemo(() => {
    if (!state.currentFile) return []
    // Deduplicate by target (Req 6.2).
    const seen = new Set<string>()
    return state.graphEdges
      .filter((e) => e.source === state.currentFile)
      .filter((e) => {
        if (seen.has(e.target)) return false
        seen.add(e.target)
        return true
      })
      .map((e) => ({
        targetPath: e.target,
        name:
          state.vault?.files.find((f) => f.path === e.target)?.name ??
          e.target.split('/').pop()?.replace('.md', '') ??
          e.target,
        snippet: e.snippet
      }))
  }, [state.currentFile, state.graphEdges, state.vault])

  if (outgoingLinks.length === 0) return null

  return (
    <section
      className="outgoing-links-panel mt-8 border-t border-white/10 pt-4"
      aria-label="Outgoing links"
    >
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded((prev) => !prev)}
        className="flex items-center gap-2 text-sm font-semibold text-white/60 hover:text-white/80 transition-colors w-full text-left mb-2"
      >
        <span>Outgoing links ({outgoingLinks.length})</span>
        <span aria-hidden="true" className="text-xs">
          {isExpanded ? '▲' : '▼'}
        </span>
      </button>
      {isExpanded && (
        <ul role="list" className="space-y-1">
          {outgoingLinks.map((ol) => {
            const isBroken = !state.vault?.files.some((f) => f.path === ol.targetPath)
            return (
              <li key={ol.targetPath}>
                <button
                  type="button"
                  disabled={isBroken}
                  onClick={() => {
                    if (isBroken) return
                    cmdNavigateToNote(ol.targetPath, dispatch).catch(console.error)
                  }}
                  className={`w-full text-left px-3 py-2 rounded transition-colors group ${
                    isBroken ? 'cursor-not-allowed opacity-50' : 'hover:bg-white/8 cursor-pointer'
                  }`}
                >
                  <span
                    className={`block font-semibold text-sm ${
                      isBroken ? 'text-red-400/60' : 'text-white/80 group-hover:text-white/95'
                    }`}
                  >
                    {ol.name}
                    {isBroken && (
                      <span className="ml-2 text-xs text-red-400/50" title="Target note not found">
                        (broken)
                      </span>
                    )}
                  </span>
                  {ol.snippet && !isBroken && (
                    <span className="block text-xs text-white/40 mt-0.5 truncate">
                      {ol.snippet}
                    </span>
                  )}
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

// ---------------------------------------------------------------------------
// BacklinksPanel
// ---------------------------------------------------------------------------

function BacklinksPanel(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const [isExpanded, setIsExpanded] = useState(true)

  const backlinks = useMemo(() => {
    if (!state.currentFile) return []
    return state.graphEdges
      .filter((e) => e.target === state.currentFile)
      .map((e) => ({
        sourcePath: e.source,
        name:
          state.vault?.files.find((f) => f.path === e.source)?.name ??
          e.source.split('/').pop()?.replace('.md', '') ??
          e.source,
        snippet: e.snippet
      }))
  }, [state.currentFile, state.graphEdges, state.vault])

  if (backlinks.length === 0) return null

  return (
    <section className="backlinks-panel mt-8 border-t border-white/10 pt-4" aria-label="Backlinks">
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded((prev) => !prev)}
        className="flex items-center gap-2 text-sm font-semibold text-white/60 hover:text-white/80 transition-colors w-full text-left mb-2"
      >
        <span>Backlinks ({backlinks.length})</span>
        <span aria-hidden="true" className="text-xs">
          {isExpanded ? '▲' : '▼'}
        </span>
      </button>
      {isExpanded && (
        <ul role="list" className="space-y-1">
          {backlinks.map((bl) => (
            <li key={bl.sourcePath}>
              <button
                type="button"
                onClick={() => {
                  cmdNavigateToNote(bl.sourcePath, dispatch).catch(console.error)
                }}
                className="w-full text-left px-3 py-2 rounded hover:bg-white/8 transition-colors group"
              >
                <span className="block font-semibold text-sm text-white/80 group-hover:text-white/95">
                  {bl.name}
                </span>
                {bl.snippet && (
                  <span className="block text-xs text-white/40 mt-0.5 truncate">{bl.snippet}</span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

// ---------------------------------------------------------------------------
// NoteViewForTab - renders note content for a specific tab (for split-pane views)
// ---------------------------------------------------------------------------

/**
 * Props for NoteViewForTab - used by PaneLayout to render per-tab content.
 * This is a simplified version that only renders in view mode.
 */
export interface NoteViewForTabProps {
  tab: Tab
}

/**
 * NoteViewForTab - renders the content of a specific tab.
 * Used by PaneLayout for non-active tabs in split views.
 * Only supports view mode; edit/live-preview modes are handled by the active tab.
 */
export function NoteViewForTab({ tab }: NoteViewForTabProps): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const { vault } = state

  // If the tab has no AST, show a loading state
  if (tab.ast === null) {
    return (
      <div className="note-view flex-1 overflow-y-auto h-full" aria-label="Note view">
        <NoteSkeleton />
      </div>
    )
  }

  // Build render context for this tab
  const renderCtx: RenderContext = {
    filePath: tab.path,
    optimisticToggles: {},
    onToggle: () => {
      // No-op for non-active tabs - task toggles only work on active tab
    },
    onNavigate: (filePath: string, blockRef?: string, pageRef?: number) => {
      // Navigate to the clicked note (activates the tab)
      cmdNavigateToNote(filePath, dispatch, { blockRef, pageRef }).catch(console.error)
    },
    vaultFiles: vault?.files ?? [],
    embedDepth: 0,
    aliasIndex: state.extendedIndex?.aliasIndex,
    headingFoldStates: {},
    onHeadingToggle: () => {
      // No-op for non-active tabs
    }
  }

  // Render the note content
  return (
    <div className="note-view flex-1 overflow-y-auto h-full" aria-label="Note view">
      <article
        className="note-content max-w-2xl mx-auto px-8 py-6"
        aria-label="Note content"
      >
        {/* YAML frontmatter → PropertiesView */}
        {(() => {
          const yamlNode = tab.ast!.children.find(
            (c) => c.type === 'yaml' || (c as unknown as { type: string }).type === 'toml'
          ) as { value?: string } | undefined
          const yamlValue = yamlNode?.value ?? null
          return (
            <PropertiesView
              key="properties"
              yamlValue={yamlValue}
              onSave={async () => {
                // No-op for non-active tabs
              }}
              onPropertySearch={(propertyName, propertyValue) => {
                dispatch({
                  type: 'SEARCH_PANEL_OPEN_WITH_QUERY',
                  payload: `property:${propertyName}:${propertyValue}`
                })
              }}
            />
          )
        })()}
        {tab.ast!.children
          .filter(
            (child) =>
              child.type !== 'yaml' && (child as unknown as { type: string }).type !== 'toml'
          )
          .map((child, i) => renderNode(child, renderCtx, i))}
      </article>
    </div>
  )
}

// ---------------------------------------------------------------------------
// NoteView
// ---------------------------------------------------------------------------

export function NoteView(): React.JSX.Element {
  const { state, dispatch } = useAppContext()
  const { currentFile, currentAST } = state

  // Loading / error state
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Optimistic toggle state: lineIndex → overridden checked value
  const [optimisticToggles, setOptimisticToggles] = useState<Record<number, boolean>>({})

  // Block reference navigation: stored while waiting for the target AST to render
  const [pendingBlockRef, setPendingBlockRef] = useState<string | null>(null)

  // Edit mode local state
  const [editContent, setEditContent] = useState('')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [saveError, setSaveError] = useState<string | null>(null)
  const [editDirty, setEditDirty] = useState(false)
  // useRef avoids stale closures inside the textarea onChange debounce
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Live Preview mode state (Req 23.4, 23.5)
  const [livePreviewContent, setLivePreviewContent] = useState('')
  const livePreviewTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  // Ref to the current filePath so IPC callbacks can reference the latest value
  const currentFileRef = useRef<string | null>(currentFile)
  useEffect(() => {
    currentFileRef.current = currentFile
  }, [currentFile])

  // ---- Load AST on file change ----
  useEffect(() => {
    if (!currentFile) {
      setIsLoading(false)
      setError(null)
      setPendingBlockRef(null)
      return
    }

    // If the AST is already loaded for this file, skip the IPC fetch
    if (currentAST !== null) {
      setIsLoading(false)
      setError(null)
      setOptimisticToggles({})
      return
    }

    let cancelled = false
    setIsLoading(true)
    setError(null)
    setOptimisticToggles({})

    cmdLoadNoteFile(currentFile, dispatch)
      .then(() => {
        if (cancelled) return
        setIsLoading(false)
      })
      .catch((err: unknown) => {
        if (cancelled) return
        const message = err instanceof Error ? err.message : 'An unknown error occurred'
        setError(message)
        setIsLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [currentFile, dispatch]) // eslint-disable-line react-hooks/exhaustive-deps

  // ---- Listen for external note:updated IPC messages ----
  useEffect(() => {
    const cleanup = window.electron.on.noteUpdated(({ path, ast, isExternal }) => {
      if (isExternal && path === currentFileRef.current) {
        // Clear optimistic state when an external edit arrives for the current file
        setOptimisticToggles({})
        dispatch({ type: 'AST_UPDATED', payload: { path, ast, isExternal } })
      }
    })
    return cleanup
  }, [dispatch])

  // ---- Scroll to block reference after AST render ----
  useEffect(() => {
    if (!pendingBlockRef || !currentAST) return

    // Use requestAnimationFrame to wait for the DOM to update after the AST render
    const raf = requestAnimationFrame(() => {
      const el = document.querySelector(`[data-block-id="${CSS.escape(pendingBlockRef)}"]`)
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'center' })
        // Brief highlight
        el.classList.add('ring-2', 'ring-yellow-500/40', 'rounded')
        setTimeout(() => {
          el.classList.remove('ring-2', 'ring-yellow-500/40', 'rounded')
        }, 2000)
      }
      setPendingBlockRef(null)
    })

    return () => cancelAnimationFrame(raf)
  }, [pendingBlockRef, currentAST])

  // ---- Initialise editContent when entering edit mode ----
  useEffect(() => {
    if (state.editMode && state.currentRaw !== null) {
      setEditContent(state.currentRaw)
      setEditDirty(false)
      setSaveStatus('idle')
      setSaveError(null)
    }
  }, [state.editMode]) // eslint-disable-line react-hooks/exhaustive-deps

  // ---- saveNote ----
  const saveNote = useCallback(async () => {
    if (!currentFile) return
    setSaveStatus('saving')
    const result = await cmdSaveNote(currentFile, editContent)
    if (result.success) {
      setSaveStatus('saved')
      setEditDirty(false)
      // Clear "saved" indicator after 2s
      setTimeout(() => setSaveStatus('idle'), 2000)
    } else {
      setSaveStatus('error')
      setSaveError(result.error ?? 'Save failed')
    }
  }, [currentFile, editContent])

  // ---- Find/Replace state ----
  const [showFindReplace, setShowFindReplace] = useState(false)

  // ---- Keyboard shortcuts: Cmd+E, Cmd+S, Cmd+H ----
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent): void => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'e') {
        e.preventDefault()
        if (!currentFile) return
        if (state.editMode) {
          cmdExitEditMode(currentFile, dispatch).catch(console.error)
        } else {
          cmdEnterEditMode(currentFile, dispatch).catch(console.error)
        }
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        if (state.editMode && currentFile) {
          e.preventDefault()
          cmdSaveNote(currentFile, editContent).catch(console.error)
        }
      }
      // Cmd+H / Ctrl+H for find/replace (Phase 0b)
      if ((e.metaKey || e.ctrlKey) && e.key === 'h') {
        e.preventDefault()
        if (state.editMode || state.livePreviewMode) {
          setShowFindReplace((prev) => !prev)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [state.editMode, state.livePreviewMode, currentFile, editContent, dispatch])

  // ---- Cleanup auto-save timer on unmount ----
  useEffect(() => {
    return () => {
      if (autoSaveTimer.current !== null) {
        clearTimeout(autoSaveTimer.current)
      }
    }
  }, [])

  // ---- Task toggle handler ----
  const handleTaskToggle = useCallback(
    (lineIndex: number) => {
      if (!currentFile) return

      // 1. Update local optimistic state immediately
      setOptimisticToggles((prev) => ({
        ...prev,
        [lineIndex]: !prev[lineIndex]
      }))

      // 2. Send IPC message
      window.electron.task.toggle(currentFile, lineIndex).catch(() => {
        // 3. Revert on failure
        setOptimisticToggles((prev) => ({
          ...prev,
          [lineIndex]: !prev[lineIndex]
        }))
      })
    },
    [currentFile]
  )

  // ---- Wiki link navigation handler ----
  // WikiLink now resolves internally and passes the resolved absolute file path.
  const handleNavigate = useCallback(
    (filePath: string, blockRef?: string, pageRef?: number) => {
      cmdNavigateToNote(filePath, dispatch, { blockRef, pageRef })
        .then(() => {
          if (blockRef) {
            // After the AST is dispatched, the next render will trigger the
            // scroll-to-block effect in the useEffect below.
            setPendingBlockRef(blockRef)
          }
        })
        .catch(console.error)
    },
    [dispatch]
  )

  // ---- Properties save handler ----
  const handlePropertiesSave = useCallback(
    async (newYaml: string) => {
      if (!currentFile) return
      await cmdWriteProperties(currentFile, newYaml)
    },
    [currentFile]
  )

  // ---- Property search handler (Req 13.5) ----
  const handlePropertySearch = useCallback(
    (propertyName: string, propertyValue: string) => {
      dispatch({
        type: 'SEARCH_PANEL_OPEN_WITH_QUERY',
        payload: `property:${propertyName}:${propertyValue}`
      })
    },
    [dispatch]
  )

  // ---- Article ref for HTML export ----
  const articleRef = useRef<HTMLElement>(null)

  // ---- HTML export handler ----
  const handleExportHtml = useCallback(async () => {
    if (!currentFile) return
    await cmdExportNoteHtml(currentFile, articleRef.current)
  }, [currentFile])

  // ---- Retry handler ----
  const handleRetry = useCallback(() => {
    if (!currentFile) return
    setError(null)
    setIsLoading(true)

    cmdRetryLoadNote(currentFile, dispatch)
      .then(() => {
        setIsLoading(false)
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : 'An unknown error occurred'
        setError(message)
        setIsLoading(false)
      })
  }, [currentFile, dispatch])

  // ---- Heading fold state management (Phase 2) ----
  const [headingFoldStates, setHeadingFoldStates] = useState<Record<string, boolean>>({})

  const handleHeadingToggle = useCallback(
    async (headingId: string) => {
      if (!currentFile || !state.vault) return
      const newState = !headingFoldStates[headingId]
      setHeadingFoldStates((prev) => ({ ...prev, [headingId]: newState }))
      // Persist to main process
      await cmdPersistHeadingFold(state.vault.path, currentFile, headingId, newState)
    },
    [currentFile, headingFoldStates, state.vault]
  )

  // ---- Render context ----
  const renderCtx: RenderContext = {
    filePath: currentFile ?? '',
    optimisticToggles,
    onToggle: handleTaskToggle,
    onNavigate: handleNavigate,
    vaultFiles: state.vault?.files ?? [],
    embedDepth: 0,
    aliasIndex: state.extendedIndex?.aliasIndex,
    headingFoldStates,
    onHeadingToggle: handleHeadingToggle
  }

  // ---- Render ----
  return (
    <div className="note-view flex-1 overflow-y-auto h-full" aria-label="Note view">
      {/* No file selected */}
      {!currentFile && <NoteEmpty />}

      {/* Loading skeleton */}
      {currentFile && isLoading && <NoteSkeleton />}

      {/* Error state */}
      {currentFile && !isLoading && error !== null && (
        <NoteError filePath={currentFile} message={error} onRetry={handleRetry} />
      )}

      {/* Edit mode UI */}
      {currentFile && state.editMode && (
        <div className="edit-mode flex flex-col h-full px-8 py-6">
          {/* toolbar */}
          <div className="flex items-center justify-between mb-3">
            <button
              type="button"
              aria-label="Switch to view mode"
              onClick={() => cmdExitEditMode(currentFile, dispatch).catch(console.error)}
              className="note-toolbar__btn"
              title="Switch to view mode"
            >
              <EyeIcon size={16} />
            </button>
            <div className="flex items-center gap-2">
              {saveStatus === 'saving' && <span className="text-xs text-white/50">Saving…</span>}
              {saveStatus === 'saved' && <span className="text-xs text-white/50">Auto-saved</span>}
              {saveStatus === 'error' && <span className="text-xs text-red-400">{saveError}</span>}
              <button
                type="button"
                aria-label="Save note"
                disabled={saveStatus === 'saving'}
                onClick={() => saveNote()}
                className="note-toolbar__btn"
                title="Save note"
              >
                <SaveIcon size={16} />
              </button>
            </div>
          </div>
          {/* CodeMirror editor */}
          <MarkdownEditor
            value={editContent}
            onChange={(val) => {
              setEditContent(val)
              setEditDirty(true)
              // Reset auto-save debounce
              if (autoSaveTimer.current !== null) clearTimeout(autoSaveTimer.current)
              autoSaveTimer.current = setTimeout(() => {
                if (editDirty) saveNote()
              }, 1000)
            }}
            showFindReplace={showFindReplace}
            onToggleFindReplace={setShowFindReplace}
          />
        </div>
      )}

      {/* Live Preview mode UI (Req 23.4, 23.5) */}
      {currentFile && state.livePreviewMode && (
        <div className="live-preview-mode flex flex-col h-full px-8 py-6">
          {/* toolbar */}
          <div className="flex items-center justify-between mb-3">
            <button
              type="button"
              aria-label="Switch to view mode"
              onClick={() => cmdExitLivePreviewMode(currentFile, livePreviewContent, dispatch).catch(console.error)}
              className="note-toolbar__btn"
              title="Switch to view mode"
            >
              <EyeIcon size={16} />
            </button>
            <div className="flex items-center gap-2">
              <button
                type="button"
                aria-label="Save note"
                onClick={() =>
                  cmdSaveNote(currentFile, livePreviewContent).catch(console.error)
                }
                className="note-toolbar__btn"
                title="Save note"
              >
                <SaveIcon size={16} />
              </button>
            </div>
          </div>
          {/* CodeMirror editor in Live Preview mode */}
          <MarkdownEditor
            value={livePreviewContent}
            onChange={(val) => {
              setLivePreviewContent(val)
              // Debounced re-parse for Live Preview (Req 23.4)
              if (livePreviewTimer.current !== null) clearTimeout(livePreviewTimer.current)
            }}
            showFindReplace={showFindReplace}
            onToggleFindReplace={setShowFindReplace}
          />
        </div>
      )}

      {/* Rendered note content (view mode) */}
      {currentFile &&
        !state.editMode &&
        !state.livePreviewMode &&
        !isLoading &&
        error === null &&
        currentAST !== null && (
          <>
            {/* View/edit toolbar */}
            <div className="flex items-center justify-end gap-1 px-8 pt-4">
              {currentFile && <FavoriteToggle filePath={currentFile} size="md" />}
              <button
                type="button"
                aria-label="Export as PDF"
                aria-disabled={!currentFile}
                disabled={!currentFile}
                onClick={() => window.print()}
                className="note-toolbar__btn"
                title="Export as PDF"
              >
                <DownloadIcon size={16} />
              </button>
              <button
                type="button"
                aria-label="Export as HTML"
                aria-disabled={!currentFile}
                disabled={!currentFile}
                onClick={handleExportHtml}
                className="note-toolbar__btn"
                title="Export as HTML"
              >
                <DownloadIcon size={16} />
              </button>
              <div className="note-toolbar__divider" />
              <button
                type="button"
                aria-label="Switch to edit mode"
                onClick={() => cmdEnterEditMode(currentFile, dispatch).catch(console.error)}
                className="note-toolbar__btn"
                title="Edit note"
              >
                <EditIcon size={16} />
              </button>
            </div>
            <article
              ref={articleRef}
              className="note-content max-w-2xl mx-auto px-8 py-6"
              aria-label="Note content"
            >
              {/* YAML frontmatter → PropertiesView */}
              {(() => {
                const yamlNode = currentAST.children.find(
                  (c) => c.type === 'yaml' || (c as unknown as { type: string }).type === 'toml'
                ) as { value?: string } | undefined
                const yamlValue = yamlNode?.value ?? null
                return (
                  <PropertiesView
                    key="properties"
                    yamlValue={yamlValue}
                    onSave={handlePropertiesSave}
                    onPropertySearch={handlePropertySearch}
                  />
                )
              })()}
              {currentAST.children
                .filter(
                  (child) =>
                    child.type !== 'yaml' && (child as unknown as { type: string }).type !== 'toml'
                )
                .map((child, i) => renderNode(child, renderCtx, i))}
              <OutgoingLinksPanel />
              <BacklinksPanel />
            </article>
          </>
        )}
    </div>
  )
}
