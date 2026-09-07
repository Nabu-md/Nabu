import { useCallback } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri } from './mock-tauri'
import { DictationPill } from './components/DictationPill'
import { Button } from './components/ui/button'
import { X } from '@phosphor-icons/react'
import { useFluidVoiceDictation } from './hooks/useFluidVoiceDictation'

/**
 * Root component for the standalone dictation window (frameless, always on
 * top). The window itself IS the pill; a drag region replaces the title bar.
 */
export function DictationWindowApp() {
  const dictation = useFluidVoiceDictation()

  const closeWindow = useCallback(() => {
    if (isTauri()) {
      void getCurrentWindow().close()
    } else {
      window.close()
    }
  }, [])

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden rounded-xl border border-border/40 bg-background/95 shadow-2xl">
      {/* Drag region + close button */}
      <div
        data-tauri-drag-region
        className="flex h-9 shrink-0 items-center justify-end border-b border-border/30 bg-muted/40 px-2"
      >
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6"
          aria-label="Close dictation window"
          data-testid="dictation-window-close"
          onClick={closeWindow}
        >
          <X className="h-4 w-4" />
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-3">
        <DictationPill vaultPath={null} opacity={1} fluidVoice={dictation} />
      </div>
    </div>
  )
}
