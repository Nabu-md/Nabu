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

import React, { useCallback } from 'react'
import { useWidgetDictation } from './widgetService'

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
  const { state: dictationState, start, stop } = useWidgetDictation()

  // Microphone permission error is derived directly from dictation state —
  // no separate state or effect needed.
  const micPermissionError =
    dictationState.status === 'error' && !!dictationState.error?.includes('Microphone access')

  // Handle "Done" button click
  const handleDone = useCallback(async () => {
    try {
      await stop()
    } catch (err) {
      console.error('[DictationWidget] Failed to stop dictation:', err)
    }
  }, [stop])

  // Handle microphone permission retry
  const handleRetryPermission = useCallback(async () => {
    try {
      await start()
    } catch (err) {
      console.error('[DictationWidget] Failed to start dictation:', err)
    }
  }, [start])

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
