import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

export interface MiniAppMeta {
  app_id: string
  title: string
  note_path: string
}

export interface MiniAppView {
  name: string
  view_type: 'table' | 'form' | 'kanban' | 'chart'
  query: string
  group_by: string | null
  fields: MiniAppFormField[]
}

export interface MiniAppFormField {
  name: string
  column: string
  field_type: 'text' | 'number' | 'email' | 'date' | 'select' | 'textarea'
  required: boolean
  options: string[]
}

export interface MiniAppDefinition {
  app_id: string
  title: string
  note_path: string
  schema: string
  views: MiniAppView[]
}

export type MiniAppRow = Record<string, unknown>

function miniAppInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

interface UseMiniAppOptions {
  vaultPath: string | null
  /** When null, no app is loaded (launcher overview only). */
  notePath: string | null
  enabled?: boolean
}

/**
 * Loads a DuckDB-backed mini-app from a markdown note (`app_id` frontmatter)
 * and drives its views: run SELECTs, submit form inserts, export/import as
 * markdown tables. All SQL stays inside the app's own `.nabu/apps/` database.
 */
export function useMiniApp({ vaultPath, notePath, enabled = true }: UseMiniAppOptions) {
  const [app, setApp] = useState<MiniAppDefinition | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    if (!vaultPath || !notePath) {
      setApp(null)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const definition = await miniAppInvoke<MiniAppDefinition>('get_mini_app', {
        vaultPath,
        notePath,
      })
      setApp(definition)
    } catch (cause) {
      setApp(null)
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setLoading(false)
    }
  }, [notePath, vaultPath])

  useEffect(() => {
    if (enabled) void load()
  }, [enabled, load])

  const runView = useCallback(
    async (viewName: string): Promise<MiniAppRow[]> => {
      if (!vaultPath || !notePath) return []
      return miniAppInvoke<MiniAppRow[]>('run_mini_app_view', {
        vaultPath,
        notePath,
        viewName,
      })
    },
    [notePath, vaultPath],
  )

  const executeForm = useCallback(
    async (viewName: string, values: Record<string, string>): Promise<void> => {
      if (!vaultPath || !notePath) return
      await miniAppInvoke('execute_mini_app_mut', {
        vaultPath,
        notePath,
        viewName,
        values,
      })
    },
    [notePath, vaultPath],
  )

  const exportMarkdown = useCallback(
    async (viewName: string): Promise<string> => {
      if (!vaultPath || !notePath) return ''
      return miniAppInvoke<string>('export_mini_app_to_markdown', {
        vaultPath,
        notePath,
        viewName,
      })
    },
    [notePath, vaultPath],
  )

  const importRows = useCallback(
    async (viewName: string, rows: Array<Record<string, string>>): Promise<number> => {
      if (!vaultPath || !notePath) return 0
      return miniAppInvoke<number>('import_mini_app_markdown_cmd', {
        vaultPath,
        notePath,
        viewName,
        rows,
      })
    },
    [notePath, vaultPath],
  )

  const discover = useCallback(async (): Promise<MiniAppMeta[]> => {
    if (!vaultPath) return []
    return miniAppInvoke<MiniAppMeta[]>('discover_mini_apps', { vaultPath })
  }, [vaultPath])

  return { app, loading, error, reload: load, runView, executeForm, exportMarkdown, importRows, discover }
}
