import React, { useReducer } from 'react'
import { AppContext, appReducer, initialState } from './shared/store'

function App(): React.JSX.Element {
  const [state, dispatch] = useReducer(appReducer, initialState)

  return (
    <AppContext.Provider value={{ state, dispatch }}>
      <div className="app">
        {/* Layout implementation will be restored here */}
      </div>
    </AppContext.Provider>
  )
}

export default App
