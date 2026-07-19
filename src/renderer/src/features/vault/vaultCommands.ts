/**
 * vaultCommands.ts
 *
 * Renderer-side command module for vault/file-tree workflow orchestration.
 *
 * Phase 5.4 — Thin UI Enforcement (Architecture Goal 9).
 *
 * This module is the single owner of file-tree workflow orchestration on the
 * renderer. The `FileTree` component must NOT compute derived paths, sequence
 * multi-step IPC flows (rename → reload, delete, create folder → re-scan), or
 * perform validation — it invokes the functions here and dispatches the
 * resulting state updates.
 *
 * Each function performs exactly the same IPC calls and dispatches the same
 * actions the component previously performed, so runtime behavior is
 * unchanged. The only difference is placement: orchestration lives here,
 * not inside the presentation component.
 *
 * Responsibilities (business logic, not presentation):
 *   - rename a file (path computation + IPC + reload-if-current)
 *   - delete a file (IPC)
 *   - create a folder (validation + IPC + vault re-scan dispatch)
 *   - create a note (validation + IPC + re-scan + enter edit mode)
 */

import { Root } from 'mdast'
import { Template } from '@shared/types'
import { ipc } from '../../shared/ipc'
import type { AppAction } from '../../shared/store'

type Dispatch = (action: AppAction) => void

// ---------------------------------------------------------------------------
// Rename
// ---------------------------------------------------------------------------

/**
 * Rename a file. Computes the new path from the parent directory, calls the
 * rename IPC, and — if the renamed file was the active note — reloads it at the
 * new path via `FILE_LOADED`.
 */
export async function renameFile(
  oldPath: string,
  newName: string,
  currentFile: string | null,
  dispatch: Dispatch
): Promise<{ success: boolean; error: string | null }> {
  if (!newName.trim()) {
    return { success: false, error: 'Name cannot be empty.' }
  }
  const parts = oldPath.split('/')
  parts.pop()
  const parentDir = parts.join('/')
  const newPath = parentDir + '/' + newName.trim()
  try {
    const result = await ipc.note.rename(oldPath, newPath)
    if (!result.success) {
      return { success: false, error: result.error ?? 'Failed to rename.' }
    }
    if (currentFile === oldPath) {
      const fileAST = await ipc.file.get(newPath)
      dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
    }
    return { success: true, error: null }
  } catch (err) {
    return {
      success: false,
      error: err instanceof Error ? err.message : 'Unknown error.'
    }
  }
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

/** Delete a file via IPC. */
export async function deleteFile(filePath: string): Promise<{ success: boolean; error: string | null }> {
  try {
    const result = await ipc.note.delete(filePath)
    if (!result.success) {
      return { success: false, error: result.error ?? 'Failed to delete.' }
    }
    return { success: true, error: null }
  } catch (err) {
    return {
      success: false,
      error: err instanceof Error ? err.message : 'Unknown error.'
    }
  }
}

// ---------------------------------------------------------------------------
// Create folder
// ---------------------------------------------------------------------------

/**
 * Create a folder under the vault. Validates the (trimmed) name, calls the
 * folder-create IPC, then re-scans the vault and dispatches `VAULT_OPENED`
 * with the refreshed metadata.
 */
export async function createFolder(
  vaultPath: string,
  folderName: string,
  dispatch: Dispatch
): Promise<{ success: boolean; error: string | null }> {
  const trimmed = folderName.trim()
  if (!trimmed) {
    return { success: false, error: 'Folder name cannot be empty.' }
  }
  try {
    const fullPath = vaultPath + '/' + trimmed
    const result = await ipc.folder.create(fullPath)
    if (!result.success) {
      return { success: false, error: result.error ?? 'Failed to create folder.' }
    }
    const updatedVault = await ipc.vault.scan()
    dispatch({ type: 'VAULT_OPENED', payload: updatedVault })
    return { success: true, error: null }
  } catch (err) {
    return {
      success: false,
      error: err instanceof Error ? err.message : 'Unknown error.'
    }
  }
}

// ---------------------------------------------------------------------------
// Create note
// ---------------------------------------------------------------------------

/**
 * Create a note from a template (or empty). Validates the (trimmed) name,
 * calls the note-create IPC, re-scans the vault, then enters edit mode with the
 * new note's raw content and dispatches `FILE_LOADED` for it.
 */
export async function createNote(
  vaultPath: string,
  noteName: string,
  selectedTemplate: Template | null,
  dispatch: Dispatch
): Promise<{ success: boolean; error: string | null }> {
  const trimmed = noteName.trim()
  if (!trimmed) {
    return { success: false, error: 'Note name cannot be empty.' }
  }
  try {
    const result = await ipc.note.create(vaultPath, trimmed, selectedTemplate?.content)
    const updatedVault = await ipc.vault.scan()
    dispatch({ type: 'VAULT_OPENED', payload: updatedVault })
    const rawResult = await ipc.note.getRaw(result.path)
    dispatch({ type: 'EDIT_MODE_ENTER', payload: rawResult.content ?? '' })
    dispatch({ type: 'FILE_LOADED', payload: { path: result.path, ast: result.ast } })
    return { success: true, error: null }
  } catch (err) {
    return {
      success: false,
      error: err instanceof Error ? err.message : 'Unknown error.'
    }
  }
}

// ---------------------------------------------------------------------------
// Open a file from the tree
// ---------------------------------------------------------------------------

/**
 * Open a file selected in the tree. PDFs open in the PDF viewer pane; notes
 * load via IPC and dispatch `FILE_LOADED`.
 */
export async function openTreeFile(filePath: string, dispatch: Dispatch): Promise<void> {
  if (filePath.toLowerCase().endsWith('.pdf')) {
    dispatch({ type: 'PDF_OPENED', payload: { path: filePath } })
    return
  }
  const fileAST = await ipc.file.get(filePath)
  dispatch({ type: 'FILE_LOADED', payload: { path: fileAST.path, ast: fileAST.ast } })
}

// Re-export Root for callers that need the type.
export type { Root }
