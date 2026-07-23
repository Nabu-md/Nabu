import React, { useEffect, useCallback, useRef, useReducer } from 'react'
import { Root } from 'mdast'
import { Edge } from '@shared/types'
import { recordExternalActivity } from './features/widgets/widgetService'
import { ipc } from './shared/ipc'
import { AppContext, appReducer, initialState } from './shared/store'

function App(): React.JSX.Element {
  const [state, dispatch] = useReducer(appReducer, initialState)
  const sidebarRef = useRef<any>(null)

  const wireListeners = useCallback(() => {
    const offNoteLoaded = ipc.on.noteLoaded(({ path, ast }: { path: string; ast: Root }) => {
      dispatch({ type: 'FILE_LOADED', payload: { path, ast } })
      dispatch({ type: 'AST_UPDATED', payload: { path, ast } })
    })

    const offNoteUpdated = ipc.on.noteUpdated(({ path, ast, isExternal }: any) => {
      dispatch({ type: 'AST_UPDATED', payload: { path, ast, isExternal } })
      if (isExternal) recordExternalActivity(path)
    })

    const offNoteDeleted = ipc.on.noteDeleted(({ path }: { path: string }) => {
      dispatch({ type: 'FILE_DELETED', payload: { path } })
    })

    const offNoteOpenRequested = ipc.on.noteOpenRequested(({ path }: { path: string }) => {
      ipc.file
        .get(path)
        .then((fileAST: any) => {
          dispatch({
            type: 'FILE_LOADED',
            payload: { path: fileAST.path, ast: fileAST.ast }
          })
        })
        .catch((err: Error) => console.error('[App] widget open-note failed:', err))
    })

    const offContextSearch = ipc.on.contextSearch((data: any) => {
      const payload = data as Record<string, unknown>
      const results = (payload['results'] ?? data) as import('@shared/types').SearchResult[]
      dispatch({ type: 'CONTEXT_RESULTS', payload: results })
    })

    const offFocusSearch = ipc.on.focusSearch(() => {
      sidebarRef.current?.focusSearch()
    })

    const offVaultOpened = ipc.on.vaultOpened((vaultMeta: any) => {
      dispatch({ type: 'VAULT_OPENED', payload: vaultMeta })
    })

    const offNotesLoaded = ipc.on.notesLoaded((data: any) => {
      if (data.vaultPath) {
        dispatch({ type: 'VAULT_OPENED', payload: { path: data.vaultPath, files: data.files } })
      }
    })

    const offIndexBuild = ipc.on.indexBuild((data: any) => {
        // Implementation logic...
    })

    const offOpenSettings = ipc.on.openSettings(() => {
      dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
    })

    const offSetupCreate = ipc.on.setupCreate(() => {
      dispatch({ type: 'SETUP_TOGGLE' })
    })
    const offSetupOpen = ipc.on.setupOpen(() => {
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
      offIndexBuild()
      offOpenSettings()
      offSetupCreate()
      offSetupOpen()
    }
  }, [dispatch])

  useEffect(() => {
    return wireListeners()
  }, [wireListeners])

  return (
    <AppContext.Provider value={{ state, dispatch }}>
      <div className="app">
        {/* App layout */}
      </div>
    </AppContext.Provider>
  )
}

export default App
