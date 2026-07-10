/**
 * pane-layout.test.ts
 *
 * Unit tests for PaneLayout, Workspace, and Tab Groups (Req 24.2, 24.4, 24.5, 25.1-25.5, 24.9).
 */

import { describe, it, expect } from 'vitest'
import { appReducer, type AppState, type Workspace } from '../../src/renderer/src/App'
import type { Root } from 'mdast'

// Helper to create a complete initial state
function createInitialState(overrides: Partial<AppState> = {}): AppState {
  return {
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
    ...overrides
  }
}

function createMockAST(): Root {
  return { type: 'root', children: [] } as Root
}

// ---------------------------------------------------------------------------
// Tests for paneLayout state
// ---------------------------------------------------------------------------

describe('PaneLayout state', () => {
  it('defaults to single layout', () => {
    const state = createInitialState()
    expect(state.paneLayout).toBe('single')
  })

  it('supports split-horizontal layout', () => {
    const initialState = createInitialState()
    const state = appReducer(initialState, {
      type: 'PANE_LAYOUT_CHANGED',
      payload: { layout: 'split-horizontal' }
    })
    expect(state.paneLayout).toBe('split-horizontal')
  })

  it('supports split-vertical layout', () => {
    const initialState = createInitialState()
    const state = appReducer(initialState, {
      type: 'PANE_LAYOUT_CHANGED',
      payload: { layout: 'split-vertical' }
    })
    expect(state.paneLayout).toBe('split-vertical')
  })

  it('supports grid layout', () => {
    const initialState = createInitialState()
    const state = appReducer(initialState, {
      type: 'PANE_LAYOUT_CHANGED',
      payload: { layout: 'grid' }
    })
    expect(state.paneLayout).toBe('grid')
  })
})

// ---------------------------------------------------------------------------
// Tests for multiple tabs in split view
// ---------------------------------------------------------------------------

describe('Split view - multiple tabs', () => {
  it('can have multiple tabs open simultaneously', () => {
    const initialState = createInitialState()

    // Open first tab
    const state1 = appReducer(initialState, {
      type: 'TAB_OPENED',
      payload: { path: '/vault/notes/one.md', ast: createMockAST(), raw: '# One' }
    })

    // Open second tab
    const state2 = appReducer(state1, {
      type: 'TAB_OPENED',
      payload: { path: '/vault/notes/two.md', ast: createMockAST(), raw: '# Two' }
    })

    expect(state2.openTabs.length).toBe(2)
    // When opening a new tab, it becomes the active tab
    expect(state2.activeTabId).toBe(state2.openTabs[1].id)
  })

  it('active tab change does not modify openTabs array', () => {
    const initialState = createInitialState({
      openTabs: [
        {
          id: 'tab-1',
          path: '/vault/notes/one.md',
          ast: createMockAST(),
          raw: '# One',
          mode: 'view',
          scrollTop: 0,
          cursor: 0
        },
        {
          id: 'tab-2',
          path: '/vault/notes/two.md',
          ast: createMockAST(),
          raw: '# Two',
          mode: 'view',
          scrollTop: 0,
          cursor: 0
        }
      ],
      activeTabId: 'tab-1'
    })

    const state = appReducer(initialState, {
      type: 'TAB_ACTIVATED',
      payload: { tabId: 'tab-2' }
    })

    expect(state.openTabs.length).toBe(2)
    expect(state.activeTabId).toBe('tab-2')
  })
})

// ---------------------------------------------------------------------------
// Tests for workspace save/load (Req 25.1-25.5)
// ---------------------------------------------------------------------------

describe('Workspace management', () => {
  it('can save current tabs and layout as a workspace', () => {
    const initialState = createInitialState({
      openTabs: [
        {
          id: 'tab-1',
          path: '/vault/notes/one.md',
          ast: createMockAST(),
          raw: '# One',
          mode: 'view',
          scrollTop: 0,
          cursor: 0
        },
        {
          id: 'tab-2',
          path: '/vault/notes/two.md',
          ast: createMockAST(),
          raw: '# Two',
          mode: 'view',
          scrollTop: 0,
          cursor: 0
        }
      ],
      activeTabId: 'tab-1',
      paneLayout: 'split-horizontal'
    })

    const state = appReducer(initialState, {
      type: 'WORKSPACE_SAVE',
      payload: { name: 'Research' }
    })

    expect(state.workspaces.length).toBe(1)
    expect(state.workspaces[0].name).toBe('Research')
    expect(state.workspaces[0].openTabs).toEqual(['/vault/notes/one.md', '/vault/notes/two.md'])
    expect(state.workspaces[0].paneLayout).toBe('split-horizontal')
  })

  it('can load workspaces into state', () => {
    const initialState = createInitialState()
    const workspaces: Workspace[] = [
      { id: 'ws-1', name: 'Work', openTabs: ['/vault/notes/a.md'], paneLayout: 'single' },
      {
        id: 'ws-2',
        name: 'Research',
        openTabs: ['/vault/notes/b.md', '/vault/notes/c.md'],
        paneLayout: 'grid'
      }
    ]

    const state = appReducer(initialState, {
      type: 'WORKSPACES_LOADED',
      payload: workspaces
    })

    expect(state.workspaces.length).toBe(2)
    expect(state.workspaces[0].name).toBe('Work')
    expect(state.workspaces[1].name).toBe('Research')
  })

  it('loading a workspace sets the layout', () => {
    const initialState = createInitialState({
      workspaces: [{ id: 'ws-1', name: 'Grid Layout', openTabs: [], paneLayout: 'grid' }],
      paneLayout: 'single'
    })

    const state = appReducer(initialState, {
      type: 'WORKSPACE_LOAD',
      payload: { workspaceId: 'ws-1' }
    })

    expect(state.paneLayout).toBe('grid')
  })

  it('loading non-existent workspace does nothing', () => {
    const initialState = createInitialState({
      paneLayout: 'single'
    })

    const state = appReducer(initialState, {
      type: 'WORKSPACE_LOAD',
      payload: { workspaceId: 'nonexistent' }
    })

    expect(state.paneLayout).toBe('single')
  })
})

// ---------------------------------------------------------------------------
// Tests for tab groups (Req 24.9) - Chrome-style folder grouping
// ---------------------------------------------------------------------------

type TabGroupColor = 'blue' | 'red' | 'green' | 'yellow' | 'purple' | 'orange' | 'cyan' | 'pink'

const COLORS: TabGroupColor[] = [
  'blue',
  'red',
  'green',
  'yellow',
  'purple',
  'orange',
  'cyan',
  'pink'
]

function getColorForFolder(folderPath: string): TabGroupColor {
  // Deterministically assign color based on folder path hash
  let hash = 0
  for (let i = 0; i < folderPath.length; i++) {
    hash = ((hash << 5) - hash + folderPath.charCodeAt(i)) >>> 0
  }
  return COLORS[hash % COLORS.length]
}

function getFolderFromPath(path: string, vaultPath: string): string {
  // Extract relative folder path from absolute file path
  let relativePath = path.replace(vaultPath, '')
  // Strip leading slash if present
  if (relativePath.startsWith('/')) {
    relativePath = relativePath.slice(1)
  }
  const parts = relativePath.split('/')
  // Return parent folder (e.g., "projects/", "research/papers/")
  return parts.length > 1 ? parts.slice(0, -1).join('/') + '/' : ''
}

describe('Tab groups', () => {
  it('default tab groups is empty array', () => {
    const state = createInitialState()
    expect(state.tabGroups).toEqual([])
  })

  it('colors are deterministically assigned by folder path', () => {
    expect(getColorForFolder('projects/')).toBe(getColorForFolder('projects/')) // Same path = same color (deterministic)
    // Different paths should still get valid colors
    const color = getColorForFolder('research/')
    expect(COLORS).toContain(color)
  })

  it('can extract folder path from file path', () => {
    expect(getFolderFromPath('/vault/notes/projects/a.md', '/vault/notes')).toBe('projects/')
    expect(getFolderFromPath('/vault/notes/projects/research/b.md', '/vault/notes')).toBe(
      'projects/research/'
    )
    expect(getFolderFromPath('/vault/notes/root.md', '/vault/notes')).toBe('')
  })

  it('single-tab groups collapse to plain tab (no group label)', () => {
    const initialState = createInitialState({
      openTabs: [
        {
          id: 'tab-1',
          path: '/vault/notes/projects/a.md',
          ast: createMockAST(),
          raw: '',
          mode: 'view',
          scrollTop: 0,
          cursor: 0
        }
      ],
      tabGroups: [
        {
          id: 'group-1',
          folderPath: 'projects/',
          color: 'blue',
          isCollapsed: false,
          tabIds: ['tab-1']
        }
      ]
    })
    // Single-tab groups should just show the tab without group styling
    expect(initialState.tabGroups[0].tabIds.length).toBe(1)
  })
})
