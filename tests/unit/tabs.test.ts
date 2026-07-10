/**
 * tabs.test.ts
 *
 * Unit tests for the tab management system (Tab, openTabs, activeTabId).
 * Validates Requirement 24.1, 24.8.
 */

import { describe, it, expect } from 'vitest'
import { appReducer, type AppState, type AppAction } from '../../src/renderer/src/App'
import type { Root } from 'mdast'

// ---------------------------------------------------------------------------
// Mock data
// ---------------------------------------------------------------------------

function createMockAST(): Root {
  return { type: 'root', children: [] } as Root
}

// Helper to create a complete initial state
function createInitialState(overrides: Partial<AppState> = {}): AppState {
  return {
    openVaults: [],
    activeVaultId: null,
    vault: null,
    openTabs: [],
    activeTabId: null,
    paneLayout: 'single',
    currentFile: null,
    currentAST: null,
    toggleStates: new Map(),
    contextPaneOpen: false,
    activityLog: [],
    contextResults: [],
    showSetup: true,
    editMode: false,
    livePreviewMode: false,
    currentRaw: null,
    graphEdges: [],
    fullTextIndex: new Map(),
    tagIndex: new Map(),
    selectedTags: new Set(),
    settingsPanelOpen: false,
    graphViewOpen: false,
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
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// Tests for TAB_OPENED action
// ---------------------------------------------------------------------------

describe('Tab System - TAB_OPENED', () => {
  it('creates a new tab when file is opened', () => {
    const initialState = createInitialState()

    const action: AppAction = {
      type: 'TAB_OPENED',
      payload: {
        path: '/vault/notes/test.md',
        ast: createMockAST(),
        raw: '# Test Note',
      },
    }

    const state = appReducer(initialState, action)

    expect(state.openTabs.length).toBe(1)
    expect(state.openTabs[0].path).toBe('/vault/notes/test.md')
    expect(state.activeTabId).toBe(state.openTabs[0].id)
    // Verify compat alias
    expect(state.currentFile).toBe('/vault/notes/test.md')
    expect(state.currentAST?.type).toBe('root')
  })

  it('activates existing tab instead of creating duplicate', () => {
    const existingTabId = 'existing-tab-id'
    const initialState = createInitialState({
      openTabs: [
        {
          id: existingTabId,
          path: '/vault/notes/test.md',
          ast: createMockAST(),
          raw: '# Test Note',
          mode: 'view',
          scrollTop: 0,
          cursor: 0,
        },
      ],
      activeTabId: null,
    })

    const action: AppAction = {
      type: 'TAB_OPENED',
      payload: {
        path: '/vault/notes/test.md',
        ast: createMockAST(),
        raw: '# Test Note',
      },
    }

    const state = appReducer(initialState, action)

    expect(state.openTabs.length).toBe(1)
    expect(state.activeTabId).toBe(existingTabId)
  })
})

// ---------------------------------------------------------------------------
// Tests for TAB_CLOSED action
// ---------------------------------------------------------------------------

describe('Tab System - TAB_CLOSED', () => {
  it('closes a tab and activates next tab', () => {
    const tab1Id = 'tab-1'
    const tab2Id = 'tab-2'
    const initialState = createInitialState({
      openTabs: [
        { id: tab1Id, path: '/vault/notes/one.md', ast: createMockAST(), raw: '# One', mode: 'view', scrollTop: 0, cursor: 0 },
        { id: tab2Id, path: '/vault/notes/two.md', ast: createMockAST(), raw: '# Two', mode: 'view', scrollTop: 0, cursor: 0 },
      ],
      activeTabId: tab1Id,
      currentFile: '/vault/notes/one.md',
      currentAST: createMockAST(),
      currentRaw: '# One',
    })

    const action: AppAction = {
      type: 'TAB_CLOSED',
      payload: { tabId: tab1Id },
    }

    const state = appReducer(initialState, action)

    expect(state.openTabs.length).toBe(1)
    expect(state.activeTabId).toBe(tab2Id)
    expect(state.currentFile).toBe('/vault/notes/two.md')
  })

  it('updates compat alias when active tab is closed', () => {
    const tab1Id = 'tab-1'
    const tab2Id = 'tab-2'
    const initialState = createInitialState({
      openTabs: [
        { id: tab1Id, path: '/vault/notes/one.md', ast: createMockAST(), raw: '# One', mode: 'edit', scrollTop: 0, cursor: 0 },
        { id: tab2Id, path: '/vault/notes/two.md', ast: createMockAST(), raw: '# Two', mode: 'view', scrollTop: 0, cursor: 0 },
      ],
      activeTabId: tab1Id,
      currentFile: '/vault/notes/one.md',
      currentAST: createMockAST(),
      currentRaw: '# One',
      editMode: true,
    })

    const action: AppAction = {
      type: 'TAB_CLOSED',
      payload: { tabId: tab1Id },
    }

    const state = appReducer(initialState, action)

    expect(state.activeTabId).toBe(tab2Id)
    // When closing the active edit-mode tab, editMode should be false
    expect(state.editMode).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Tests for TAB_ACTIVATED action
// ---------------------------------------------------------------------------

describe('Tab System - TAB_ACTIVATED', () => {
  it('activates the specified tab and updates compat aliases', () => {
    const tab1Id = 'tab-1'
    const tab2Id = 'tab-2'
    const initialState = createInitialState({
      openTabs: [
        { id: tab1Id, path: '/vault/notes/one.md', ast: createMockAST(), raw: '# One', mode: 'view', scrollTop: 0, cursor: 0 },
        { id: tab2Id, path: '/vault/notes/two.md', ast: createMockAST(), raw: '# Two', mode: 'edit', scrollTop: 0, cursor: 0 },
      ],
      activeTabId: tab1Id,
      currentFile: '/vault/notes/one.md',
      currentAST: createMockAST(),
    })

    const action: AppAction = {
      type: 'TAB_ACTIVATED',
      payload: { tabId: tab2Id },
    }

    const state = appReducer(initialState, action)

    expect(state.activeTabId).toBe(tab2Id)
    expect(state.currentFile).toBe('/vault/notes/two.md')
    // Tab 2 was in edit mode, so editMode should be true
    expect(state.editMode).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// Tests for TAB_UPDATED action
// ---------------------------------------------------------------------------

describe('Tab System - TAB_UPDATED', () => {
  it('updates tab properties and syncs compat aliases when active', () => {
    const tabId = 'tab-1'
    const initialState = createInitialState({
      openTabs: [
        { id: tabId, path: '/vault/notes/test.md', ast: createMockAST(), raw: '# Test', mode: 'view', scrollTop: 0, cursor: 0 },
      ],
      activeTabId: tabId,
      currentFile: '/vault/notes/test.md',
      currentAST: createMockAST(),
    })

    const action: AppAction = {
      type: 'TAB_UPDATED',
      payload: {
        tabId,
        patch: { scrollTop: 100 },
      },
    }

    const state = appReducer(initialState, action)

    expect(state.openTabs[0].scrollTop).toBe(100)
  })
})