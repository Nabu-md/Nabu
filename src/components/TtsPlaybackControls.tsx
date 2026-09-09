import { forwardRef } from 'react'
import {
  ArrowClockwise,
  ArrowCounterClockwise,
  Pause,
  Play,
  SpeakerHigh,
  SpeakerSlash,
} from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { KokoroTtsState } from '../hooks/useKokoroTts'

interface TtsPlaybackControlsProps {
  tts: KokoroTtsState
  /** Compact inline variant (e.g. under the dictation pill). */
  compact?: boolean
  className?: string
}

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '0:00'
  const total = Math.floor(seconds)
  const minutes = Math.floor(total / 60)
  const remainder = total % 60
  return `${minutes}:${String(remainder).padStart(2, '0')}`
}

/**
 * Speechify-style playback bar for Kokoro TTS: progress, ±15s, play/pause,
 * stop, plus speed/voice readout. Floats over the editor or docks inline
 * under the dictation pill (`compact`).
 */
export const TtsPlaybackControls = forwardRef<HTMLDivElement, TtsPlaybackControlsProps>(
  function TtsPlaybackControls({ tts, compact = false, className }, ref) {
    const { isPlaying, isPaused, currentTime, duration, speed } = tts
    const progress = duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0
    const active = isPlaying || isPaused

    return (
      <div
        ref={ref}
        className={cn(
          'flex items-center gap-1.5 rounded-xl border border-border bg-background/95 px-3 py-2 shadow-lg backdrop-blur',
          compact ? 'w-[280px]' : 'w-[420px]',
          className,
        )}
        data-testid="tts-playback-controls"
      >
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="Back 15 seconds"
          title="Back 15 seconds"
          onClick={tts.rewind15}
          disabled={!active}
        >
          <ArrowCounterClockwise size={14} />
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={isPlaying ? 'Pause' : 'Play'}
          title={isPlaying ? 'Pause' : 'Play'}
          onClick={() => (isPlaying ? tts.pause() : tts.resume())}
          disabled={!active}
        >
          {isPlaying ? <Pause size={15} weight="fill" /> : <Play size={15} weight="fill" />}
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="Forward 15 seconds"
          title="Forward 15 seconds"
          onClick={tts.forward15}
          disabled={!active}
        >
          <ArrowClockwise size={14} />
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="Stop reading"
          title="Stop reading"
          onClick={tts.stop}
          disabled={!active}
        >
          {active ? <SpeakerSlash size={15} /> : <SpeakerHigh size={15} />}
        </Button>

        <div className="min-w-0 flex-1">
          <div
            className="h-1.5 cursor-pointer overflow-hidden rounded-full bg-[var(--hover)]"
            role="slider"
            aria-label="Playback position"
            aria-valuemin={0}
            aria-valuemax={Math.round(duration)}
            aria-valuenow={Math.round(currentTime)}
            onClick={(event) => {
              const bounds = event.currentTarget.getBoundingClientRect()
              const fraction = (event.clientX - bounds.left) / bounds.width
              tts.seek(fraction * duration)
            }}
          >
            <div
              className="h-full rounded-full bg-[var(--accent-blue)] transition-[width] duration-150"
              style={{ width: `${progress}%` }}
            />
          </div>
          {!compact && (
            <div className="mt-1 flex items-center justify-between text-[10px] text-muted-foreground">
              <span data-testid="tts-elapsed">{formatTime(currentTime)}</span>
              <span>
                {tts.voice} · {speed}x
              </span>
              <span data-testid="tts-duration">{formatTime(duration)}</span>
            </div>
          )}
        </div>
      </div>
    )
  },
)
