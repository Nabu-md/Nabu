import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'
import type { MiniAppContextPayload } from '../utils/miniAppWindow'

export interface MiniApp {
  id: string
  name: string
  icon: string | null
  entrypoint_url: string
  width: number
  height: number
  resizable: boolean
  allow_vault_access: boolean
}

export interface MiniAppContext {
  note_path?: string
  note_title?: string
  vault_path?: string
  extra?: unknown
}

export interface MiniAppsState {
  apps: MiniApp[]
  loading: boolean
  error: string | null
}

function miniAppInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

function packageContext(
  vaultPath: string,
  options: { notePath?: string | null; noteTitle?: string | null; extra?: unknown } = {},
): MiniAppContextPayload {
  const context: MiniAppContextPayload = {
    vault_path: vaultPath,
  }
  if (options.notePath) context.note_path = options.notePath
  if (options.noteTitle) context.note_title = options.noteTitle
  if (options.extra !== undefined) context.extra = options.extra
  return context
}

/**
 * Lists installed mini-apps and opens them in their own Tauri window.
 * In browser mode the list resolves to the bundled sample apps via the mock
 * invoke layer, and opening a window is a no-op (the desktop app is required).
 */
export function useMiniApps({
  vaultPath,
  enabled = true,
}: {
  vaultPath: string | null
  enabled?: boolean
}) {
  const [state, setState] = useState<MiniAppsState>({ apps: [], loading: false, error: null })

  const refresh = useCallback(async () => {
    if (!enabled) return
    if (!vaultPath) {
      setState({ apps: [], loading: false, error: null })
      return
    }
    setState((current) => ({ ...current, loading: true, error: null }))
    try {
      const apps = await miniAppInvoke<MiniApp[]>('list_mini_apps', { vault_path: vaultPath })
      setState({ apps, loading: false, error: null })
    } catch (error) {
      setState({
        apps: [],
        loading: false,
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }, [enabled, vaultPath])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const openMiniApp = useCallback(
    async (id: string, context: MiniAppContextPayload = {}) => {
      if (!isTauri()) return null
      if (!vaultPath) return null
      try {
        return await miniAppInvoke<string>('open_mini_app_window', {
          id,
          context: {
            ...packageContext(vaultPath),
            ...context,
          },
        })
      } catch (error) {
        setState((current) => ({
          ...current,
          error: error instanceof Error ? error.message : String(error),
        }))
        return null
      }
    },
    [vaultPath],
  )

  const createMiniAppContextFromCurrentNote = useCallback(
    (options: { notePath?: string | null; noteTitle?: string | null; extra?: unknown } = {}): MiniAppContextPayload => {
      return vaultPath ? packageContext(vaultPath, options) : {}
    },
    [vaultPath],
  )

  return useMemo(
    () => ({
      ...state,
      refresh,
      openMiniApp,
      createMiniAppContextFromCurrentNote,
    }),
    [state, refresh, openMiniApp, createMiniAppContextFromCurrentNote],
  )
}