import { useCallback, useEffect, useMemo, useState } from 'react'
import { Download } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { createTranslator, type AppLocale } from '../lib/i18n'
import { trackEvent } from '../lib/telemetry'
import { useMiniApp, type MiniAppFormField, type MiniAppRow, type MiniAppView } from '../hooks/useMiniApp'

export interface MiniAppRendererProps {
  vaultPath: string
  /** Path of the markdown note that defines the app (app_id frontmatter). */
  notePath: string
  locale?: AppLocale
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return ''
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

/** Table view: renders the DuckDB rows returned by the view query. */
function MiniAppTableView({ data }: { data: MiniAppRow[] }) {
  const columns = useMemo(() => {
    const seen: string[] = []
    for (const row of data.slice(0, 50)) {
      for (const key of Object.keys(row)) {
        if (!seen.includes(key)) seen.push(key)
      }
    }
    return seen
  }, [data])

  if (data.length === 0) {
    return <p className="p-4 text-[13px] text-muted-foreground">No records yet.</p>
  }

  return (
    <table className="w-full border-collapse text-[13px]" data-testid="mini-app-table">
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column} className="border-b border-border px-3 py-2 text-left font-semibold text-muted-foreground">
              {column}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {data.map((row, index) => (
          <tr key={index} className="hover:bg-[var(--hover)]">
            {columns.map((column) => (
              <td key={column} className="border-b border-border/60 px-3 py-1.5">
                {formatCell(row[column])}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/** Kanban view: groups rows by the view's group_by column. */
function MiniAppKanbanView({ data, groupBy }: { data: MiniAppRow[]; groupBy: string | null }) {
  const groups = useMemo(() => {
    const map = new Map<string, MiniAppRow[]>()
    for (const row of data) {
      const key = formatCell(groupBy ? row[groupBy] : 'All') || '—'
      const bucket = map.get(key) ?? []
      bucket.push(row)
      map.set(key, bucket)
    }
    return [...map.entries()]
  }, [data, groupBy])

  const nameOf = (row: MiniAppRow) => formatCell(row.name ?? row.title ?? row.id ?? Object.values(row)[0])

  return (
    <div className="flex gap-3 overflow-x-auto p-4" data-testid="mini-app-kanban">
      {groups.map(([group, rows]) => (
        <div key={group} className="flex min-w-[220px] flex-1 flex-col gap-2 rounded-lg bg-[var(--hover)]/50 p-2">
          <div className="flex items-center justify-between px-1 text-[12px] font-semibold text-muted-foreground">
            <span>{group}</span>
            <span>{rows.length}</span>
          </div>
          {rows.map((row, index) => (
            <div key={index} className="rounded-md border border-border bg-background px-2.5 py-2 text-[13px]">
              {nameOf(row)}
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

/** Form view: collects values for the view's declared fields and inserts them. */
function MiniAppFormView({
  fields,
  submitting,
  submitLabel,
  requiredLabel,
  onSubmit,
}: {
  fields: MiniAppFormField[]
  submitting: boolean
  submitLabel: string
  requiredLabel: string
  onSubmit: (values: Record<string, string>) => void
}) {
  const [values, setValues] = useState<Record<string, string>>({})
  const [errors, setErrors] = useState<Record<string, boolean>>({})

  const handleSubmit = () => {
    const nextErrors: Record<string, boolean> = {}
    for (const field of fields) {
      if (field.required && !(values[field.column] ?? '').trim()) nextErrors[field.column] = true
    }
    setErrors(nextErrors)
    if (Object.keys(nextErrors).length > 0) return
    onSubmit(values)
    setValues({})
  }

  return (
    <form
      className="flex max-w-md flex-col gap-3 p-4"
      data-testid="mini-app-form"
      onSubmit={(event) => {
        event.preventDefault()
        handleSubmit()
      }}
    >
      {fields.map((field) => (
        <label key={field.column} className="flex flex-col gap-1 text-[12px]">
          <span className="font-medium text-foreground">
            {field.name}
            {field.required && <span className="text-red-500"> *</span>}
          </span>
          {field.field_type === 'select' ? (
            <select
              className="h-8 rounded-md border border-border bg-background px-2 text-[13px]"
              value={values[field.column] ?? ''}
              onChange={(event) => setValues((current) => ({ ...current, [field.column]: event.target.value }))}
            >
              <option value="">—</option>
              {field.options.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          ) : field.field_type === 'textarea' ? (
            <textarea
              className="min-h-20 rounded-md border border-border bg-background px-2 py-1.5 text-[13px]"
              value={values[field.column] ?? ''}
              onChange={(event) => setValues((current) => ({ ...current, [field.column]: event.target.value }))}
            />
          ) : (
            <input
              className={cn(
                'h-8 rounded-md border border-border bg-background px-2 text-[13px]',
                errors[field.column] && 'border-red-500',
              )}
              type={field.field_type === 'number' ? 'number' : field.field_type === 'email' ? 'email' : field.field_type === 'date' ? 'date' : 'text'}
              value={values[field.column] ?? ''}
              onChange={(event) => setValues((current) => ({ ...current, [field.column]: event.target.value }))}
            />
          )}
          {errors[field.column] && <span className="text-[11px] text-red-500">{requiredLabel}</span>}
        </label>
      ))}
      <Button type="submit" size="sm" className="self-start" disabled={submitting}>
        {submitLabel}
      </Button>
    </form>
  )
}

/**
 * Renders a DuckDB-backed mini-app defined by a markdown note (plan 4 §3.7):
 * view tabs (table/kanban/form/chart), data loading, form inserts, and
 * markdown export.
 */
export function MiniAppRenderer({ vaultPath, notePath, locale = 'en' }: MiniAppRendererProps) {
  const t = createTranslator(locale)
  const { app, loading, error, reload, runView, executeForm, exportMarkdown } = useMiniApp({ vaultPath, notePath })
  const [activeView, setActiveView] = useState('')
  const [data, setData] = useState<MiniAppRow[] | null>(null)
  const [viewLoading, setViewLoading] = useState(false)
  const [viewError, setViewError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    setActiveView(app?.views[0]?.name ?? '')
  }, [app])

  useEffect(() => {
    if (!activeView) return
    let cancelled = false
    setViewLoading(true)
    setViewError(null)
    runView(activeView)
      .then((rows) => {
        if (!cancelled) setData(rows)
      })
      .catch((cause) => {
        if (!cancelled) {
          setData([])
          setViewError(cause instanceof Error ? cause.message : String(cause))
        }
      })
      .finally(() => {
        if (!cancelled) setViewLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [activeView, runView])

  const view: MiniAppView | undefined = app?.views.find((candidate) => candidate.name === activeView)

  const handleSubmit = useCallback(
    async (values: Record<string, string>) => {
      if (!activeView) return
      setSubmitting(true)
      try {
        await executeForm(activeView, values)
        trackEvent('mini_app_record_created', { app_id: app?.app_id ?? '', view_name: activeView })
        const rows = await runView(activeView)
        setData(rows)
      } catch (cause) {
        setViewError(cause instanceof Error ? cause.message : String(cause))
      } finally {
        setSubmitting(false)
      }
    },
    [activeView, app?.app_id, executeForm, runView],
  )

  const handleExport = useCallback(async () => {
    if (!activeView || !app) return
    try {
      const markdown = await exportMarkdown(activeView)
      if (!markdown) return
      const { writeClipboardText } = await import('../utils/clipboardText')
      await writeClipboardText(markdown)
      trackEvent('mini_app_data_exported', { app_id: app.app_id, format: 'markdown' })
    } catch {
      // Clipboard failures are non-fatal.
    }
  }, [activeView, app, exportMarkdown])

  if (loading) return <div className="p-4 text-[13px] text-muted-foreground">{t('miniApp.loading')}</div>
  if (error) return <div className="p-4 text-[13px] text-red-500">{error}</div>
  if (!app) return <div className="p-4 text-[13px] text-muted-foreground">{t('miniApp.notFound')}</div>

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="mini-app-renderer">
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
          {app.views.map((candidate) => (
            <Button
              key={candidate.name}
              type="button"
              variant={candidate.name === activeView ? 'default' : 'ghost'}
              size="sm"
              onClick={() => setActiveView(candidate.name)}
            >
              {candidate.name}
            </Button>
          ))}
        </div>
        <div className="flex items-center gap-1">
          <Button type="button" variant="ghost" size="sm" onClick={() => void reload()} title="Reload">
            Reload
          </Button>
          <Button type="button" variant="ghost" size="sm" onClick={() => void handleExport()} data-testid="mini-app-export">
            <Download size={14} aria-hidden />
            {t('miniApp.exportMarkdown')}
          </Button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {viewLoading && <p className="p-4 text-[13px] text-muted-foreground">{t('miniApp.loading')}</p>}
        {viewError && <p className="p-4 text-[13px] text-red-500">{viewError}</p>}
        {!viewLoading && !viewError && view?.view_type === 'table' && <MiniAppTableView data={data ?? []} />}
        {!viewLoading && !viewError && view?.view_type === 'kanban' && (
          <MiniAppKanbanView data={data ?? []} groupBy={view.group_by} />
        )}
        {!viewLoading && !viewError && view?.view_type === 'form' && (
          <MiniAppFormView
            fields={view.fields}
            submitting={submitting}
            submitLabel={t('miniApp.save')}
            requiredLabel={t('miniApp.requiredField')}
            onSubmit={(values) => void handleSubmit(values)}
          />
        )}
        {!viewLoading && !viewError && view?.view_type === 'chart' && (
          <p className="p-4 text-[13px] text-muted-foreground">
            {`Chart view "${view.name}" has no renderer yet — showing ${data?.length ?? 0} rows.`}
          </p>
        )}
      </div>
      <div className="border-t border-border px-3 py-1.5 text-[11px] text-muted-foreground">
        {t('miniApp.records', { count: data?.length ?? 0 })}
      </div>
    </div>
  )
}
