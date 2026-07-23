import React, { useEffect, useRef, useState } from 'react'
import { useAppContext } from '../../shared/store'

export function SettingsPanel(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const { settingsPanelOpen } = state

  const [isReindexing, setIsReindexing] = useState(false)
  const [reindexError, setReindexError] = useState<string | null>(null)
  const [featureToggles, setFeatureToggles] = useState<
    Array<{ id: string; label: string; description: string; enabled: boolean }>
  >([])
  const [toggleErrors, setToggleErrors] = useState<Record<string, string>>({})
  
  const [dictationModel, setDictationModel] = useState<'base' | 'large-v3-turbo-q5'>('base')
  const [dictationModelStatus, setDictationModelStatus] = useState<{
    installed: boolean
    downloading: boolean
    downloadProgress: number
  }>({ installed: false, downloading: false, downloadProgress: 0 })
  const [dictationAvailable, _setDictationAvailable] = useState(false)
  const [dictationError, setDictationError] = useState<string | null>(null)
  
  const [widgetShortcut, setWidgetShortcut] = useState<string>('')
  const [isCapturingShortcut, setIsCapturingShortcut] = useState(false)

  useEffect(() => {
    if (settingsPanelOpen) {
      window.ipc.settings
        .getFeatureToggles()
        .then((toggles) => {
          setFeatureToggles(toggles)
        })
        .catch(console.error)
      window.ipc.settings
        .get('clipboardShortcut')
        .then((result: { value: string }) => {
          setWidgetShortcut(result.value ?? 'CmdOrCtrl+§')
        })
        .catch(console.error)
    }
  }, [settingsPanelOpen])

  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (settingsPanelOpen) {
      panelRef.current?.focus()
    }
  }, [settingsPanelOpen])

  if (!settingsPanelOpen) return null

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
      await window.ipc.vault.scan()
    } catch (err) {
      setReindexError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsReindexing(false)
    }
  }

  const handleThemeChange = async (newTheme: 'dark' | 'light' | 'system'): Promise<void> => {
    dispatch({ type: 'THEME_CHANGED', payload: newTheme })
    try {
      await window.ipc.settings.set('theme', newTheme)
    } catch (err) {
      console.error('[SettingsPanel] Failed to persist theme:', err)
    }
  }

  const handleFeatureToggle = async (id: string, enabled: boolean): Promise<void> => {
    try {
      await window.ipc.settings.setFeatureToggle(id, enabled)
      setFeatureToggles((prev) => prev.map((t) => (t.id === id ? { ...t, enabled } : t)))
    } catch (err) {
      setToggleErrors((prev) => ({
        ...prev,
        [id]: err instanceof Error ? err.message : String(err)
      }))
    }
  }

  const handleStartCapture = (): void => {
    setIsCapturingShortcut(true)
  }

  const handleCaptureKeydown = (e: React.KeyboardEvent): void => {
    if (!isCapturingShortcut) return
    e.preventDefault()
    e.stopPropagation()

    const parts: string[] = []
    if (e.metaKey) parts.push('CmdOrCtrl')
    else if (e.ctrlKey) parts.push('CmdOrCtrl')
    if (e.altKey) parts.push('Option')
    if (e.shiftKey) parts.push('Shift')

    const isModifierOnly = ['Alt', 'Shift', 'Control', 'Meta'].includes(e.key)
    if (!isModifierOnly) {
      let key = e.key === '§' ? '§' : e.key
      if (key === ' ' || key === 'Space') key = 'Space'
      if (key === 'Enter') key = 'Enter'
      if (key === 'Escape') key = 'Escape'
      if (key === 'Tab') key = 'Tab'
      if (/^[a-zA-Z0-9]$/.test(key)) key = key.toUpperCase()

      parts.push(key)
      const combo = parts.join('+')
      setWidgetShortcut(combo)
      setIsCapturingShortcut(false)

      window.ipc.settings
        .set('clipboardShortcut', combo)
        .catch(console.error)
      window.ipc.widget.setShortcut(combo).catch(console.error)
    }
  }

  const handleResetShortcut = (): void => {
    const defaultShortcut = 'CmdOrCtrl+§'
    setWidgetShortcut(defaultShortcut)
    window.ipc.settings.set('clipboardShortcut', defaultShortcut).catch(console.error)
    window.ipc.widget.setShortcut(defaultShortcut).catch(console.error)
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center
                 bg-black/60 backdrop-blur-sm"
      onClick={handleBackdropClick}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        tabIndex={-1}
        className="relative w-full max-w-md mx-4 rounded-lg shadow-2xl
                   bg-nabu-bg-soft border border-nabu-border
                   focus:outline-none"
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-nabu-border">
          <h2 id="settings-title" className="text-base font-semibold text-nabu-text">
            Settings
          </h2>
          <button
            aria-label="Close settings"
            onClick={() => dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })}
            className="p-1 rounded text-nabu-text-muted hover:text-nabu-text
                       hover:bg-nabu-bg-mute transition-colors"
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

        <div className="px-5 py-4 flex flex-col gap-6">
          <section aria-labelledby="settings-vault-heading">
            <h3
              id="settings-vault-heading"
              className="text-xs font-medium uppercase tracking-wider
                         text-nabu-text-muted mb-3"
            >
              Vault
            </h3>
            <div className="flex flex-col gap-2">
              <button
                aria-label="Switch vault"
                onClick={handleSwitchVault}
                className="w-full px-3 py-2 rounded text-sm text-left
                           bg-nabu-bg-mute hover:bg-nabu-border border border-nabu-border
                           text-nabu-text transition-colors"
              >
                Switch Vault
              </button>
              <button
                aria-label="Re-index vault"
                disabled={isReindexing}
                onClick={handleReindex}
                className="w-full px-3 py-2 rounded text-sm text-left
                           bg-nabu-bg-mute hover:bg-nabu-border border border-nabu-border
                           text-nabu-text transition-colors
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
              {reindexError && (
                <p role="alert" aria-live="assertive" className="text-xs text-red-400 mt-1">
                  {reindexError}
                </p>
              )}
            </div>
          </section>
        </div>
      </div>
    </div>
  )
}

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
