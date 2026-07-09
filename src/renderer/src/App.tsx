import React, {
  createContext,
  useContext,
  useReducer,
  useEffect,
  useCallback,
  useRef,
  Component,
  ErrorInfo,
  ReactNode
} from 'react'
import { Root } from 'mdast'
import { VaultMetadata, ActivityEntry, SearchResult, Edge } from '../../shared/types'
import type { ExtendedSearchIndex } from '../../shared/extended-indexing'
import { Sidebar, SidebarHandle } from './components/Sidebar'
import { NoteView } from './components/NoteView'
import { GraphView } from './components/GraphView'
import { SettingsPanel } from './components/SettingsPanel'
import { ContextPane } from './components/ContextPane'
import { ActivityTimeline } from './components/ActivityTimeline'
import { SetupWizard } from './components/SetupWizard'
import { SearchPanel } from './components/SearchPanel'
import { QuickSwitcher } from './components/QuickSwitcher'
import { CommandPalette } from './components/CommandPalette'
import { seedCommands, registerCommand } from './commands/registry'
import type { SearchQueryResult } from '../../shared/search-query'

// ---------------------------------------------------------------------------
// State shape
// ---------------------------------------------------------------------------

export interface OpenVault {
  id: string // vaultId
  path: string
  name: string
}

export interface AppState {
  openVaults: OpenVault[] // all open vaults (multi-vault)
  activeVaultId: string | null // currently active vault
  vault: VaultMetadata | null // active vault for backward compatibility
  currentFile: string | null
  currentAST: Root | null
  toggleStates: Map<string, Map<string, boolean>> // filePath → (headingId → isOpen)
  contextPaneOpen: boolean
  activityLog: ActivityEntry[]
  contextResults: SearchResult[]
  showSetup: boolean
  editMode: boolean
  currentRaw: string | null
  graphEdges: Edge[]
  fullTextIndex: Map<string, Set<string>>
  tagIndex: Map<string, Set<string>>
  selectedTags: Set<string>
  settingsPanelOpen: boolean
  graphViewOpen: boolean
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
}

// Backward-compatible accessor (getter function)
export function getActiveVault(state: AppState): VaultMetadata | null {
  return state.vault
}

// ---------------------------------------------------------------------------
// Action types
// ---------------------------------------------------------------------------

export type AppAction =
  | { type: 'VAULT_OPENED'; payload: VaultMetadata }
  | { type: 'FILE_LOADED'; payload: { path: string; ast: Root } }
  | { type: 'AST_UPDATED'; payload: { path: string; ast: Root; isExternal?: boolean } }
  | { type: 'TOGGLE_BLOCK'; payload: { filePath: string; headingId: string; isOpen: boolean } }
  | { type: 'CONTEXT_PANE_TOGGLE' }
  | { type: 'ACTIVITY_ADD'; payload: ActivityEntry }
  | { type: 'FILE_DELETED'; payload: { path: string } }
  | { type: 'CONTEXT_RESULTS'; payload: SearchResult[] }
  | { type: 'SETUP_TOGGLE' }
  | { type: 'EDIT_MODE_ENTER'; payload: string }
  | { type: 'EDIT_MODE_EXIT' }
  | { type: 'GRAPH_UPDATED'; payload: Edge[] }
  | { type: 'FULL_TEXT_INDEX_BUILT'; payload: Map<string, Set<string>> }
  | { type: 'TAG_INDEX_BUILT'; payload: Map<string, Set<string>> }
  | { type: 'TAG_FILTER_TOGGLE'; payload: string }
  | { type: 'SETTINGS_PANEL_TOGGLE' }
  | { type: 'GRAPH_VIEW_TOGGLE' }
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

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

const initialState: AppState = {
  openVaults: [],
  activeVaultId: null,
  vault: null,
  currentFile: null,
  currentAST: null,
  toggleStates: new Map(),
  contextPaneOpen: false,
  activityLog: [],
  contextResults: [],
  // vault restore (existing `pollForVault`) dispatches `VAULT_OPENED` which sets `showSetup: false`, so the wizard only shows when no vault is auto-restored
  showSetup: true,
  editMode: false,
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
}

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'VAULT_OPENED':
      return { ...state, vault: action.payload, showSetup: false, currentFile: null, currentAST: null }

    case 'FILE_LOADED': {
      // Track recently opened notes (capped at 10, deduped)
      const recentNotes = [
        action.payload.path,
        ...state.recentNotes.filter((p) => p !== action.payload.path),
      ].slice(0, 10)
      return {
        ...state,
        currentFile: action.payload.path,
        currentAST: action.payload.ast,
        recentNotes,
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

    case 'ACTIVITY_ADD':
      return {
        ...state,
        activityLog: [action.payload, ...state.activityLog].slice(0, 100)
      }

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

    case 'EDIT_MODE_ENTER':
      return { ...state, editMode: true, currentRaw: action.payload }

    case 'EDIT_MODE_EXIT':
      return { ...state, editMode: false, currentRaw: null }

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

    case 'THEME_CHANGED':
      return { ...state, theme: action.payload }

    case 'VECTOR_STATUS_UPDATED':
      return { ...state, vectorDisabled: action.payload.disabled, vectorDisabledReason: action.payload.reason }

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
        ...state.recentNotes.filter((p) => p !== action.payload),
      ].slice(0, 10)
      return { ...state, recentNotes }
    }

    default:
      return state
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

// ---------------------------------------------------------------------------
// ErrorBoundary
// ---------------------------------------------------------------------------

interface ErrorBoundaryProps {
  children: ReactNode
  fallback?: ReactNode
}

interface ErrorBoundaryState {
  hasError: boolean
  error: Error | null
}

class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('[ErrorBoundary] Caught error:', error, info)
  }

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        this.props.fallback ?? (
          <div className="error-boundary" role="alert" aria-live="assertive">
            <p className="error-boundary__title">Something went wrong</p>
            <p className="error-boundary__message">{this.state.error?.message}</p>
          </div>
        )
      )
    }
    return this.props.children
  }
}

// ---------------------------------------------------------------------------
// Root App component
// ---------------------------------------------------------------------------

function App(): React.JSX.Element {
  const [state, dispatch] = useReducer(appReducer, initialState)
  const sidebarRef = useRef<SidebarHandle>(null)

  // Cmd+Shift+F → focus sidebar search input
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent): void => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'f') {
        e.preventDefault()
        dispatch({ type: 'SEARCH_PANEL_TOGGLE' })
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'g') {
        e.preventDefault()
        dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'o') {
        e.preventDefault()
        dispatch({ type: 'QUICK_SWITCHER_TOGGLE' })
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'p') {
        e.preventDefault()
        dispatch({ type: 'COMMAND_PALETTE_TOGGLE' })
      }
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'o') {
        e.preventDefault()
        // Focus the outline panel in the sidebar.
        const panel = document.querySelector('.outline-panel')
        if (panel) {
          panel.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
        } else {
          // No outline available — dispatch a hint for the user
          console.info('[App] No outline to focus (note has no headings)')
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  // Wire IPC listeners
  const wireListeners = useCallback(() => {
    const { electron } = window

    const offNoteLoaded = electron.on.noteLoaded(({ path, ast }) => {
      dispatch({ type: 'FILE_LOADED', payload: { path, ast } })
      dispatch({ type: 'AST_UPDATED', payload: { path, ast } })
    })

    const offNoteUpdated = electron.on.noteUpdated(({ path, ast, isExternal }) => {
      dispatch({ type: 'AST_UPDATED', payload: { path, ast, isExternal } })
      if (isExternal) {
        dispatch({
          type: 'ACTIVITY_ADD',
          payload: { filePath: path, timestamp: Date.now(), isExternal: true }
        })
      }
    })

    const offNoteDeleted = electron.on.noteDeleted(({ path }) => {
      dispatch({ type: 'FILE_DELETED', payload: { path } })
    })

    const offContextSearch = electron.on.contextSearch((data) => {
      const payload = data as Record<string, unknown>
      const results = (payload['results'] ?? data) as import('../../shared/types').SearchResult[]
      dispatch({ type: 'CONTEXT_RESULTS', payload: results })
    })

    const offFocusSearch = electron.on.focusSearch(() => {
      sidebarRef.current?.focusSearch()
    })

    const offActivityLog = electron.on.activityLog((entry) => {
      // activity:log messages have { level, message, timestamp } shape.
      // Convert to ActivityEntry for the timeline display.
      const logEntry = entry as unknown as { level: string; message: string; timestamp: number }
      dispatch({
        type: 'ACTIVITY_ADD',
        payload: {
          filePath: logEntry.message ?? '',
          timestamp: logEntry.timestamp ?? Date.now(),
          isExternal: false
        }
      })
    })

    // Handle vault:opened-test — used by E2E tests to inject vault state without
    // going through the native dialog picker. The main process sends this channel
    // after opening the NABU_TEST_VAULT env var path.
    const offVaultOpened = electron.on.vaultOpened((vaultMeta) => {
      dispatch({ type: 'VAULT_OPENED', payload: vaultMeta })
    })

    const offNotesLoaded = electron.on.notesLoaded((data) => {
      // When files are added/removed, update the vault file list
      if (data.vaultPath) {
        dispatch({ type: 'VAULT_OPENED', payload: { path: data.vaultPath, files: data.files } })
      }
    })

    const offIndexBuild = electron.on.indexBuild((data) => {
      const p = data as {
        ftIndex: Record<string, string[]>;
        tagIndex: Record<string, string[]>;
        edges: unknown[];
        extendedIndex: {
          positions: Record<string, Record<string, number[]>>;
          lineSnippets: Record<string, string[]>;
          tagIndex: Record<string, string[]>;
          aliasIndex: Record<string, string[]>;
          propertyIndex: Record<string, Record<string, string[]>>;
          blockRefs: Record<string, Record<string, string>>;
        };
      }
      const ftIndex = new Map(Object.entries(p.ftIndex).map(([k, v]) => [k, new Set(v)]))
      const tagIndex = new Map(Object.entries(p.tagIndex).map(([k, v]) => [k, new Set(v)]))
      dispatch({ type: 'FULL_TEXT_INDEX_BUILT', payload: ftIndex })
      dispatch({ type: 'TAG_INDEX_BUILT', payload: tagIndex })
      dispatch({ type: 'GRAPH_UPDATED', payload: p.edges as Edge[] })

      // Deserialise extended index from JSON-safe payload to Maps/Sets
      const ext = p.extendedIndex
      const positions = new Map<string, Map<string, number[]>>()
      for (const [word, fileMap] of Object.entries(ext.positions)) {
        positions.set(word, new Map(Object.entries(fileMap)))
      }
      const lineSnippets = new Map(Object.entries(ext.lineSnippets))
      const extTagIndex = new Map<string, Set<string>>()
      for (const [tag, paths] of Object.entries(ext.tagIndex)) {
        extTagIndex.set(tag, new Set(paths))
      }
      const aliasIndex = new Map(Object.entries(ext.aliasIndex))
      const propertyIndex = new Map<string, Map<string, Set<string>>>()
      for (const [propName, valueMap] of Object.entries(ext.propertyIndex)) {
        const inner = new Map<string, Set<string>>()
        for (const [value, paths] of Object.entries(valueMap)) {
          inner.set(value, new Set(paths))
        }
        propertyIndex.set(propName, inner)
      }
      const blockRefs = new Map<string, Map<string, string>>()
      for (const [filePath, refs] of Object.entries(ext.blockRefs)) {
        blockRefs.set(filePath, new Map(Object.entries(refs)))
      }
      dispatch({
        type: 'EXTENDED_INDEX_BUILT',
        payload: { positions, lineSnippets, tagIndex: extTagIndex, aliasIndex, propertyIndex, blockRefs },
      })
    })

    const offOpenSettings = electron.on.openSettings(() => {
      dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
    })

    const offSetupCreate = electron.on.setupCreate(() => { dispatch({ type: 'SETUP_TOGGLE' }) })
    const offSetupOpen = electron.on.setupOpen(() => { dispatch({ type: 'SETUP_TOGGLE' }) })

    return () => {
      offNoteLoaded()
      offNoteUpdated()
      offNoteDeleted()
      offContextSearch()
      offActivityLog()
      offVaultOpened()
      offNotesLoaded()
      offFocusSearch()
      offIndexBuild()
      offOpenSettings()
      offSetupCreate()
      offSetupOpen()
    }
  }, [])

  // Initialise theme from persisted settings on mount
  useEffect(() => {
    window.electron.settings.get('theme').then(({ value }) => {
      if (value === 'dark' || value === 'light' || value === 'system') {
        dispatch({ type: 'THEME_CHANGED', payload: value })
      }
    }).catch(console.error)
  }, [])

  // Apply theme to document root and track system preference changes
  useEffect(() => {
    const applyTheme = (theme: 'dark' | 'light' | 'system'): void => {
      if (theme === 'system') {
        const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches
        document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light')
      } else {
        document.documentElement.setAttribute('data-theme', theme)
      }
    }

    applyTheme(state.theme)

    let mediaQuery: MediaQueryList | null = null
    let mediaListener: ((e: MediaQueryListEvent) => void) | null = null

    if (state.theme === 'system') {
      mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      mediaListener = (e: MediaQueryListEvent): void => {
        document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light')
      }
      mediaQuery.addEventListener('change', mediaListener)
    }

    return () => {
      if (mediaQuery && mediaListener) {
        mediaQuery.removeEventListener('change', mediaListener)
      }
    }
  }, [state.theme])

  useEffect(() => {
    const cleanup = wireListeners()

    // Pull the current vault state from main process on mount.
    // This handles the case where NABU_TEST_VAULT (or vault restore) fires
    // before React has mounted — the push message would have been dropped.
    //
    // Because openVault() is async in the main process, we may need to retry
    // if the first call happens before the vault is fully loaded.
    const pollForVault = async (attempts = 0): Promise<void> => {
      try {
        const vaultMeta = await window.electron.vault.getCurrent()
        if (vaultMeta) {
          dispatch({ type: 'VAULT_OPENED', payload: vaultMeta })
        } else if (attempts < 10) {
          // Vault not ready yet — retry after 50ms
          setTimeout(() => pollForVault(attempts + 1), 50)
        }
      } catch (err) {
        console.error('[App] vault:get-current error:', err)
      }
    }
    pollForVault().catch(console.error)

    // Query vector index status on mount so the renderer can surface a
    // non-blocking notice when the bge-micro model failed to load (Req 1.4)
    window.electron.context.status().then((status) => {
      dispatch({ type: 'VECTOR_STATUS_UPDATED', payload: status })
    }).catch(console.error)

    // Seed the command registry with built-in actions (Req 5.2, 5.3)
    seedCommands(dispatch, {
      createDailyNote: async () => {
        const v = state.vault
        if (!v) return
        try {
          const result = await window.electron.note.daily(v.path)
          if (result && (result as { path: string }).path) {
            const { path, ast } = result as { path: string; ast: Root }
            dispatch({ type: 'FILE_LOADED', payload: { path, ast } })
          }
        } catch (err) {
          console.error('[App] Failed to open daily note:', err)
        }
      },
    })

    // Register outline-specific command (Req 7.6)
    registerCommand({
      id: 'outline.focus',
      label: 'Focus outline',
      keywords: ['outline', 'headings', 'toc', 'table of contents'],
      run: () => {
        const panel = document.querySelector('.outline-panel')
        if (panel) {
          panel.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
        }
      },
    })

    return cleanup
  }, [wireListeners])

  return (
    <AppContext.Provider value={{ state, dispatch }}>
      {state.showSetup ? (
        <div className="app-container">
          <SetupWizard />
        </div>
      ) : (
        <div className={`app-container${state.searchPanelOpen ? ' app-container--search-open' : ''}`}>
          <Sidebar ref={sidebarRef} />

          <main className="note-container">
            {/* Tab bar: Note | Graph */}
            <div className="flex items-center border-b border-nabu-border shrink-0">
              <button
                onClick={() => { if (state.graphViewOpen) dispatch({ type: 'GRAPH_VIEW_TOGGLE' }) }}
                className={`px-4 py-2 text-sm border-b-2 transition-colors ${!state.graphViewOpen ? 'border-nabu-accent text-nabu-accent' : 'border-transparent text-nabu-text-muted hover:text-nabu-text'}`}
              >
                Note
              </button>
              <button
                onClick={() => { if (!state.graphViewOpen) dispatch({ type: 'GRAPH_VIEW_TOGGLE' }) }}
                className={`px-4 py-2 text-sm border-b-2 transition-colors ${state.graphViewOpen ? 'border-nabu-accent text-nabu-accent' : 'border-transparent text-nabu-text-muted hover:text-nabu-text'}`}
              >
                Graph
              </button>
            </div>

            <ErrorBoundary
              fallback={
                <div className="error-boundary" role="alert" aria-live="assertive">
                  <p className="error-boundary__title">Could not render note</p>
                  <p className="error-boundary__message">
                    An error occurred while displaying this note. Try selecting a different file.
                  </p>
                </div>
              }
            >
              {state.graphViewOpen ? <GraphView /> : <NoteView />}
            </ErrorBoundary>
          </main>

          <ContextPane />
          <ActivityTimeline />
          {state.searchPanelOpen && (
            <SearchPanel
              query={state.searchQuery}
              results={state.searchResults}
              onQueryChange={(query) => {
                dispatch({ type: 'SEARCH_RESULTS_UPDATED', payload: { query, results: state.searchResults } })
              }}
              onResultsChange={(results) => {
                dispatch({ type: 'SEARCH_RESULTS_UPDATED', payload: { query: state.searchQuery, results } })
              }}
              onClose={() => dispatch({ type: 'SEARCH_PANEL_CLOSE' })}
            />
          )}
        </div>
      )}
      {state.settingsPanelOpen && <SettingsPanel />}
      {state.quickSwitcherOpen && <QuickSwitcher />}
      {state.commandPaletteOpen && <CommandPalette />}
    </AppContext.Provider>
  )
}

export default App
