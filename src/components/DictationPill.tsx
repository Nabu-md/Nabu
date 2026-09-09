import { useCallback, useEffect, useRef, useState } from 'react'
import { ClipboardText, Image, Microphone, MicrophoneSlash, SpeakerHigh, X } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { trackEvent } from '../lib/telemetry'
import { writeClipboardText } from '../utils/clipboardText'
import {
  useClipboardCache,
  useFileDropZone,
  useSpeechRecognition,
} from '../hooks/useDictation'
import type { useFluidVoiceDictation } from '../hooks/useFluidVoiceDictation'
import { TtsPlaybackControls } from './TtsPlaybackControls'
import { useKokoroTts } from '../hooks/useKokoroTts'
import { useTtsTextSource } from '../hooks/useTtsTextSource'

export interface DictationPillProps {
  vaultPath: string | null
  enabled?: boolean
  position?: 'bottom-right' | 'bottom-left'
  opacity?: number
  /** When provided, FluidVoice becomes the selectable dictation backend. */
  fluidVoice?: ReturnType<typeof useFluidVoiceDictation>
}

const DICTATION_SHORTCUT_KEY = 'd'

function shortcutMatches(event: KeyboardEvent): boolean {
  const modifier = event.metaKey || event.ctrlKey
  return modifier && event.shiftKey && event.key.toLowerCase() === DICTATION_SHORTCUT_KEY
}

function positionStyle(position: 'bottom-right' | 'bottom-left') {
  return position === 'bottom-right'
    ? { right: '20px' }
    : { left: '20px' }
}

/**
 * Floating dictation pill. Clicking it opens a panel with live speech-to-text
 * (Web Speech API), the recent clipboard cache (click to restore), and a file
 * drop zone that captures dropped files into the vault. Toggle with Cmd+Shift+D.
 */
export function DictationPill({
  vaultPath,
  enabled = true,
  position = 'bottom-right',
  opacity = 0.85,
  fluidVoice,
}: DictationPillProps) {
  const [panelOpen, setPanelOpen] = useState(false)
  const fallbackSpeech = useSpeechRecognition()
  const speech = fluidVoice ? fluidVoice.webSpeech : fallbackSpeech
  // Prefer FluidVoice state (start/stop + status) over the raw Web Speech
  // hook when FluidVoice is the active backend.
  const usingFluidVoice = Boolean(fluidVoice?.installed && fluidVoice.backend === 'fluidvoice')
  const isListening = usingFluidVoice ? fluidVoice?.status === 'listening' : speech.isListening
  const clipboard = useClipboardCache()
  const dropZone = useFileDropZone(vaultPath)
  const panelRef = useRef<HTMLDivElement | null>(null)

  // Plan 4 §1.6C: when the user selects text anywhere, the pill switches from
  // microphone to speaker mode and reads the selection aloud with Kokoro.
  const tts = useKokoroTts()
  const ttsSource = useTtsTextSource({ tts, sourceId: 'selection' })
  const [hasSelection, setHasSelection] = useState(false)
  useEffect(() => {
    if (tts.status !== 'ready') return
    const syncSelection = () => {
      const selection = document.getSelection()
      setHasSelection(Boolean(selection && selection.toString().trim().length > 0))
    }
    document.addEventListener('selectionchange', syncSelection)
    return () => document.removeEventListener('selectionchange', syncSelection)
  }, [tts.status])
  const ttsMode = tts.status === 'ready' && hasSelection && !panelOpen && !isListening

  const togglePanel = useCallback(() => {
    setPanelOpen((current) => !current)
    if (speech.isListening) speech.stop()
  }, [speech])

  useEffect(() => {
    if (!enabled) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (shortcutMatches(event)) {
        event.preventDefault()
        togglePanel()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [enabled, togglePanel])

  useEffect(() => {
    if (!panelOpen) return
    const handlePointerDown = (event: PointerEvent) => {
      if (panelRef.current?.contains(event.target as Node)) return
      setPanelOpen(false)
    }
    window.addEventListener('pointerdown', handlePointerDown)
    return () => window.removeEventListener('pointerdown', handlePointerDown)
  }, [panelOpen])

  useEffect(() => {
    if (panelOpen) void clipboard.refresh()
  }, [panelOpen, clipboard])

  if (!enabled) return null

  const handleStartStop = () => {
    if (usingFluidVoice && fluidVoice) {
      if (isListening) {
        void fluidVoice.stopDictation()
      } else {
        void fluidVoice.startDictation()
      }
      trackEvent(isListening ? 'dictation_stopped' : 'dictation_started', { backend: 'fluidvoice' })
      return
    }
    if (speech.isListening) {
      speech.stop()
      trackEvent('dictation_stopped', { backend: 'web_speech' })
    } else {
      speech.start()
      trackEvent('dictation_started', { backend: 'web_speech' })
    }
  }

  const handleCopyTranscript = () => {
    if (!speech.text.trim()) return
    void writeClipboardText(speech.text.trim())
      .then(() => trackEvent('dictation_transcript_copied', { outcome: 'success' }))
      .catch(() => trackEvent('dictation_transcript_copied', { outcome: 'failed' }))
  }

  const handleRestoreEntry = (id: string) => {
    void clipboard.restore(id).then((restored) => {
      if (restored) trackEvent('dictation_clipboard_restored', { outcome: 'success' })
    })
  }

  return (
    <div
      className="fixed bottom-14 z-50 flex flex-col items-end gap-2"
      style={{ ...positionStyle(position), opacity }}
      data-testid="dictation-pill"
    >
      {panelOpen && (
        <div
          ref={panelRef}
          className="w-[320px] overflow-hidden rounded-xl border border-border bg-background text-foreground shadow-xl"
          data-testid="dictation-panel"
        >
          <div className="flex items-center justify-between border-b border-border px-3 py-2">
            <span className="text-[12px] font-semibold uppercase tracking-wide text-muted-foreground">
              Dictation
            </span>
            <span className="text-[11px] text-muted-foreground">⌘⇧D</span>
          </div>

          <div className="space-y-3 p-3">
            {/* Recording */}
            <div>
              <div className="mb-1.5 flex items-center gap-2">
                <Button
                  type="button"
                  variant={isListening ? 'default' : 'outline'}
                  size="sm"
                  className="gap-1.5"
                  onClick={handleStartStop}
                  disabled={!usingFluidVoice && !speech.supported}
                  data-testid="dictation-record-toggle"
                >
                  {isListening ? <MicrophoneSlash size={14} /> : <Microphone size={14} />}
                  {isListening ? 'Stop' : 'Record'}
                </Button>
                {isListening && (
                  <span className="flex items-center gap-1.5 text-[11px] text-red-500">
                    <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-red-500" />
                    Listening…
                  </span>
                )}
                {!usingFluidVoice && !speech.supported && (
                  <span className="text-[11px] text-muted-foreground">
                    Speech recognition unavailable in this webview
                  </span>
                )}
              </div>
              {speech.text.trim() ? (
                <div className="relative">
                  <p className="max-h-32 overflow-y-auto rounded-lg bg-[var(--hover)] px-3 py-2 text-[13px] leading-relaxed">
                    {speech.text}
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    className="absolute right-1 top-1"
                    onClick={handleCopyTranscript}
                    data-testid="dictation-copy-transcript"
                  >
                    Copy
                  </Button>
                </div>
              ) : (
                <p className="rounded-lg border border-dashed border-border px-3 py-2 text-[12px] text-muted-foreground">
                  Live transcription appears here while recording.
                </p>
              )}
            </div>

            {/* Clipboard cache */}
            <div>
              <div className="mb-1 flex items-center justify-between">
                <span className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                  Recent clipboard
                </span>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  onClick={() => void clipboard.refresh()}
                  data-testid="dictation-clipboard-refresh"
                >
                  Refresh
                </Button>
              </div>
              {clipboard.error ? (
                <p className="text-[11px] text-red-500">{clipboard.error}</p>
              ) : clipboard.entries.length === 0 ? (
                <p className="text-[11px] text-muted-foreground">
                  Nothing cached yet. Copy some text or an image to build the cache.
                </p>
              ) : (
                <ul className="max-h-36 space-y-0.5 overflow-y-auto" data-testid="dictation-clipboard-list">
                  {clipboard.entries.map((entry) => (
                    <li key={entry.id}>
                      <button
                        type="button"
                        className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12px] transition-colors hover:bg-[var(--hover)]"
                        title={entry.kind === 'image' ? 'Restore image source' : 'Restore text to clipboard'}
                        onClick={() => handleRestoreEntry(entry.id)}
                        data-testid={`dictation-clipboard-entry-${entry.id}`}
                      >
                        {entry.kind === 'image' ? (
                          <Image size={13} className="shrink-0 text-muted-foreground" aria-hidden />
                        ) : (
                          <ClipboardText size={13} className="shrink-0 text-muted-foreground" aria-hidden />
                        )}
                        <span className="truncate">{entry.preview}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            {/* Drop zone */}
            <div
              className={cn(
                'rounded-lg border border-dashed px-3 py-2 text-center text-[11px] text-muted-foreground transition-colors',
                dropZone.dragging && 'border-[var(--accent-blue)] bg-[var(--accent-blue)]/5 text-[var(--accent-blue)]',
              )}
              data-testid="dictation-drop-zone"
            >
              {dropZone.dragging ? 'Drop files to capture into the vault' : 'Drop files here to capture into the vault'}
              {dropZone.error && <p className="mt-1 text-red-500">{dropZone.error}</p>}
              {dropZone.captured.length > 0 && (
                <ul className="mt-1 space-y-0.5 text-left">
                  {dropZone.captured.map((path) => (
                    <li key={path} className="truncate" title={path}>
                      ✓ {path.split('/').pop()}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>
      )}

      {ttsMode && ttsSource.active && (
        <div className="mb-1">
          <TtsPlaybackControls tts={tts} compact />
        </div>
      )}

      {/* Oblong pill: wide when idle, expands with the panel open. */}
      <Button
        type="button"
        variant="default"
        size="sm"
        className={cn(
          'h-11 rounded-full px-5 shadow-lg transition-all',
          panelOpen ? 'w-11 px-0' : 'w-auto gap-2',
        )}
        aria-label={ttsMode ? 'Read selected text aloud' : 'Toggle dictation'}
        title={ttsMode ? 'Read aloud' : 'Dictation (⌘⇧D)'}
        aria-expanded={panelOpen}
        data-testid={ttsMode ? 'dictation-tts-toggle' : 'dictation-toggle'}
        onClick={() => {
          if (ttsMode) {
            void ttsSource.speak()
            return
          }
          togglePanel()
        }}
      >
        {panelOpen ? (
          <X size={18} />
        ) : ttsMode ? (
          <>
            <SpeakerHigh size={18} weight="fill" />
            <span className="text-[13px] font-medium">Read aloud</span>
          </>
        ) : (
          <>
            <Microphone size={18} weight="fill" />
            <span className="text-[13px] font-medium">Dictate</span>
          </>
        )}
      </Button>
    </div>
  )
}