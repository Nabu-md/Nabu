/**
 * store.ts — Renderer application state store (Phase 5.3 ownership extraction)
 *
 * This module is the single owner of the renderer's shared state infrastructure:
 *   - state shape (`AppState`) and supporting types (`Tab`, `PDFTab`, `Workspace`,
 *     `TabGroup`, `PaneLayout`, `OpenVault`, `GraphMode`)
 *   - action vocabulary (`AppAction`)
 *   - the reducer (`appReducer`) — the ONLY writer of shared state
 *   - the derivation helper (`syncActiveAliases`)
 *   - the React context (`AppContext`) and its hook (`useAppContext`)
 *
 * Extracted from `App.tsx` during Phase 5.3 so that feature components depend on
 * a focused state module rather than the root composition module (which also
 * contains IPC wiring and layout). This reduces coupling: components no longer
 * transitively import the entire root `App` module. Runtime behavior is unchanged.
 */

import React, { createContext, useContext } from 'react'
import { Root } from 'mdast'
import { VaultMetadata, SearchResult, Edge } from '@shared/types'
import type { ExtendedSearchIndex } from '@shared/extended-indexing'
import type { SearchQueryResult } from '@shared/search-query'

// ---------------------------------------------------------------------------
// State shape
// ---------------------------------------------------------------------------

export interface OpenVault {
  id: string // vaultId
  path: string
  name: string
}

/** Tab represents an open note in the tab system (Req 24.1) */
export interface Tab {
  id: string // unique tab ID (UUID)
  path: string // absolute file path
  ast: Root | null
  raw: string | null
  mode: 'view' | 'edit' | 'live-preview'
  scrollTop: number
  cursor: number
}

/** PDF tab type for PDF viewer (Req 40.1) */
export interface PDFTab {
  id: string
  path: string
  mode: 'pdf'
  pdfData: {
    currentPage: number
    scale: number
  }
  scrollTop: number
  cursor: number
}

// ---------------------------------------------------------------------------
// Workspace types (Req 25.1-25.5)
// ---------------------------------------------------------------------------

export interface Workspace {
  id: string
  name: string
  openTabs: string[] // array of file paths
  paneLayout: PaneLayout
}

// ---------------------------------------------------------------------------
// Tab groups types (Req 24.9) - Chrome-style folder grouping
// ---------------------------------------------------------------------------

export type TabGroupColor =
  | 'blue'
  | 'red'
  | 'green'
  | 'yellow'
  | 'purple'
  | 'orange'
  | 'cyan'
  | 'pink'

export interface TabGroup {
  id: string // unique group ID
  folderPath: string // relative folder path (e.g., "projects/", "research/papers/")
  color: TabGroupColor
  isCollapsed: boolean
  tabIds: string[] // ordered tab IDs in this group
}

export type PaneLayout = 'single' | 'split-horizontal' | 'split-vertical' | 'grid'

export interface AppState {
  openVaults: OpenVault[] // all open vaults (multi-vault)
  activeVaultId: string | null // currently active vault
  vault: VaultMetadata | null // active vault for backward compatibility
  openTabs: Tab[] // all open tabs (split-pane system) - Req 24.1
  activeTabId: string | null // currently active tab
  paneLayout: PaneLayout // current layout type - Req 24.2
  workspaces: Workspace[] // saved workspaces per vault - Req 25.1
  tabGroups: TabGroup[] // folder-based tab groups - Req 24.9
  // --- Derived (single owner: openTabs[activeTabId]) ---
  // The following five fields are DERIVED from openTabs + activeTabId and are
  // never stored independently. They are computed by syncActiveAliases after
  // every tab mutation so there is exactly one owner of the underlying
  // data and no manual synchronization can drift out of sync.
  currentFile: string | null // derived: openTabs[activeTabId]?.path
  currentAST: Root | null // derived: openTabs[activeTabId]?.ast
  toggleStates: Map<string, Map<string, boolean>> // filePath → (headingId → isOpen)
  contextPaneOpen: boolean
  contextResults: SearchResult[]
  showSetup: boolean
  // --- Derived (single owner: openTabs[activeTabId].mode) ---
  editMode: boolean // derived: openTabs[activeTabId]?.mode === 'edit'
  livePreviewMode: boolean // derived: openTabs[activeTabId]?.mode === 'live-preview'
  currentRaw: string | null // derived: openTabs[activeTabId]?.raw
  graphEdges: Edge[]
  fullTextIndex: Map<string, Set<string>>
  tagIndex: Map<string, Set<string>>
  selectedTags: Set<string>
  settingsPanelOpen: boolean
  graphViewOpen: boolean
  pdfViewOpen: boolean
  pdfPath: string | null
  theme: 'dark' | 'light' | 'system'
  vectorDisabled: boolean
  vectorDisabledReason: string | null
  extendedIndex: ExtendedSearchIndex | null
  searchPanelOpen: boolean
  searchQuery: string
  searchResults: SearchQueryResult[]
  quickSwitcherOpen: boolean
  commandPaletteOpen: boolean
  recentNotes: string[]
  /** Current graph view mode - Req 38.1 */
  graphMode: 'files' | 'tags' | 'blocks'
  /** Page to navigate to when opening a PDF (for annotation links) - Req 40.8 */
  pdfPage: number | null
}

/** Type helper for graph mode */
export type GraphMode = 'files' | 'tags' | 'blocks'

// Backward-compatible accessor (getter function)
export function getActiveVault(state: AppState): VaultMetadata | null {
  return state.vault
}

// ---------------------------------------------------------------------------
// Action types
// ---------------------------------------------------------------------------

export type AppAction =
  | { type: 'VAULT_OPENED'; payload: VaultMetadata }
  | { type: 'VAULT_SWITCHED'; payload: { vaultId: string; vault: VaultMetadata } }
  | { type: 'VAULT_CLOSED'; payload: { vaultId: string } }
  | { type: 'FILE_LOADED'; payload: { path: string; ast: Root } }
  | { type: 'TAB_OPENED'; payload: { path: string; ast: Root; raw: string } }
  | { type: 'TAB_CLOSED'; payload: { tabId: string } }
  | { type: 'TAB_ACTIVATED'; payload: { tabId: string } }
  | { type: 'TAB_UPDATED'; payload: { tabId: string; patch: Partial<Tab> } }
  | {
      type: 'PANE_LAYOUT_CHANGED'
      payload: { layout: 'single' | 'split-horizontal' | 'split-vertical' | 'grid' }
    }
  | { type: 'TAB_CLOSE_ALL' }
  | { type: 'TAB_SPLIT'; payload: { tabId: string } }
  | { type: 'AST_UPDATED'; payload: { path: string; ast: Root; isExternal?: boolean } }
  | { type: 'TOGGLE_BLOCK'; payload: { filePath: string; headingId: string; isOpen: boolean } }
  | { type: 'CONTEXT_PANE_TOGGLE' }
  | { type: 'FILE_DELETED'; payload: { path: string } }
  | { type: 'CONTEXT_RESULTS'; payload: SearchResult[] }
  | { type: 'SETUP_TOGGLE' }
  | { type: 'EDIT_MODE_ENTER'; payload: string }
  | { type: 'EDIT_MODE_EXIT' }
  | { type: 'LIVE_PREVIEW_MODE_ENTER'; payload: string }
  | { type: 'LIVE_PREVIEW_MODE_EXIT' }
  | { type: 'GRAPH_UPDATED'; payload: Edge[] }
  | { type: 'FULL_TEXT_INDEX_BUILT'; payload: Map<string, Set<string>> }
  | { type: 'TAG_INDEX_BUILT'; payload: Map<string, Set<string>> }
  | { type: 'TAG_FILTER_TOGGLE'; payload: string }
  | { type: 'SETTINGS_PANEL_TOGGLE' }
  | { type: 'GRAPH_VIEW_TOGGLE' }
  | { type: 'PDF_OPENED'; payload: { path: string; page?: number } }
  | { type: 'PDF_CLOSED' }
  | { type: 'GRAPH_MODE_CHANGED'; payload: 'files' | 'tags' | 'blocks' }
  | { type: 'THEME_CHANGED'; payload: 'dark' | 'light' | 'system' }
  | { type: 'VECTOR_STATUS_UPDATED'; payload: { disabled: boolean; reason: string | null } }
  | { type: 'EXTENDED_INDEX_BUILT'; payload: ExtendedSearchIndex }
  | { type: 'SEARCH_PANEL_TOGGLE' }
  | { type: 'SEARCH_PANEL_OPEN' }
  | { type: 'SEARCH_PANEL_CLOSE' }
  | { type: 'SEARCH_PANEL_OPEN_WITH_QUERY'; payload: string }
  | { type: 'SEARCH_RESULTS_UPDATED'; payload: { query: string; results: SearchQueryResult[] } }
  | { type: 'QUICK_SWITCHER_TOGGLE' }
  | { type: 'QUICK_SWITCHER_OPEN' }
  | { type: 'QUICK_SWITCHER_CLOSE' }
  | { type: 'COMMAND_PALETTE_TOGGLE' }
  | { type: 'COMMAND_PALETTE_OPEN' }
  | { type: 'COMMAND_PALETTE_CLOSE' }
  | { type: 'RECENT_NOTE_OPENED'; payload: string }
  | { type: 'WORKSPACE_SAVE'; payload: { name: string } }
  | { type: 'WORKSPACE_LOAD'; payload: { workspaceId: string } }
  | { type: 'WORKSPACES_LOADED'; payload: Workspace[] }

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

export const initialState: AppState = {
  openVaults: [],
  activeVaultId: null,
  vault: null,
  openTabs: [],
  activeTabId: null,
  paneLayout: 'single',
  workspaces: [],
  tabGroups: [],
  currentFile: null,
  currentAST: null,
  toggleStates: new Map(),
  contextPaneOpen: false,
  contextResults: [],
  // vault restore (existing `pollForVault`) dispatches `VAULT_OPENED` which sets `showSetup: false`, so the wizard only shows when no vault is auto-restored
  showSetup: true,
  livePreviewMode: false,
  editMode: false,
  currentRaw: null,
  graphEdges: [],
  fullTextIndex: new Map(),
  tagIndex: new Map(),
  selectedTags: new Set(),
  settingsPanelOpen: false,
  graphViewOpen: false,
  pdfViewOpen: false,
  pdfPath: null,
  pdfPage: null,
  theme: 'dark',
  vectorDisabled: false,
  vectorDisabledReason: null,
  extendedIndex: null,
  searchPanelOpen: false,
  searchQuery: '',
  searchResults: [],
  quickSwitcherOpen: false,
  commandPaletteOpen: false,
  recentNotes: [],
  graphMode: 'files'
}

// Generate a unique tab ID
function generateTabId(): string {
  return typeof crypto !== 'undefined' && crypto.randomUUID
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2, 11)
}

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'VAULT_OPENED': {
      // Ensure the vault is in openVaults array
      const existingVault = state.openVaults.find((v) => v.path === action.payload.path)
      const openVaults = existingVault
        ? state.openVaults
        : [
            ...state.openVaults,
            {
              id: action.payload.path,
              path: action.payload.path,
              name: action.payload.path.split('/').pop() ?? 'vault'
            }
          ]
      return {
        ...state,
        openVaults,
        activeVaultId: action.payload.path,
        vault: action.payload,
        showSetup: false
      }
    }

    case 'VAULT_SWITCHED': {
      const { vaultId, vault } = action.payload
      const existingVault = state.openVaults.find((v) => v.id === vaultId)
      const openVaults = existingVault
        ? state.openVaults
        : [
            ...state.openVaults,
            { id: vaultId, path: vault.path, name: vault.path.split('/').pop() ?? 'vault' }
          ]
      return {
        ...state,
        openVaults,
        activeVaultId: vaultId,
        vault
      }
    }

    case 'VAULT_CLOSED': {
      const { vaultId } = action.payload
      const openVaults = state.openVaults.filter((v) => v.id !== vaultId)
      return {
        ...state,
        openVaults,
        activeVaultId:
          state.activeVaultId === vaultId
            ? openVaults.length > 0
              ? openVaults[0].id
              : null
            : state.activeVaultId,
        vault:
          state.activeVaultId === vaultId
            ? openVaults.length > 0
              ? state.vault
              : null
            : state.vault
      }
    }

    // Task 71: Tab management actions (Req 24.1)
    case 'TAB_OPENED': {
      const { path, ast, raw } = action.payload
      // Check if tab already exists for this path
      const existingTab = state.openTabs.find((t) => t.path === path)
      if (existingTab) {
        // Tab exists, just activate it
        return {
          ...syncActiveAliases({ ...state, activeTabId: existingTab.id }),
          currentFile: existingTab.path,
          currentAST: existingTab.ast
        }
      }
      // Create new tab
      const newTab: Tab = {
        id: generateTabId(),
        path,
        ast,
        raw,
        mode: 'view',
        scrollTop: 0,
        cursor: 0
      }
      return {
        ...syncActiveAliases({
          ...state,
          openTabs: [...state.openTabs, newTab],
          activeTabId: newTab.id
        }),
        currentFile: newTab.path,
        currentAST: newTab.ast
      }
    }

    case 'TAB_CLOSED': {
      const { tabId } = action.payload
      const tabIndex = state.openTabs.findIndex((t) => t.id === tabId)
      const wasActive = state.activeTabId === tabId
      const remainingTabs = state.openTabs.filter((t) => t.id !== tabId)

      // Determine new active tab
      let newActiveTabId: string | null = state.activeTabId
      let newActiveTab: Tab | null = null
      if (wasActive && remainingTabs.length > 0) {
        // Activate the next tab (or previous if closing last)
        const newIndex = Math.min(tabIndex, remainingTabs.length - 1)
        newActiveTabId = remainingTabs[newIndex]?.id ?? null
        newActiveTab = remainingTabs[newIndex] ?? null
      }

      return {
        ...syncActiveAliases({
          ...state,
          openTabs: remainingTabs,
          activeTabId: newActiveTabId
        }),
        currentFile: newActiveTab?.path ?? null,
        currentAST: newActiveTab?.ast ?? null
      }
    }

    case 'TAB_ACTIVATED': {
      const { tabId } = action.payload
      const activatedTab = state.openTabs.find((t) => t.id === tabId)
      if (!activatedTab) return state
      return {
        ...syncActiveAliases({ ...state, activeTabId: tabId }),
        currentFile: activatedTab.path,
        currentAST: activatedTab.ast
      }
    }

    case 'TAB_UPDATED': {
      const { tabId, patch } = action.payload
      const updatedTabs = state.openTabs.map((tab) =>
        tab.id === tabId ? { ...tab, ...patch } : tab
      )
      const updatedActive = updatedTabs.find((t) => t.id === state.activeTabId) ?? null
      return {
        ...syncActiveAliases({ ...state, openTabs: updatedTabs }),
        // Keep currentFile/currentAST in sync when the active tab is patched.
        currentFile:
          state.activeTabId === tabId ? (updatedActive?.path ?? state.currentFile) : state.currentFile,
        currentAST:
          state.activeTabId === tabId ? (updatedActive?.ast ?? state.currentAST) : state.currentAST
      }
    }

    // Backward-compatible FILE_LOADED (used by existing IPC handler)
    case 'FILE_LOADED': {
      // Track recently opened notes (capped at 10, deduped)
      const recentNotes = [
        action.payload.path,
        ...state.recentNotes.filter((p) => p !== action.payload.path)
      ].slice(0, 10)
      return {
        ...state,
        currentFile: action.payload.path,
        currentAST: action.payload.ast,
        recentNotes
      }
    }

    case 'AST_UPDATED': {
      const updatedToggleStates = new Map(state.toggleStates)
      if (!updatedToggleStates.has(action.payload.path)) {
        updatedToggleStates.set(action.payload.path, new Map())
      }
      return {
        ...state,
        currentAST:
          state.currentFile === action.payload.path ? action.payload.ast : state.currentAST,
        toggleStates: updatedToggleStates
      }
    }

    case 'TOGGLE_BLOCK': {
      const { filePath, headingId, isOpen } = action.payload
      const nextToggleStates = new Map(state.toggleStates)
      const fileToggles = new Map(nextToggleStates.get(filePath) ?? new Map<string, boolean>())
      fileToggles.set(headingId, isOpen)
      nextToggleStates.set(filePath, fileToggles)
      return { ...state, toggleStates: nextToggleStates }
    }

    case 'CONTEXT_PANE_TOGGLE':
      return { ...state, contextPaneOpen: !state.contextPaneOpen }

    case 'FILE_DELETED':
      return {
        ...state,
        currentFile: state.currentFile === action.payload.path ? null : state.currentFile,
        currentAST: state.currentFile === action.payload.path ? null : state.currentAST
      }

    case 'CONTEXT_RESULTS':
      return { ...state, contextResults: action.payload }

    case 'SETUP_TOGGLE':
      return { ...state, showSetup: !state.showSetup }

    case 'EDIT_MODE_ENTER': {
      // The active note (FILE_LOADED path may have no tab) owns editMode/
      // currentRaw directly. If a tab exists for the active note we also keep
      // its `mode`/`raw` in sync so the two representations never diverge.
      const openTabs = state.openTabs.map((t) =>
        t.id === state.activeTabId ? { ...t, mode: 'edit' as const, raw: action.payload } : t
      )
      return { ...syncActiveAliases({ ...state, openTabs }), editMode: true, currentRaw: action.payload }
    }

    case 'EDIT_MODE_EXIT': {
      const openTabs = state.openTabs.map((t) =>
        t.id === state.activeTabId ? { ...t, mode: 'view' as const, raw: null } : t
      )
      return { ...syncActiveAliases({ ...state, openTabs }), editMode: false, currentRaw: null }
    }

    case 'LIVE_PREVIEW_MODE_ENTER': {
      const openTabs = state.openTabs.map((t) =>
        t.id === state.activeTabId
          ? { ...t, mode: 'live-preview' as const, raw: action.payload }
          : t
      )
      return {
        ...syncActiveAliases({ ...state, openTabs }),
        livePreviewMode: true,
        currentRaw: action.payload
      }
    }

    case 'LIVE_PREVIEW_MODE_EXIT': {
      const openTabs = state.openTabs.map((t) =>
        t.id === state.activeTabId ? { ...t, mode: 'view' as const, raw: null } : t
      )
      return {
        ...syncActiveAliases({ ...state, openTabs }),
        livePreviewMode: false,
        currentRaw: null
      }
    }

    case 'GRAPH_UPDATED':
      return { ...state, graphEdges: action.payload }

    case 'FULL_TEXT_INDEX_BUILT':
      return { ...state, fullTextIndex: action.payload }

    case 'TAG_INDEX_BUILT':
      return { ...state, tagIndex: action.payload }

    case 'TAG_FILTER_TOGGLE': {
      const next = new Set(state.selectedTags)
      if (next.has(action.payload)) {
        next.delete(action.payload)
      } else {
        next.add(action.payload)
      }
      return { ...state, selectedTags: next }
    }

    case 'SETTINGS_PANEL_TOGGLE':
      return { ...state, settingsPanelOpen: !state.settingsPanelOpen }

    case 'GRAPH_VIEW_TOGGLE':
      return { ...state, graphViewOpen: !state.graphViewOpen }

    // Task 92: PDF viewer open/close (Req 40.1, 40.3)
    // Task 95: Support page navigation for annotation links (Req 40.8)
    case 'PDF_OPENED': {
      return {
        ...state,
        pdfViewOpen: true,
        pdfPath: action.payload.path,
        pdfPage: action.payload.page ?? null,
        graphViewOpen: false
      }
    }

    case 'PDF_CLOSED':
      return {
        ...state,
        pdfViewOpen: false,
        pdfPath: null
      }

    case 'GRAPH_MODE_CHANGED':
      return { ...state, graphMode: action.payload }

    case 'THEME_CHANGED':
      return { ...state, theme: action.payload }

    case 'VECTOR_STATUS_UPDATED':
      return {
        ...state,
        vectorDisabled: action.payload.disabled,
        vectorDisabledReason: action.payload.reason
      }

    case 'EXTENDED_INDEX_BUILT':
      return { ...state, extendedIndex: action.payload }

    case 'SEARCH_PANEL_TOGGLE':
      return { ...state, searchPanelOpen: !state.searchPanelOpen }

    case 'SEARCH_PANEL_OPEN':
      return { ...state, searchPanelOpen: true }

    case 'SEARCH_PANEL_CLOSE':
      return { ...state, searchPanelOpen: false }

    case 'SEARCH_PANEL_OPEN_WITH_QUERY':
      return { ...state, searchPanelOpen: true, searchQuery: action.payload, searchResults: [] }

    case 'SEARCH_RESULTS_UPDATED':
      return { ...state, searchQuery: action.payload.query, searchResults: action.payload.results }

    case 'QUICK_SWITCHER_TOGGLE':
      return { ...state, quickSwitcherOpen: !state.quickSwitcherOpen }

    case 'QUICK_SWITCHER_OPEN':
      return { ...state, quickSwitcherOpen: true }

    case 'QUICK_SWITCHER_CLOSE':
      return { ...state, quickSwitcherOpen: false }

    case 'COMMAND_PALETTE_TOGGLE':
      return { ...state, commandPaletteOpen: !state.commandPaletteOpen }

    case 'COMMAND_PALETTE_OPEN':
      return { ...state, commandPaletteOpen: true }

    case 'COMMAND_PALETTE_CLOSE':
      return { ...state, commandPaletteOpen: false }

    case 'RECENT_NOTE_OPENED': {
      const recentNotes = [
        action.payload,
        ...state.recentNotes.filter((p) => p !== action.payload)
      ].slice(0, 10)
      return { ...state, recentNotes }
    }

    case 'PANE_LAYOUT_CHANGED':
      return { ...state, paneLayout: action.payload.layout }

    case 'TAB_CLOSE_ALL':
      return syncActiveAliases({
        ...state,
        openTabs: [],
        activeTabId: null,
        currentFile: null,
        currentAST: null
      })

    // Workspace actions (Req 25.1-25.5)
    case 'WORKSPACE_SAVE': {
      const { name } = action.payload
      const workspace: Workspace = {
        id: generateTabId(),
        name,
        openTabs: state.openTabs.map((t) => t.path),
        paneLayout: state.paneLayout
      }
      return {
        ...state,
        workspaces: [...state.workspaces, workspace]
      }
    }

    case 'WORKSPACE_LOAD': {
      const { workspaceId } = action.payload
      const workspace = state.workspaces.find((w) => w.id === workspaceId)
      if (!workspace) return state
      return {
        ...state,
        paneLayout: workspace.paneLayout
      }
    }

    case 'WORKSPACES_LOADED':
      return { ...state, workspaces: action.payload }

    default:
      return state
  }
}

/**
 * Synchronize the active-tab alias fields from the canonical tab state.
 *
 * `currentRaw`, `editMode`, and `livePreviewMode` are PURE functions of
 * `openTabs[activeTabId]` and are recomputed here after every tab mutation so
 * there is exactly one owner of that derivation (the reducer) and no
 * hand-written synchronization can drift out of sync.
 *
 * `currentFile` / `currentAST` are intentionally NOT overwritten here: they can
 * also be set by the legacy `FILE_LOADED` action, which loads a note without
 * opening a tab. That path is the single justified non-tab writer and is
 * documented in the Phase 5.2 duplicate-state report.
 *
 * This is a deterministic, side-effect-free projection; runtime behavior is
 * unchanged.
 */
export function syncActiveAliases(state: AppState): AppState {
  const activeTab = state.openTabs.find((t) => t.id === state.activeTabId) ?? null
  return {
    ...state,
    currentRaw: activeTab?.raw ?? null,
    editMode: activeTab?.mode === 'edit',
    livePreviewMode: activeTab?.mode === 'live-preview'
  }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface AppContextValue {
  state: AppState
  dispatch: React.Dispatch<AppAction>
}

export const AppContext = createContext<AppContextValue>({
  state: initialState,
  dispatch: () => undefined
})

export function useAppContext(): AppContextValue {
  return useContext(AppContext)
}
