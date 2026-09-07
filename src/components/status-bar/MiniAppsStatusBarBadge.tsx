import { useState } from 'react'
import { SquaresFour } from '@phosphor-icons/react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useMiniApps, type MiniApp } from '../../hooks/useMiniApps'
import { trackEvent } from '../../lib/telemetry'
import { StatusBarAction, StatusBarSeparator } from './StatusBarBadges'
import { ICON_STYLE } from './styles'

interface MiniAppsStatusBarBadgeProps {
  vaultPath: string | null
  activeNote?: { path?: string | null; title?: string | null } | null
  onToast?: (message: string) => void
  showSeparator?: boolean
  compact?: boolean
}

function MiniAppRow({ app, onOpen }: { app: MiniApp; onOpen: (app: MiniApp) => void }) {
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
 * Bottom-ribbon mini apps launcher (Phase 4: replaces the floating
 * MiniAppsLauncher button). Opens the same app-list popover in its own window.
 */
export function MiniAppsStatusBarBadge({
  vaultPath,
  activeNote,
  onToast,
  showSeparator = true,
  compact = false,
}: MiniAppsStatusBarBadgeProps) {
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
      if (!label && typeof window !== 'undefined' && !('__TAURI__' in window || '__TAURI_INTERNALS__' in window)) {
        onToast?.('Mini-apps require the desktop app')
      }
    })
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) void refresh()
      }}
    >
      <StatusBarSeparator show={showSeparator} />
      <PopoverTrigger asChild>
        <StatusBarAction
          copy={{ label: 'Mini Apps' }}
          onClick={() => setOpen((current) => !current)}
          testId="mini-apps-launcher"
          compact={compact}
        >
          <span style={ICON_STYLE}>
            <SquaresFour size={13} />
            {compact ? null : 'Mini Apps'}
          </span>
        </StatusBarAction>
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
