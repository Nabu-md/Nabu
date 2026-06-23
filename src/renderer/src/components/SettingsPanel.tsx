import React, { useEffect, useRef, useState } from 'react'
import { useAppContext } from '../App'

// ---------------------------------------------------------------------------
// SettingsPanel
//
// Modal overlay for app settings. Covers:
//  - Vault management (switch, re-index)
//  - Theme selection (dark / light / system)
// ---------------------------------------------------------------------------

export function SettingsPanel(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const { settingsPanelOpen, theme } = state

  const [isReindexing, setIsReindexing] = useState(false)
  const [reindexError, setReindexError] = useState<string | null>(null)

  // Trap focus and handle Escape key while panel is open
  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!settingsPanelOpen) return

    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [settingsPanelOpen, dispatch])

  // Move focus into the panel when it opens
  useEffect(() => {
    if (settingsPanelOpen) {
      panelRef.current?.focus()
    }
  }, [settingsPanelOpen])

  if (!settingsPanelOpen) return null

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  const handleBackdropClick = (e: React.MouseEvent<HTMLDivElement>): void => {
    if (e.target === e.currentTarget) {
      dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
    }
  }

  const handleSwitchVault = (): void => {
    dispatch({ type: 'SETUP_TOGGLE' })
    dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
  }

  const handleReindex = async (): Promise<void> => {
    setIsReindexing(true)
    setReindexError(null)
    try {
      await window.electron.vault.scan()
    } catch (err) {
      setReindexError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsReindexing(false)
    }
  }

  const handleThemeChange = async (newTheme: 'dark' | 'light' | 'system'): Promise<void> => {
    dispatch({ type: 'THEME_CHANGED', payload: newTheme })
    try {
      await window.electron.settings.set('theme', newTheme)
    } catch (err) {
      console.error('[SettingsPanel] Failed to persist theme:', err)
    }
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    /* Backdrop */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center
                 bg-black/60 backdrop-blur-sm"
      onClick={handleBackdropClick}
    >
      {/* Panel */}
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        tabIndex={-1}
        className="relative w-full max-w-md mx-4 rounded-lg shadow-2xl
                   bg-onyx-bg-soft border border-onyx-border
                   focus:outline-none"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-onyx-border">
          <h2
            id="settings-title"
            className="text-base font-semibold text-onyx-text"
          >
            Settings
          </h2>
          <button
            aria-label="Close settings"
            onClick={() => dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })}
            className="p-1 rounded text-onyx-text-muted hover:text-onyx-text
                       hover:bg-onyx-bg-mute transition-colors"
          >
            <svg
              aria-hidden="true"
              className="w-4 h-4"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            >
              <path d="M2 2l12 12M14 2L2 14" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4 flex flex-col gap-6">
          {/* ----------------------------------------------------------------
              Vault section
          ---------------------------------------------------------------- */}
          <section aria-labelledby="settings-vault-heading">
            <h3
              id="settings-vault-heading"
              className="text-xs font-medium uppercase tracking-wider
                         text-onyx-text-muted mb-3"
            >
              Vault
            </h3>

            <div className="flex flex-col gap-2">
              {/* Switch Vault */}
              <button
                aria-label="Switch vault"
                onClick={handleSwitchVault}
                className="w-full px-3 py-2 rounded text-sm text-left
                           bg-onyx-bg-mute hover:bg-onyx-border border border-onyx-border
                           text-onyx-text transition-colors"
              >
                Switch Vault
              </button>

              {/* Re-index Vault */}
              <button
                aria-label="Re-index vault"
                disabled={isReindexing}
                onClick={handleReindex}
                className="w-full px-3 py-2 rounded text-sm text-left
                           bg-onyx-bg-mute hover:bg-onyx-border border border-onyx-border
                           text-onyx-text transition-colors
                           disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isReindexing ? (
                  <span className="flex items-center gap-2">
                    <Spinner />
                    Re-indexing…
                  </span>
                ) : (
                  'Re-index Vault'
                )}
              </button>

              {/* Re-index error */}
              {reindexError && (
                <p
                  role="alert"
                  aria-live="assertive"
                  className="text-xs text-red-400 mt-1"
                >
                  {reindexError}
                </p>
              )}
            </div>
          </section>

          {/* ----------------------------------------------------------------
              Theme section
          ---------------------------------------------------------------- */}
          <section aria-labelledby="settings-theme-heading">
            <h3
              id="settings-theme-heading"
              className="text-xs font-medium uppercase tracking-wider
                         text-onyx-text-muted mb-3"
            >
              Theme
            </h3>

            <div
              role="radiogroup"
              aria-label="Theme selection"
              className="flex gap-2"
            >
              {(['dark', 'light', 'system'] as const).map((t) => (
                <button
                  key={t}
                  role="radio"
                  aria-checked={theme === t}
                  onClick={() => handleThemeChange(t)}
                  className={`flex-1 px-3 py-2 rounded text-sm capitalize transition-colors
                    border
                    ${
                      theme === t
                        ? 'bg-onyx-accent/20 border-onyx-accent text-onyx-accent'
                        : 'bg-onyx-bg-mute border-onyx-border text-onyx-text-muted hover:text-onyx-text'
                    }`}
                >
                  {t.charAt(0).toUpperCase() + t.slice(1)}
                </button>
              ))}
            </div>
          </section>

          {/* ----------------------------------------------------------------
              Nemo Sync — paid add-on, available at nemo.app
          ---------------------------------------------------------------- */}
          <section aria-labelledby="settings-sync-heading">
            <h3
              id="settings-sync-heading"
              className="text-xs font-medium uppercase tracking-wider
                         text-onyx-text-muted mb-3"
            >
              Nemo Sync
            </h3>

            <p className="text-xs text-onyx-text-muted mb-3 leading-relaxed">
              End-to-end encrypted sync is available as a paid add-on at
              {' '}<a
                href="https://nemo.app"
                className="text-onyx-accent hover:underline"
                target="_blank"
                rel="noreferrer noopener"
              >nemo.app</a>.
            </p>
          </section>
        </div>
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
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z"
      />
    </svg>
  )
}
