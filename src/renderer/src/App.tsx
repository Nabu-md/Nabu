import React, { useEffect, useCallback, useRef, useReducer } from 'react'
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
import { ipc } from "@renderer-shared/ipc"
import { createNote } from './features/vault/vaultCommands'
import { AppContext, appReducer, initialState } from './shared/store'

// ... (component implementation)

function App(): React.JSX.Element {
  const [state, dispatch] = useReducer(appReducer, initialState)
  const sidebarRef = useRef<any>(null)

  const wireListeners = useCallback(() => {
    const offNoteLoaded = ipc.on.noteLoaded((p: any) => {
      dispatch({ type: 'FILE_LOADED', payload: p })
      dispatch({ type: 'AST_UPDATED', payload: p })
    })

    const offNoteUpdated = ipc.on.noteUpdated((p: any) => {
      dispatch({ type: 'AST_UPDATED', payload: p })
      if (p.isExternal) recordExternalActivity(p.path)
    })

    const offNoteDeleted = ipc.on.noteDeleted((p: any) => {
      dispatch({ type: 'FILE_DELETED', payload: p })
    })

    const offNoteOpenRequested = ipc.on.noteOpenRequested((p: any) => {
      ipc.file
        .get(p.path)
        .then((fileAST: any) => {
          dispatch({
            type: 'FILE_LOADED',
            payload: { path: fileAST.path, ast: fileAST.ast }
          })
        })
        .catch((err: Error) => console.error('[App] widget open-note failed:', err))
    })

    // ... (other listeners using ipc.on)

    return () => {
      offNoteLoaded()
      offNoteUpdated()
      offNoteDeleted()
      offNoteOpenRequested()
    }
  }, [dispatch])

  useEffect(() => {
    return wireListeners()
  }, [wireListeners])

  return (
    <AppContext.Provider value={{ state, dispatch }}>
      <div className="app">
        {/* Layout */}
      </div>
    </AppContext.Provider>
  )
}

export default App
