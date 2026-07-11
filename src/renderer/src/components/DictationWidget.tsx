/**
 * DictationWidget.tsx
 *
 * Always-on-top widget for audio dictation.
 * Shows a wiggling waveform animation with a "Listening..." label
 * when dictation is active.
 *
 * This component is rendered in a separate BrowserWindow (the widget window)
 * that is transparent, frameless, and always-on-top.
 *
 * Requirements: 41.4, 42.2, 42.3, 43.1, 43.2, 43.4
 */

import React, { useEffect, useState, useCallback } from 'react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type WidgetMode = 'clipboard' | 'dictation'

interface DictationState {
  status: 'idle' | 'listening' | 'complete' | 'error'
  text: string
  error: string | null
  silent: boolean
}

// ---------------------------------------------------------------------------
// Waveform Animation Component
// ---------------------------------------------------------------------------

const WaveformAnimation: React.FC = () => {
  const barCount = 16
  const bars = Array.from({ length: barCount }, (_, i) => i)

  return (
    <div className="dictation-widget__waveform">
      {bars.map((i) => (
        <div
          key={i}
          className="dictation-widget__bar"
          style={{
            animationDelay: `${i * 0.1}s`,
            height: `${22 + Math.sin(i * 0.7) * 12}px`
          }}
        />
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// DictationWidget Component
// ---------------------------------------------------------------------------

const DictationWidget: React.FC = () => {
  const [mode, setMode] = useState<WidgetMode>('dictation')
  const [dictationState, setDictationState] = useState<DictationState>({
    status: 'idle',
    text: '',
    error: null,
    silent: false
  })
  const [micPermissionError, setMicPermissionError] = useState(false)

  // Listen for IPC messages from the main process
  useEffect(() => {
    const { electron } = window

    // Listen for mode changes
    const removeModeChanged = electron.on.widgetModeChanged((data: unknown) => {
      const { mode: newMode } = data as { mode: WidgetMode }
      setMode(newMode)
      if (newMode === 'dictation') {
        setDictationState({ status: 'listening', text: '', error: null, silent: false })
      }
    })

    // Listen for dictation starting
    const removeDictationStarting = electron.on.widgetDictationStarting(() => {
      setDictationState({ status: 'listening', text: '', error: null, silent: false })
    })

    // Listen for dictation complete
    const removeDictationComplete = electron.on.widgetDictationComplete((data: unknown) => {
      const { text, silent } = data as { text: string; silent: boolean }
      setDictationState({
        status: 'complete',
        text,
        error: null,
        silent: silent ?? false
      })
    })

    // Listen for dictation errors
    const removeDictationError = electron.on.widgetDictationError((data: unknown) => {
      const { error } = data as { error: string }
      setDictationState({
        status: 'error',
        text: '',
        error,
        silent: false
      })
      if (error && error.includes('Microphone access')) {
        setMicPermissionError(true)
      }
    })

    return () => {
      removeModeChanged()
      removeDictationStarting()
      removeDictationComplete()
      removeDictationError()
    }
  }, [])

  // Handle "Done" button click
  const handleDone = useCallback(async () => {
    try {
      await window.electron.dictation.stop()
    } catch (err) {
      console.error('[DictationWidget] Failed to stop dictation:', err)
    }
  }, [])

  // Handle microphone permission retry
  const handleRetryPermission = useCallback(async () => {
    setMicPermissionError(false)
    setDictationState({ status: 'idle', text: '', error: null, silent: false })
    try {
      await window.electron.dictation.start()
    } catch (err) {
      console.error('[DictationWidget] Failed to start dictation:', err)
    }
  }, [])

  // Render the widget content
  const renderContent = (): React.ReactNode => {
    // Error state
    if (dictationState.status === 'error' && dictationState.error) {
      return (
        <div className="dictation-widget__error">
          <p className="dictation-widget__error-text">{dictationState.error}</p>
          {micPermissionError && (
            <button
              className="dictation-widget__button"
              onClick={handleRetryPermission}
              type="button"
            >
              Retry
            </button>
          )}
        </div>
      )
    }

    // Complete state (transient - widget will auto-hide)
    if (dictationState.status === 'complete') {
      if (dictationState.silent) {
        return (
          <div className="dictation-widget__complete">
            <p className="dictation-widget__complete-text">No speech detected</p>
          </div>
        )
      }
      return (
        <div className="dictation-widget__complete">
          <p className="dictation-widget__complete-text">✓ Transcribed</p>
        </div>
      )
    }

    // Listening state
    return (
      <div className="dictation-widget__listening">
        <WaveformAnimation />
        <p className="dictation-widget__label">Listening...</p>
        <button
          className="dictation-widget__button dictation-widget__button--done"
          onClick={handleDone}
          type="button"
        >
          Done
        </button>
      </div>
    )
  }

  return (
    <div className="dictation-widget">
      <div className="dictation-widget__body">{renderContent()}</div>
    </div>
  )
}

export default DictationWidget
