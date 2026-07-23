/**
 * noteCommands.ts
 *
 * Renderer-side command module for note-related workflow orchestration.
 *
 * Phase 5.4 — Thin UI Enforcement (Architecture Goal 9).
 *
 * This module is the single owner of note workflow orchestration on the
 * renderer. Components (e.g. `NoteView`) must NOT coordinate IPC calls,
 * compute derived paths, or sequence multi-step flows themselves — they
 * invoke the functions here and dispatch the resulting state updates.
 *
 * Each function performs exactly the same IPC calls and dispatches the same
 * actions the component previously performed, so runtime behavior is
 * unchanged. The only difference is placement: orchestration lives here,
 * not inside the presentation component.
 *
 * Responsibilities (business logic, not presentation):
 *   - file load (with timeout) + dispatch
 *   - note save + status reporting
 *   - edit-mode enter/exit sequencing
 *   - live-preview exit (save-then-exit)
 *   - wiki-link / file navigation + dispatch
 *   - outgoing/backlink navigation
 *   - HTML export (DOM serialization + IPC)
 *   - properties write
 *   - heading fold persistence
 */

import { Root } from 'mdast'
import { ipc } from "../../shared/ipc"'
import type { AppAction } from '../../shared/store'

// ---------------------------------------------------------------------------
// Timeout helper (shared renderer IPC timeout)
// ---------------------------------------------------------------------------

const IPC_TIMEOUT_MS = 3000

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
// Dispatch helper type
// ---------------------------------------------------------------------------

type Dispatch = (action: AppAction) => void

// ---------------------------------------------------------------------------
// File loading
// ---------------------------------------------------------------------------

/**
 * Load a note's AST from the main process and dispatch `FILE_LOADED`.
 * Returns the loaded AST path, or throws on failure (caller handles UI state).
 */
export async function loadNoteFile(
  filePath: string,
  dispatch: Dispatch
): Promise<{ path: string; ast: Root }> {
  const fileAST = await withTimeout(ipc.file.get(filePath), IPC_TIMEOUT_MS)
  dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
  return fileAST
}

// ---------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------

/** Persist note content via IPC. Returns success/error for UI status. */
export async function saveNote(filePath: string, content: string): Promise<{ success: boolean; error: string | null }> {
  try {
    const result = await ipc.note.save(filePath, content)
    if (result.success) {
      return { success: true, error: null }
    }
    return { success: false, error: result.error ?? 'Save failed' }
  } catch (err) {
    return {
      success: false,
      error: err instanceof Error ? err.message : 'Save failed'
    }
  }
}

// ---------------------------------------------------------------------------
// Edit-mode transitions
// ---------------------------------------------------------------------------

/** Enter edit mode: fetch raw content, then dispatch `EDIT_MODE_ENTER`. */
export async function enterEditMode(filePath: string, dispatch: Dispatch): Promise<void> {
  const result = await ipc.note.getRaw(filePath)
  dispatch({ type: 'EDIT_MODE_ENTER', payload: result.content ?? '' })
}

/** Exit edit mode: reload AST, then dispatch `EDIT_MODE_EXIT`. */
export async function exitEditMode(filePath: string, dispatch: Dispatch): Promise<void> {
  const fileAST = await ipc.file.get(filePath)
  dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
  dispatch({ type: 'EDIT_MODE_EXIT' })
}

/** Exit live-preview mode: save current content, then dispatch `LIVE_PREVIEW_MODE_EXIT`. */
export async function exitLivePreviewMode(
  filePath: string,
  content: string,
  dispatch: Dispatch
): Promise<void> {
  try {
    const result = await ipc.note.save(filePath, content)
    if (!result.success) {
      console.error('[noteCommands] Live Preview save error:', result.error)
    }
  } catch (err) {
    console.error('[noteCommands] Live Preview save error:', err)
  }
  dispatch({ type: 'LIVE_PREVIEW_MODE_EXIT' })
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

/**
 * Navigate to a file (or PDF) by absolute path. PDFs open in the PDF viewer
 * pane; notes load via IPC and dispatch `FILE_LOADED`.
 */
export async function navigateToNote(
  filePath: string,
  dispatch: Dispatch,
  options?: { blockRef?: string; pageRef?: number }
): Promise<void> {
  if (filePath.toLowerCase().endsWith('.pdf')) {
    dispatch({ type: 'PDF_OPENED', payload: { path: filePath, page: options?.pageRef } })
    return
  }
  const fileAST = await ipc.file.get(filePath)
  dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/** Write YAML frontmatter for a note via IPC. */
export async function writeProperties(filePath: string, yaml: string): Promise<void> {
  try {
    const result = await ipc.properties.write(filePath, yaml)
    if (!result.success) {
      console.error('[noteCommands] properties write error:', result.error)
    }
  } catch (err) {
    console.error('[noteCommands] properties save error:', err)
  }
}

// ---------------------------------------------------------------------------
// Heading fold persistence
// ---------------------------------------------------------------------------

/** Persist a heading's collapsed/expanded state to the main process. */
export async function persistHeadingFold(
  vaultPath: string,
  filePath: string,
  headingId: string,
  isOpen: boolean
): Promise<void> {
  try {
    await ipc.viewState?.setFold(vaultPath, filePath, headingId, isOpen)
  } catch {
    // viewState API may not be available yet
  }
}

// ---------------------------------------------------------------------------
// HTML export
// ---------------------------------------------------------------------------

/**
 * Serialize the rendered note article to a standalone HTML document and export
 * it via IPC. The DOM serialization (reading CSS variables, building the
 * wrapper) is presentation-adjacent orchestration owned here so the component
 * only supplies the article element.
 */
export async function exportNoteHtml(filePath: string, articleEl: HTMLElement | null): Promise<void> {
  const noteHtml = articleEl?.outerHTML ?? ''
  const getVar = (v: string): string =>
    getComputedStyle(document.documentElement).getPropertyValue(v).trim() || ''
  const html = `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>${filePath.split('/').pop()?.replace(/\.md$/i, '') ?? 'Note'}</title>
<style>
body { background: ${getVar('--nabu-bg') || '#0a0a0a'}; color: ${getVar('--nabu-text') || '#e5e5e5'}; font-family: -apple-system, BlinkMacSystemFont, sans-serif; max-width: 720px; margin: 0 auto; padding: 2rem; line-height: 1.6; }
h1,h2,h3,h4,h5,h6 { color: ${getVar('--nabu-text') || '#e5e5e5'}; }
a { color: ${getVar('--nabu-accent') || '#60a5fa'}; }
code { background: rgba(255,255,255,0.1); padding: 2px 4px; border-radius: 3px; font-family: monospace; }
pre { background: rgba(255,255,255,0.05); padding: 1rem; border-radius: 6px; overflow-x: auto; }
blockquote { border-left: 3px solid ${getVar('--nabu-border') || '#2a2a2a'}; padding-left: 1rem; opacity: 0.7; }
</style>
</head>
<body>${noteHtml}</body>
</html>`
  try {
    const result = await ipc.note.exportHtml(filePath, html)
    if (!result.success && result.error) {
      console.error('[noteCommands] HTML export failed:', result.error)
    }
  } catch (err) {
    console.error('[noteCommands] HTML export error:', err)
  }
}

// ---------------------------------------------------------------------------
// Retry (force reload of a note)
// ---------------------------------------------------------------------------

/**
 * Force a reload of a note by clearing the AST and re-fetching. Used by the
 * error-state retry button. Returns the loaded AST or throws on failure.
 */
export async function retryLoadNote(
  filePath: string,
  dispatch: Dispatch
): Promise<{ path: string; ast: Root }> {
  dispatch({ type: 'FILE_LOADED', payload: { path: filePath, ast: null as unknown as Root } })
  return loadNoteFile(filePath, dispatch)
}
