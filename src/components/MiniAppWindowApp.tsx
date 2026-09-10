import { useEffect, useMemo, useState } from 'react'
import { CaretDown, CaretUp, SquaresFour, X } from '@phosphor-icons/react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri } from '../mock-tauri'
import {
  installMiniAppContext,
  miniAppFrameSource,
  readMiniAppWindowParams,
  type MiniAppContextPayload,
} from '../utils/miniAppWindow'
import { cleanupTauriEventListener, type TauriUnlisten } from '../utils/tauriEventCleanup'
import { trackMiniAppGatewayUsed } from '../lib/productAnalytics'

interface MiniAppConfig {
  id: string
  name: string
  icon: string | null
  entrypoint_url: string
  width: number
  height: number
  resizable: boolean
  allow_vault_access: boolean
}

const MINI_APP_VAULT_DATA_REQUEST_EVENT = 'mini-app-request-vault-data'

/** Posted by a scheduled mini-app when its cron task has finished. */
const MINI_APP_CRON_DONE_MESSAGE = 'mini-app-cron-done'

/** Safety net matching the scheduler-side budget in `mini_apps_cron.rs`. */
const CRON_RUN_WATCHDOG_MS = 10 * 60 * 1000

/** Gateway commands mini-apps may invoke; instrumented for adoption metrics. */
const GATEWAY_COMMANDS = new Set([
  'proxy_fetch',
  'scrape_selection',
  'get_current_activity',
  'fetch_rss_feed',
  'sync_email',
])

function postMiniAppContext(
  target: Window,
  params: ReturnType<typeof readMiniAppWindowParams>,
  requestId = '',
): void {
  target.postMessage(
    {
      type: 'mini-app-context',
      request_id: requestId,
      context: {
        note_path: params?.notePath ?? null,
        note_title: params?.noteTitle ?? null,
        vault_path: params?.vaultPath ?? null,
        extra: params?.context ?? null,
        cron_task: params?.cronTask ?? null,
        cron_target: params?.cronTarget ?? null,
      },
    },
    '*',
  )
}

interface MiniAppMcpCallMessage {
  type: 'mini-app-mcp-call'
  id: string
  method: string
  params: Record<string, unknown>
}

function isMiniAppMcpCallMessage(payload: unknown): payload is MiniAppMcpCallMessage {
  if (typeof payload !== 'object' || payload === null) return false
  const candidate = payload as Partial<MiniAppMcpCallMessage>
  return (
    candidate.type === 'mini-app-mcp-call'
    && typeof candidate.id === 'string'
    && typeof candidate.method === 'string'
    && typeof candidate.params === 'object'
    && candidate.params !== null
  )
}

async function runMiniAppMcpCall(
  message: MiniAppMcpCallMessage,
  vaultPath: string | null,
): Promise<unknown> {
  if (!vaultPath) throw new Error('Vault access is unavailable without an open vault')
  return invoke('mcp_tool_call', {
    tool: message.method,
    args: message.params,
    vaultPath,
  })
}

async function runMiniAppGatewayCommand(command: string, args: Record<string, unknown>): Promise<unknown> {
  trackMiniAppGatewayUsed(command === 'scrape_selection' ? 'scrape_selector' : (command as 'proxy_fetch' | 'get_current_activity' | 'fetch_rss_feed'))
  return invoke(command, args)
}

async function closeMiniAppWindow(): Promise<void> {
  if (!isTauri()) return
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close().catch(() => {})
}

async function toggleMiniAppDevTools(): Promise<void> {
  if (!isTauri()) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const windowLabel = getCurrentWindow().label
    await invoke<void>('open_mini_app_devtools', { label: windowLabel })
  } catch {
    // Devtools are unavailable in release builds; ignore.
  }
}

async function loadMiniAppConfig(
  appId: string,
  vaultPath: string | null,
): Promise<MiniAppConfig | null> {
  if (!isTauri()) return null
  try {
    return await invoke<MiniAppConfig>('get_mini_app_config', {
      id: appId,
      vault_path: vaultPath,
    })
  } catch {
    return null
  }
}

/**
 * The shell window for a standalone mini-app. Renders the app's HTML in a
 * sandboxed iframe served from the `nabu-mini-app://` protocol, exposes
 * window controls (close, toggle dev tools), and relays vault-data requests
 * from the app to the main window.
 */
export function MiniAppWindowApp() {
  const params = useMemo(() => readMiniAppWindowParams(), [])
  const [config, setConfig] = useState<MiniAppConfig | null>(null)
  const [devToolsOpen, setDevToolsOpen] = useState(false)
  const appId = params?.appId ?? ''
  const vaultPath = params?.vaultPath ?? null

  useEffect(() => {
    if (!appId) return
    installMiniAppContext({
      note_path: params?.notePath,
      note_title: params?.noteTitle,
      vault_path: params?.vaultPath ?? undefined,
      extra: params?.context,
      cron_task: params?.cronTask,
      cron_target: params?.cronTarget,
    } satisfies MiniAppContextPayload)

    let cancelled = false
    void loadMiniAppConfig(appId, vaultPath).then((loaded) => {
      if (!cancelled) setConfig(loaded)
    })
    return () => {
      cancelled = true
    }
  }, [appId, params, vaultPath])

  useEffect(() => {
    if (!isTauri()) return
    let unlisten: TauriUnlisten | undefined

    void import('@tauri-apps/api/event')
      .then(({ listen }) => listen<{ request_id: string }>(MINI_APP_VAULT_DATA_REQUEST_EVENT, (event) => {
        // Relay the packaged context back to the requesting mini-app iframe.
        const frame = document.querySelector<HTMLIFrameElement>('iframe[data-mini-app-frame]')
        if (!frame?.contentWindow) return
        postMiniAppContext(frame.contentWindow, params, event.payload.request_id)
      }))
      .then((nextUnlisten) => {
        unlisten = nextUnlisten
      })
      .catch(() => undefined)

    return () => cleanupTauriEventListener(unlisten)
  }, [params])

  // Mini-apps run in a sandboxed iframe (a different window), so the packaged
  // context is delivered via postMessage both on load and on request. When the
  // app declares `allow_vault_access: true`, MCP tool calls are relayed to the
  // Rust backend over invoke and the result is posted back to the iframe.
  // Gateway commands (proxy_fetch, scrape_selection, …) go out over direct
  // invoke so mediated network access works regardless of the vault-access
  // toggle; each use is instrumented via `miniapp_gateway_used`.
  useEffect(() => {
    const handleMessage = async (event: MessageEvent) => {
      const frame = document.querySelector<HTMLIFrameElement>('iframe[data-mini-app-frame]')
      if (!frame?.contentWindow) return
      if (event.source !== frame.contentWindow) return
      const payload = event.data as { type?: string; request_id?: string; command?: string; args?: Record<string, unknown> } | null
      if (payload?.type === 'mini-app-request-vault-data') {
        postMiniAppContext(frame.contentWindow, params, payload.request_id ?? '')
        return
      }
      if (payload?.type === MINI_APP_CRON_DONE_MESSAGE) {
        void closeMiniAppWindow()
        return
      }
      if (payload?.type === 'mini-app-gateway-call' && typeof payload.command === 'string' && GATEWAY_COMMANDS.has(payload.command)) {
        try {
          const result = await runMiniAppGatewayCommand(payload.command, payload.args ?? {})
          frame.contentWindow.postMessage(
            { type: 'mini-app-gateway-response', id: payload.request_id, result },
            '*',
          )
        } catch (error) {
          frame.contentWindow.postMessage(
            {
              type: 'mini-app-gateway-response',
              id: payload.request_id,
              error: error instanceof Error ? error.message : String(error),
            },
            '*',
          )
        }
        return
      }
      if (!config?.allow_vault_access || !isMiniAppMcpCallMessage(payload)) return
      try {
        const result = await runMiniAppMcpCall(payload, vaultPath)
        frame.contentWindow.postMessage(
          { type: 'mini-app-mcp-response', id: payload.id, result },
          '*',
        )
      } catch (error) {
        frame.contentWindow.postMessage(
          {
            type: 'mini-app-mcp-response',
            id: payload.id,
            error: error instanceof Error ? error.message : String(error),
          },
          '*',
        )
      }
    }
    window.addEventListener('message', handleMessage)
    return () => window.removeEventListener('message', handleMessage)
  }, [config?.allow_vault_access, params, vaultPath])

  const handleFrameLoad = () => {
    const frame = document.querySelector<HTMLIFrameElement>('iframe[data-mini-app-frame]')
    if (frame?.contentWindow) postMiniAppContext(frame.contentWindow, params)
  }

  const frameSrc = appId ? miniAppFrameSource(appId, config?.entrypoint_url) : null
  const isCronRun = Boolean(params?.cronTask)

  // Scheduled-run watchdog: hidden cron windows must never linger. Close the
  // window when the app reports completion (handled in the message listener
  // above) or after the watchdog elapses, whichever comes first.
  useEffect(() => {
    if (!isCronRun || !isTauri()) return
    const timer = window.setTimeout(() => {
      void closeMiniAppWindow()
    }, CRON_RUN_WATCHDOG_MS)
    return () => window.clearTimeout(timer)
  }, [isCronRun])

  return (
    <div className="flex h-full w-full flex-col overflow-hidden bg-background text-foreground">
      <header
        className="flex h-10 shrink-0 items-center gap-2 border-b border-border px-3"
        data-testid="mini-app-chrome"
      >
        <SquaresFour size={15} className="shrink-0 text-muted-foreground" aria-hidden />
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium" title={config?.name ?? appId}>
          {config?.name ?? (appId || 'Mini App')}
        </span>
        {isTauri() && (
          <div className="flex shrink-0 items-center gap-1">
            <button
              type="button"
              className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-[var(--hover)] hover:text-foreground"
              aria-label={devToolsOpen ? 'Close developer tools' : 'Open developer tools'}
              title="Toggle developer tools"
              data-testid="mini-app-dev-tools"
              onClick={() => {
                setDevToolsOpen((current) => !current)
                void toggleMiniAppDevTools()
              }}
            >
              {devToolsOpen ? <CaretDown size={15} /> : <CaretUp size={15} />}
            </button>
            <button
              type="button"
              className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-red-500/10 hover:text-red-500"
              aria-label="Close mini-app"
              title="Close"
              data-testid="mini-app-close"
              onClick={() => void closeMiniAppWindow()}
            >
              <X size={15} />
            </button>
          </div>
        )}
      </header>
      <div className="min-h-0 flex-1">
        {frameSrc ? (
          <iframe
            data-mini-app-frame
            src={frameSrc}
            className="h-full w-full border-0"
            sandbox="allow-scripts allow-same-origin allow-popups allow-forms allow-modals"
            referrerPolicy="no-referrer"
            title={`${config?.name ?? appId} mini-app`}
            onLoad={handleFrameLoad}
          />
        ) : (
          <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted-foreground">
            <div>
              <p className="mb-2 font-medium text-foreground">Mini-apps require the desktop app</p>
              <p>The {appId || 'requested'} mini-app can only run inside Nabu. Open it from the main window.</p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}