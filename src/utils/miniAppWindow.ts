import { isTauri } from '../mock-tauri'

export const MINI_APP_WINDOW_PARAM = 'window=mini-app'

export interface MiniAppWindowParams {
  appId: string
  vaultPath: string | null
  notePath?: string
  noteTitle?: string
  context?: unknown
  /** Scheduled-task id when this window was opened by the cron scheduler. */
  cronTask?: string
  /** Optional note path the scheduled task writes to. */
  cronTarget?: string
}

export interface MiniAppContextPayload {
  note_path?: string
  note_title?: string
  vault_path?: string
  extra?: unknown
  /** Scheduled-task id when the window was opened by the cron scheduler. */
  cron_task?: string
  /** Optional note path the scheduled task writes to. */
  cron_target?: string
}

export function isMiniAppWindow(search = window.location.search): boolean {
  return new URLSearchParams(search).get('window') === 'mini-app'
}

export function readMiniAppWindowParams(search = window.location.search): MiniAppWindowParams | null {
  const params = new URLSearchParams(search)
  const appId = params.get('appId')
  if (!appId) return null

  let context: unknown
  const rawContext = params.get('context')
  if (rawContext) {
    try {
      context = JSON.parse(rawContext)
    } catch {
      context = undefined
    }
  }

  return {
    appId,
    vaultPath: params.get('vault'),
    notePath: params.get('note') ?? undefined,
    noteTitle: params.get('title') ?? undefined,
    context,
    cronTask: params.get('cronTask') ?? undefined,
    cronTarget: params.get('cronTarget') ?? undefined,
  }
}

export function buildMiniAppWindowUrl(
  appId: string,
  vaultPath: string,
  context: MiniAppContextPayload = {},
): string {
  const params = new URLSearchParams({
    window: 'mini-app',
    appId,
    vault: vaultPath,
  })
  if (context.note_path) params.set('note', context.note_path)
  if (context.note_title) params.set('title', context.note_title)
  if (context.extra !== undefined) params.set('context', JSON.stringify(context.extra))
  return `/?${params.toString()}`
}

/**
 * Resolves the URL for the mini-app's content iframe. Mini-app files are
 * served by the `nabu-mini-app://` URI protocol (registered in the Tauri
 * backend); outside Tauri (browser previews) the app cannot be served, so a
 * fallback URL is returned instead.
 */
export function miniAppFrameSource(appId: string, entrypointUrl = 'index.html'): string | null {
  if (!isTauri()) return null
  const entrypoint = entrypointUrl.trim().replace(/^\/+/, '') || 'index.html'
  return `http://nabu-mini-app.localhost/${encodeURIComponent(appId)}/${entrypoint.split('/').map(encodeURIComponent).join('/')}`
}

export function installMiniAppContext(context: MiniAppContextPayload | null | undefined): void {
  if (!context) return
  try {
    ;(window as Window & { miniAppContext?: unknown }).miniAppContext = {
      note_path: context.note_path ?? null,
      note_title: context.note_title ?? null,
      vault_path: context.vault_path ?? null,
      extra: context.extra ?? null,
      cron_task: context.cron_task ?? null,
      cron_target: context.cron_target ?? null,
    }
  } catch {
    // Best-effort: the iframe gets the context via URL params as well.
  }
}