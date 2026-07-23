import React, {
  useEffect,
  useCallback,
  useRef,
  useReducer,
  Component,
  ErrorInfo,
  ReactNode
} from 'react'
import { Root } from 'mdast'
import { Edge } from '@shared/types'
import { recordExternalActivity } from './features/widgets/widgetService'
import { Sidebar, SidebarHandle } from './features/vault/Sidebar'
import { NoteView } from './features/notes/NoteView'
import { PaneLayout } from './features/vault/PaneLayout'
import { GraphView } from './features/graph/GraphView'
import { PdfViewer } from './features/pdf/PdfViewer'
import { SettingsPanel } from './features/settings/SettingsPanel'
import { ContextPane } from './features/notes/ContextPane'
import { ActivityTimeline } from './features/widgets/ActivityTimeline'
import { SetupWizard } from './features/vault/SetupWizard'
import { SearchPanel } from './features/search/SearchPanel'
import { QuickSwitcher } from './features/search/QuickSwitcher'
import { CommandPalette } from './features/search/CommandPalette'
import { NoteIcon, GraphIcon, EyeIcon, EditIcon } from './shared/components/icons'
import { seedCommands, registerCommand } from './shared/commands/registry'
import { ipc } from './shared/ipc'
import { createNote } from './features/vault/vaultCommands'
import { AppContext, appReducer, initialState } from './shared/store'

// Re-export store symbols so existing imports from `../../App` continue to
// resolve. This preserves backward compatibility for any external consumer
// (e.g. tests) that referenced these names from the root module.
export {
  AppContext,
  useAppContext,
  appReducer,
  syncActiveAliases
} from './shared/store'
export type { AppState, AppAction, Tab, PDFTab, Workspace, TabGroup, PaneLayout, OpenVault, GraphMode } from './shared/store'

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
        recordExternalActivity(path)
      }
    })

    const offNoteDeleted = electron.on.noteDeleted(({ path }) => {
      dispatch({ type: 'FILE_DELETED', payload: { path } })
    })

    const offNoteOpenRequested = electron.on.noteOpenRequested(({ path }) => {
      window.electron.file
        .get(path)
        .then((fileAST) => {
          dispatch({
            type: 'FILE_LOADED',
            payload: { path: fileAST.path, ast: fileAST.ast }
          })
        })
        .catch((err) => console.error('[App] widget open-note failed:', err))
    })

    const offContextSearch = electron.on.contextSearch((data) => {
      const payload = data as Record<string, unknown>
      const results = (payload['results'] ?? data) as import('@shared/types').SearchResult[]
      dispatch({ type: 'CONTEXT_RESULTS', payload: results })
    })

    const offFocusSearch = electron.on.focusSearch(() => {
      sidebarRef.current?.focusSearch()
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
        ftIndex: Record<string, string[]>
        tagIndex: Record<string, string[]>
        edges: unknown[]
        extendedIndex: {
          positions: Record<string, Record<string, number[]>>
          lineSnippets: Record<string, string[]>
          tagIndex: Record<string, string[]>
          aliasIndex: Record<string, string[]>
          propertyIndex: Record<string, Record<string, string[]>>
          blockRefs: Record<string, Record<string, string>>
        }
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
        payload: {
          positions,
          lineSnippets,
          tagIndex: extTagIndex,
          aliasIndex,
          propertyIndex,
          blockRefs
        }
      })
    })

    const offOpenSettings = electron.on.openSettings(() => {
      dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
    })

    const offSetupCreate = electron.on.setupCreate(() => {
      dispatch({ type: 'SETUP_TOGGLE' })
    })
    const offSetupOpen = electron.on.setupOpen(() => {
      dispatch({ type: 'SETUP_TOGGLE' })
    })

    return () => {
      offNoteLoaded()
      offNoteUpdated()
      offNoteDeleted()
      offNoteOpenRequested()
      offContextSearch()
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
    window.electron.settings
      .get('theme')
      .then(({ value }) => {
        if (value === 'dark' || value === 'light' || value === 'system') {
          dispatch({ type: 'THEME_CHANGED', payload: value })
        }
      })
      .catch(console.error)
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
        const vaultMeta = await ipc.vault.getCurrent()
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
    window.electron.context
      .status()
      .then((status) => {
        dispatch({ type: 'VECTOR_STATUS_UPDATED', payload: status })
      })
      .catch(console.error)

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
      createNote: async () => {
        const v = state.vault
        if (!v) return
        const name = window.prompt('Note name (without .md):', '')
        if (!name || !name.trim()) return
        try {
          await createNote(v.path, name.trim(), null, dispatch)
        } catch (err) {
          console.error('[App] Failed to create note:', err)
        }
      }
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
      }
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
        <div
          className={`app-container${state.searchPanelOpen ? ' app-container--search-open' : ''}`}
        >
          <Sidebar ref={sidebarRef} />

          <main className="note-container">
            {/* Toolbar: icon row */}
            <div className="note-toolbar shrink-0">
              <button
                onClick={() => {
                  if (state.graphViewOpen) dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
                }}
                className={`note-toolbar__btn ${!state.graphViewOpen ? 'note-toolbar__btn--active' : ''}`}
                title="Note view"
                aria-label="Note view"
                type="button"
              >
                <NoteIcon size={16} />
              </button>
              <button
                onClick={() => {
                  if (!state.graphViewOpen) dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
                }}
                className={`note-toolbar__btn ${state.graphViewOpen ? 'note-toolbar__btn--active' : ''}`}
                title="Graph view"
                aria-label="Graph view"
                type="button"
              >
                <GraphIcon size={16} />
              </button>
              {!state.graphViewOpen && state.currentAST && (
                <>
                  <div className="note-toolbar__divider" />
                  <button
                    onClick={() => {
                      if (state.editMode) {
                        dispatch({ type: 'EDIT_MODE_EXIT' })
                      } else if (state.currentFile) {
                        dispatch({ type: 'EDIT_MODE_ENTER', payload: state.currentRaw ?? '' })
                      }
                    }}
                    className={`note-toolbar__btn ${state.editMode ? 'note-toolbar__btn--active' : ''}`}
                    title={state.editMode ? 'Preview' : 'Edit'}
                    aria-label={state.editMode ? 'Preview' : 'Edit'}
                    type="button"
                  >
                    {state.editMode ? <EyeIcon size={16} /> : <EditIcon size={16} />}
                  </button>
                </>
              )}
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
              {state.pdfViewOpen ? (
                <PdfViewer
                  filePath={state.pdfPath ?? ''}
                  initialPage={state.pdfPage ?? undefined}
                  onClose={() => dispatch({ type: 'PDF_CLOSED' })}
                />
              ) : state.graphViewOpen ? (
                <GraphView />
              ) : state.paneLayout === 'single' ? (
                <NoteView />
              ) : (
                <PaneLayout />
              )}
            </ErrorBoundary>
          </main>

          <ContextPane />
          <ActivityTimeline />
          {state.searchPanelOpen && (
            <SearchPanel
              query={state.searchQuery}
              results={state.searchResults}
              onQueryChange={(query) => {
                dispatch({
                  type: 'SEARCH_RESULTS_UPDATED',
                  payload: { query, results: state.searchResults }
                })
              }}
              onResultsChange={(results) => {
                dispatch({
                  type: 'SEARCH_RESULTS_UPDATED',
                  payload: { query: state.searchQuery, results }
                })
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
