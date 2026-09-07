import { useState } from 'react'
import { SquaresFour } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useMiniApps, type MiniApp } from '../hooks/useMiniApps'
import { trackEvent } from '../lib/telemetry'

interface MiniAppsLauncherProps {
  vaultPath: string | null
  activeNote?: { path?: string | null; title?: string | null } | null
  onToast?: (message: string) => void
}

function MiniAppRow({
  app,
  onOpen,
}: {
  app: MiniApp
  onOpen: (app: MiniApp) => void
}) {
  return (
    <button
      type="button"
      className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors hover:bg-[var(--hover)]"
      data-testid={`mini-app-${app.id}`}
      onClick={() => onOpen(app)}
    >
      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-[var(--accent-blue)]/10 text-[var(--accent-blue)]">
        <SquaresFour size={16} aria-hidden />
      </span>
      <span className="min-w-0">
        <span className="block truncate text-[13px] font-medium text-foreground">{app.name}</span>
        <span className="block truncate text-[11px] text-muted-foreground">{app.id}</span>
      </span>
    </button>
  )
}

/**
 * Floating launcher for installed HTML mini-apps (a dock item for the
 * Command Palette alternative described in the Phase 3 plan). Clicking an app
 * opens it in its own Tauri window with the current note as context.
 */
export function MiniAppsLauncher({ vaultPath, activeNote, onToast }: MiniAppsLauncherProps) {
  const [open, setOpen] = useState(false)
  const { apps, loading, error, refresh, openMiniApp, createMiniAppContextFromCurrentNote } = useMiniApps({
    vaultPath,
    enabled: open,
  })

  const handleOpen = (app: MiniApp) => {
    trackEvent('mini_app_opened', { app_id: app.id })
    const context = createMiniAppContextFromCurrentNote({
      notePath: activeNote?.path ?? null,
      noteTitle: activeNote?.title ?? null,
    })
    void openMiniApp(app.id, context).then((label) => {
      setOpen(false)
      if (!label && !isDesktopRuntime()) {
        onToast?.('Mini-apps require the desktop app')
      }
    })
  }

  return (
    <Popover open={open} onOpenChange={(next) => {
      setOpen(next)
      if (next) void refresh()
    }}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="fixed bottom-11 right-40 z-30 gap-2 rounded-full border border-border/60 bg-background/80 px-3 text-[12px] text-muted-foreground shadow-[0_10px_28px_rgba(15,23,42,0.18),0_2px_8px_rgba(15,23,42,0.12)] backdrop-blur hover:text-foreground"
          aria-label="Mini apps"
          title="Mini apps"
          data-testid="mini-apps-launcher"
        >
          <SquaresFour size={14} weight="fill" aria-hidden />
          <span className="hidden sm:inline">Mini Apps</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" side="top" className="w-64 p-1.5">
        <div className="px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          Mini Apps
        </div>
        {loading ? (
          <div className="px-3 py-3 text-[12px] text-muted-foreground">Loading…</div>
        ) : error ? (
          <div className="px-3 py-3 text-[12px] text-red-500">{error}</div>
        ) : apps.length === 0 ? (
          <div className="px-3 py-3 text-[12px] text-muted-foreground">
            No mini-apps installed. Drop an app folder into <code className="text-foreground">.apps/</code> in your vault.
          </div>
        ) : (
          <div className="flex flex-col gap-0.5">
            {apps.map((app) => (
              <MiniAppRow key={app.id} app={app} onOpen={handleOpen} />
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  )
}

function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && ('__TAURI__' in window || '__TAURI_INTERNALS__' in window)
}