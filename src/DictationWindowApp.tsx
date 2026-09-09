import { useCallback, useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri, mockInvoke } from './mock-tauri'
import { DictationPill } from './components/DictationPill'
import { Button } from './components/ui/button'
import { X } from '@phosphor-icons/react'
import { useFluidVoiceDictation } from './hooks/useFluidVoiceDictation'

/** Loads the user's dictation opacity so the standalone window honors it. */
async function loadDictationOpacity(): Promise<number> {
  try {
    const settings = await (isTauri()
      ? import('@tauri-apps/api/core').then(({ invoke }) => invoke<Record<string, unknown>>('get_settings'))
      : mockInvoke<Record<string, unknown>>('get_settings'))
    const opacity = settings?.dictation_opacity
    return typeof opacity === 'number' && opacity > 0 && opacity <= 1 ? opacity : 1
  } catch {
    return 1
  }
}

/**
 * Root component for the standalone dictation window (frameless, always on
 * top). The window itself IS the pill; a drag region replaces the title bar.
 */
export function DictationWindowApp() {
  const dictation = useFluidVoiceDictation()
  const [opacity, setOpacity] = useState(1)

  useEffect(() => {
    let cancelled = false
    void loadDictationOpacity().then((value) => {
      if (!cancelled) setOpacity(value)
    })
    return () => {
      cancelled = true
    }
  }, [])

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
        <DictationPill vaultPath={null} opacity={opacity} fluidVoice={dictation} />
      </div>
    </div>
  )
}
