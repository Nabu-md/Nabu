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
import { ToggleBlock as ToggleBlockNode, TaskList as TaskListNode, WikiLink as WikiLinkNode } from '@shared/types'
import { useAppContext } from '../App'
import { ToggleBlock } from './blocks/ToggleBlock'
import { TaskList } from './blocks/TaskList'
import { WikiLink } from './blocks/WikiLink'
import { CodeBlock } from './blocks/CodeBlock'

// ---------------------------------------------------------------------------
// Timeout constant
// ---------------------------------------------------------------------------

const IPC_TIMEOUT_MS = 3000

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Returns a promise that rejects after `ms` milliseconds. */
function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Request timed out')), ms)
    promise.then(
      (val) => {
        clearTimeout(timer)
        resolve(val)
      },
      (err) => {
        clearTimeout(timer)
        reject(err)
      }
    )
  })
}

// ---------------------------------------------------------------------------
// Skeleton placeholder
// ---------------------------------------------------------------------------

function NoteSkeleton(): React.JSX.Element {
  return (
    <div className="note-skeleton w-full px-8 py-6 animate-pulse" aria-busy="true" aria-label="Loading note…">
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
      <div className="text-red-400 text-4xl" aria-hidden="true">⚠</div>
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
// Recursive AST renderer
// ---------------------------------------------------------------------------

interface RenderContext {
  filePath: string
  optimisticToggles: Record<number, boolean>
  onToggle: (lineIndex: number) => void
  onNavigate: (filePath: string) => void
  vaultFiles: import('@shared/types').FileEntry[]
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
          nodes.map((child, i) =>
            renderNode(child, { ...ctx, filePath: fp }, i)
          )
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
      />
    )
  }

  // ---- Standard mdast types ----

  if (type === 'root') {
    const n = node as Root
    return (
      <React.Fragment key={key}>
        {n.children.map((child, i) => renderNode(child, ctx, i))}
      </React.Fragment>
    )
  }

  if (type === 'heading') {
    const n = node as Heading
    const depth = n.depth
    const Tag = (`h${depth}`) as keyof React.JSX.IntrinsicElements
    const classMap: Record<number, string> = {
      1: 'text-2xl font-bold mt-6 mb-3 text-white/90',
      2: 'text-xl font-semibold mt-5 mb-2 text-white/85',
      3: 'text-lg font-semibold mt-4 mb-2 text-white/80',
      4: 'text-base font-semibold mt-3 mb-1 text-white/75',
      5: 'text-sm font-semibold mt-3 mb-1 text-white/70',
      6: 'text-xs font-semibold mt-2 mb-1 text-white/65'
    }
    return (
      <Tag key={key} className={classMap[depth] ?? 'font-semibold mt-3 mb-1'}>
        {(n as Parent).children.map((child, i) => renderNode(child, ctx, i))}
      </Tag>
    )
  }

  if (type === 'paragraph') {
    const n = node as Paragraph
    return (
      <p key={key} className="my-2 leading-relaxed text-white/75 text-sm">
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
    return (
      <li key={key} className="leading-relaxed">
        {(n as Parent).children.map((child, i) => renderNode(child, ctx, i))}
      </li>
    )
  }

  if (type === 'code') {
    const n = node as Code
    return <CodeBlock key={key} node={n} />
  }

  if (type === 'table') {
    const n = node as Table
    const [headerRow, ...bodyRows] = n.children as TableRow[]
    return (
      <div key={key} className="overflow-x-auto my-4">
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
    return (
      <blockquote
        key={key}
        className="border-l-2 border-white/25 pl-4 my-3 text-white/55 italic text-sm"
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
    // Render raw HTML as sanitized text to avoid injection
    return (
      <pre key={key} className="text-xs text-white/40 font-mono my-2 whitespace-pre-wrap">
        {n.value}
      </pre>
    )
  }

  // ---- Inline nodes ----

  if (type === 'text') {
    return (node as Text).value
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
      <code
        key={key}
        className="font-mono text-xs bg-white/10 rounded px-1 py-0.5 text-white/80"
      >
        {n.value}
      </code>
    )
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

  // ---- YAML frontmatter (remark-frontmatter) — skip silently ----
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
// BacklinksPanel
// ---------------------------------------------------------------------------

function BacklinksPanel(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const [isExpanded, setIsExpanded] = useState(true)

  const backlinks = useMemo(() => {
    if (!state.currentFile) return []
    return state.graphEdges
      .filter(e => e.target === state.currentFile)
      .map(e => ({
        sourcePath: e.source,
        name: state.vault?.files.find(f => f.path === e.source)?.name ?? e.source.split('/').pop()?.replace('.md', '') ?? e.source,
        snippet: e.snippet
      }))
  }, [state.currentFile, state.graphEdges, state.vault])

  if (backlinks.length === 0) return null

  return (
    <section className="backlinks-panel mt-8 border-t border-white/10 pt-4" aria-label="Backlinks">
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded(prev => !prev)}
        className="flex items-center gap-2 text-sm font-semibold text-white/60 hover:text-white/80 transition-colors w-full text-left mb-2"
      >
        <span>Backlinks ({backlinks.length})</span>
        <span aria-hidden="true" className="text-xs">{isExpanded ? '▲' : '▼'}</span>
      </button>
      {isExpanded && (
        <ul role="list" className="space-y-1">
          {backlinks.map((bl) => (
            <li key={bl.sourcePath}>
              <button
                type="button"
                onClick={() => {
                  window.electron.file.get(bl.sourcePath).then((fileAST) => {
                    dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
                  }).catch(console.error)
                }}
                className="w-full text-left px-3 py-2 rounded hover:bg-white/8 transition-colors group"
              >
                <span className="block font-semibold text-sm text-white/80 group-hover:text-white/95">
                  {bl.name}
                </span>
                {bl.snippet && (
                  <span className="block text-xs text-white/40 mt-0.5 truncate">
                    {bl.snippet}
                  </span>
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

  // Edit mode local state
  const [editContent, setEditContent] = useState('')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [saveError, setSaveError] = useState<string | null>(null)
  const [editDirty, setEditDirty] = useState(false)
  // useRef avoids stale closures inside the textarea onChange debounce
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

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

    withTimeout(window.electron.file.get(currentFile), IPC_TIMEOUT_MS)
      .then((fileAST) => {
        if (cancelled) return
        dispatch({
          type: 'FILE_LOADED',
          payload: { path: fileAST.path, ast: fileAST.ast }
        })
        setIsLoading(false)
      })
      .catch((err: unknown) => {
        if (cancelled) return
        const message =
          err instanceof Error ? err.message : 'An unknown error occurred'
        setError(message)
        setIsLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [currentFile]) // eslint-disable-line react-hooks/exhaustive-deps

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
    try {
      const result = await window.electron.note.save(currentFile, editContent)
      if (result.success) {
        setSaveStatus('saved')
        setEditDirty(false)
        // Clear "saved" indicator after 2s
        setTimeout(() => setSaveStatus('idle'), 2000)
      } else {
        setSaveStatus('error')
        setSaveError(result.error ?? 'Save failed')
      }
    } catch (err) {
      setSaveStatus('error')
      setSaveError(err instanceof Error ? err.message : 'Save failed')
    }
  }, [currentFile, editContent])

  // ---- Toggle edit mode ----
  const enterEditMode = useCallback(async () => {
    if (!currentFile) return
    try {
      const result = await window.electron.note.getRaw(currentFile)
      dispatch({ type: 'EDIT_MODE_ENTER', payload: result.content ?? '' })
    } catch (err) {
      console.error('[NoteView] getRaw error:', err)
    }
  }, [currentFile, dispatch])

  const exitEditMode = useCallback(async () => {
    if (!currentFile) return
    // Clear auto-save timer
    if (autoSaveTimer.current !== null) {
      clearTimeout(autoSaveTimer.current)
      autoSaveTimer.current = null
    }
    try {
      const fileAST = await window.electron.file.get(currentFile)
      dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
    } catch (err) {
      console.error('[NoteView] file.get on exit error:', err)
    }
    dispatch({ type: 'EDIT_MODE_EXIT' })
  }, [currentFile, dispatch])

  // ---- Keyboard shortcuts: Cmd+E and Cmd+S ----
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent): void => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'e') {
        e.preventDefault()
        if (state.editMode) {
          exitEditMode().catch(console.error)
        } else {
          enterEditMode().catch(console.error)
        }
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        if (state.editMode) {
          e.preventDefault()
          saveNote().catch(console.error)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [state.editMode, enterEditMode, exitEditMode, saveNote])

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
    (filePath: string) => {
      window.electron.file.get(filePath).then((fileAST) => {
        dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
      }).catch(console.error)
    },
    [dispatch]
  )

  // ---- Article ref for HTML export ----
  const articleRef = useRef<HTMLElement>(null)

  // ---- HTML export handler ----
  const handleExportHtml = useCallback(async () => {
    if (!currentFile) return
    const noteHtml = articleRef.current?.outerHTML ?? ''
    const getVar = (v: string): string =>
      getComputedStyle(document.documentElement).getPropertyValue(v).trim() || ''
    const html = `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>${currentFile.split('/').pop()?.replace(/\.md$/i, '') ?? 'Note'}</title>
<style>
body { background: ${getVar('--onyx-bg') || '#0a0a0a'}; color: ${getVar('--onyx-text') || '#e5e5e5'}; font-family: -apple-system, BlinkMacSystemFont, sans-serif; max-width: 720px; margin: 0 auto; padding: 2rem; line-height: 1.6; }
h1,h2,h3,h4,h5,h6 { color: ${getVar('--onyx-text') || '#e5e5e5'}; }
a { color: ${getVar('--onyx-accent') || '#60a5fa'}; }
code { background: rgba(255,255,255,0.1); padding: 2px 4px; border-radius: 3px; font-family: monospace; }
pre { background: rgba(255,255,255,0.05); padding: 1rem; border-radius: 6px; overflow-x: auto; }
blockquote { border-left: 3px solid ${getVar('--onyx-border') || '#2a2a2a'}; padding-left: 1rem; opacity: 0.7; }
</style>
</head>
<body>${noteHtml}</body>
</html>`
    try {
      const result = await window.electron.note.exportHtml(currentFile, html)
      if (!result.success && result.error) {
        console.error('[NoteView] HTML export failed:', result.error)
      }
    } catch (err) {
      console.error('[NoteView] HTML export error:', err)
    }
  }, [currentFile])

  // ---- Retry handler ----
  const handleRetry = useCallback(() => {
    if (!currentFile) return
    // Force a reload by clearing the AST and triggering the effect again
    dispatch({ type: 'FILE_LOADED', payload: { path: currentFile, ast: null as unknown as Root } })
    setError(null)
    setIsLoading(true)

    withTimeout(window.electron.file.get(currentFile), IPC_TIMEOUT_MS)
      .then((fileAST) => {
        dispatch({
          type: 'FILE_LOADED',
          payload: { path: fileAST.path, ast: fileAST.ast }
        })
        setIsLoading(false)
      })
      .catch((err: unknown) => {
        const message =
          err instanceof Error ? err.message : 'An unknown error occurred'
        setError(message)
        setIsLoading(false)
      })
  }, [currentFile, dispatch])

  // ---- Render context ----
  const renderCtx: RenderContext = {
    filePath: currentFile ?? '',
    optimisticToggles,
    onToggle: handleTaskToggle,
    onNavigate: handleNavigate,
    vaultFiles: state.vault?.files ?? []
  }

  // ---- Render ----
  return (
    <div
      className="note-view flex-1 overflow-y-auto h-full"
      aria-label="Note view"
    >
      {/* No file selected */}
      {!currentFile && <NoteEmpty />}

      {/* Loading skeleton */}
      {currentFile && isLoading && <NoteSkeleton />}

      {/* Error state */}
      {currentFile && !isLoading && error !== null && (
        <NoteError
          filePath={currentFile}
          message={error}
          onRetry={handleRetry}
        />
      )}

      {/* Edit mode UI */}
      {currentFile && state.editMode && (
        <div className="edit-mode flex flex-col h-full px-8 py-6">
          {/* toolbar */}
          <div className="flex items-center justify-between mb-3">
            <button
              type="button"
              aria-label="Switch to view mode"
              onClick={() => exitEditMode().catch(console.error)}
              className="px-3 py-1 text-sm rounded bg-white/10 hover:bg-white/20 text-white/70 transition-colors"
            >
              View Mode
            </button>
            <div className="flex items-center gap-2">
              {saveStatus === 'saving' && (
                <span className="text-xs text-white/50">Saving…</span>
              )}
              {saveStatus === 'saved' && (
                <span className="text-xs text-white/50">Auto-saved</span>
              )}
              {saveStatus === 'error' && (
                <span className="text-xs text-red-400">{saveError}</span>
              )}
              <button
                type="button"
                aria-label="Save note"
                disabled={saveStatus === 'saving'}
                onClick={() => saveNote().catch(console.error)}
                className="px-3 py-1 text-sm rounded bg-white/10 hover:bg-white/20 text-white/70 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Save
              </button>
            </div>
          </div>
          {/* textarea */}
          <textarea
            aria-label="Edit note"
            value={editContent}
            onChange={(e) => {
              setEditContent(e.target.value)
              setEditDirty(true)
              // Reset auto-save debounce
              if (autoSaveTimer.current !== null) clearTimeout(autoSaveTimer.current)
              autoSaveTimer.current = setTimeout(() => {
                if (editDirty) saveNote().catch(console.error)
              }, 1000)
            }}
            className="flex-1 w-full resize-none bg-transparent text-onyx-text text-sm font-mono focus:outline-none border border-onyx-border rounded p-3"
          />
        </div>
      )}

      {/* Rendered note content (view mode) */}
      {currentFile && !state.editMode && !isLoading && error === null && currentAST !== null && (
        <>
          {/* View/edit toolbar */}
          <div className="flex items-center justify-end gap-2 px-8 pt-4">
            <button
              type="button"
              aria-label="Export as PDF"
              aria-disabled={!currentFile}
              disabled={!currentFile}
              onClick={() => window.print()}
              className="px-3 py-1 text-xs rounded bg-white/8 hover:bg-white/15 text-white/50 hover:text-white/70 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              PDF
            </button>
            <button
              type="button"
              aria-label="Export as HTML"
              aria-disabled={!currentFile}
              disabled={!currentFile}
              onClick={handleExportHtml}
              className="px-3 py-1 text-xs rounded bg-white/8 hover:bg-white/15 text-white/50 hover:text-white/70 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              HTML
            </button>
            <button
              type="button"
              aria-label="Switch to edit mode"
              onClick={() => enterEditMode().catch(console.error)}
              className="px-3 py-1 text-xs rounded bg-white/8 hover:bg-white/15 text-white/50 hover:text-white/70 transition-colors"
            >
              Edit
            </button>
          </div>
          <article
            ref={articleRef}
            className="note-content max-w-2xl mx-auto px-8 py-6"
            aria-label="Note content"
          >
            {currentAST.children.map((child, i) => renderNode(child, renderCtx, i))}
            <BacklinksPanel />
          </article>
        </>
      )}
    </div>
  )
}
