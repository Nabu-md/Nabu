import { tauriBridge } from '../../shared/tauri-ipc'

import React, { useState } from 'react'
import { useAppContext } from '../../shared/store'

// ---------------------------------------------------------------------------
// SetupWizard
//
// Shown when no vault is open (state.showSetup === true).
// Lets the user either open an existing vault or create a new one.
// ---------------------------------------------------------------------------

type Mode = 'idle' | 'create' | 'open'

export function SetupWizard(): React.JSX.Element {
  const { dispatch } = useAppContext()

  const [mode, setMode] = useState<Mode>('idle')
  const [createVaultName, setCreateVaultName] = useState('')
  const [createVaultParentPath, setCreateVaultParentPath] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  const handleOpenVault = async (): Promise<void> => {
    setIsLoading(true)
    setError(null)
    try {
      const result = await tauriBridge.vault.open()
      if (result) {
        dispatch({ type: 'VAULT_OPENED', payload: result })
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }

  const handleChooseParentFolder = async (): Promise<void> => {
    setError(null)
    try {
      // vault.open() doubles as a directory picker — the result gives us the path
      const result = await window.electron.vault.open()
      setCreateVaultParentPath(result.path)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  const handleCreateVault = async (): Promise<void> => {
    if (!createVaultParentPath || !createVaultName) return
    setIsLoading(true)
    setError(null)
    try {
      await tauriBridge.vault.create(createVaultParentPath, createVaultName)
      // Immediately open the newly created vault
      const result = await tauriBridge.vault.open({ path: join(createVaultParentPath, createVaultName) })
      if (result) {
        dispatch({ type: 'VAULT_OPENED', payload: result })
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }

  const handleRetry = (): void => {
    setError(null)
    setMode('idle')
    setIsLoading(false)
    setCreateVaultName('')
    setCreateVaultParentPath(null)
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div
      className="flex flex-col items-center justify-center h-full w-full
                 bg-nabu-bg text-nabu-text"
    >
      <div
        className="flex flex-col items-center gap-6 w-full max-w-sm px-6 py-10
                   bg-nabu-bg-soft border border-nabu-border rounded-lg shadow-lg"
      >
        {/* App title */}
        <div className="flex flex-col items-center gap-1 mb-2">
          <h1 className="text-2xl font-semibold tracking-tight">Nabu</h1>
          <p className="text-sm text-nabu-text-muted">Your personal knowledge base</p>
        </div>

        {/* Error alert */}
        {error && (
          <div
            role="alert"
            aria-live="assertive"
            className="w-full px-3 py-2 rounded bg-red-900/30 border border-red-700/50
                       text-red-300 text-sm"
          >
            {error}
          </div>
        )}

        {/* Idle state — two primary action buttons */}
        {mode === 'idle' && !error && (
          <div className="flex flex-col gap-3 w-full">
            <button
              aria-label="Create new vault"
              disabled={isLoading}
              onClick={() => setMode('create')}
              className="w-full px-4 py-2.5 rounded bg-nabu-accent hover:bg-nabu-accent-hover
                         text-white font-medium text-sm transition-colors
                         disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Create New Vault
            </button>
            <button
              aria-label="Open existing vault"
              disabled={isLoading}
              onClick={handleOpenVault}
              className="w-full px-4 py-2.5 rounded bg-nabu-bg-mute hover:bg-nabu-border
                         text-nabu-text font-medium text-sm border border-nabu-border
                         transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isLoading ? (
                <span className="flex items-center justify-center gap-2">
                  <Spinner />
                  Opening…
                </span>
              ) : (
                'Open Existing Vault'
              )}
            </button>
          </div>
        )}

        {/* Error state — retry button */}
        {error && (
          <button
            onClick={handleRetry}
            className="w-full px-4 py-2.5 rounded bg-nabu-bg-mute hover:bg-nabu-border
                       text-nabu-text font-medium text-sm border border-nabu-border
                       transition-colors"
          >
            Retry
          </button>
        )}

        {/* Create flow */}
        {mode === 'create' && !error && (
          <div className="flex flex-col gap-3 w-full">
            {/* Folder picker */}
            <div className="flex flex-col gap-1">
              <label className="text-xs text-nabu-text-muted">Parent folder</label>
              <div className="flex items-center gap-2">
                <button
                  disabled={isLoading}
                  onClick={handleChooseParentFolder}
                  className="flex-1 px-3 py-2 rounded bg-nabu-bg-mute hover:bg-nabu-border
                             text-nabu-text text-sm border border-nabu-border
                             transition-colors disabled:opacity-50 disabled:cursor-not-allowed
                             text-left truncate"
                >
                  {createVaultParentPath ?? 'Choose parent folder'}
                </button>
              </div>
            </div>

            {/* Vault name input */}
            <div className="flex flex-col gap-1">
              <label htmlFor="vault-name-input" className="text-xs text-nabu-text-muted">
                Vault name
              </label>
              <input
                id="vault-name-input"
                type="text"
                aria-label="Vault name"
                aria-required="true"
                placeholder="My Vault"
                value={createVaultName}
                disabled={isLoading}
                onChange={(e) => setCreateVaultName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleCreateVault()}
                className="w-full px-3 py-2 rounded bg-nabu-bg-mute border border-nabu-border
                           text-nabu-text placeholder:text-nabu-text-faint text-sm
                           focus:outline-none focus:border-nabu-accent transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed"
              />
            </div>

            {/* Action buttons */}
            <div className="flex gap-2 mt-1">
              <button
                disabled={isLoading || !createVaultName.trim()}
                onClick={handleCreateVault}
                className="flex-1 px-4 py-2.5 rounded bg-nabu-accent hover:bg-nabu-accent-hover
                           text-white font-medium text-sm transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isLoading ? (
                  <span className="flex items-center justify-center gap-2">
                    <Spinner />
                    Creating…
                  </span>
                ) : (
                  'Create'
                )}
              </button>
              <button
                disabled={isLoading}
                onClick={() => {
                  setMode('idle')
                  setError(null)
                  setCreateVaultName('')
                  setCreateVaultParentPath(null)
                }}
                className="px-4 py-2.5 rounded bg-nabu-bg-mute hover:bg-nabu-border
                           text-nabu-text text-sm border border-nabu-border
                           transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Spinner — small inline loading indicator
// ---------------------------------------------------------------------------

function Spinner(): React.JSX.Element {
  return (
    <svg
      aria-hidden="true"
      className="w-3.5 h-3.5 animate-spin"
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z" />
    </svg>
  )
}
