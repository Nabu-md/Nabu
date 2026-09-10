import { useCallback, useEffect, useState } from 'react'
import { ArrowsClockwise, Play, SquaresFour, Warning } from '@phosphor-icons/react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'
import type { TranslationKey, TranslationValues } from '../lib/i18n'
import { Button } from './ui/button'
import { SectionHeading, SettingsGroup, SettingsRow, SettingsSwitchRow } from './SettingsControls'
import { trackMiniAppCronJobRunNow } from '../lib/productAnalytics'

type Translate = (key: TranslationKey, values?: TranslationValues) => string

interface MiniAppCronJobView {
  app_id: string
  app_name: string
  vault_path: string
  schedule: string
  task: string
  target_note: string | null
}

interface MiniAppsSettingsSectionProps {
  t: Translate
  miniAppsEnabled: boolean
  setMiniAppsEnabled: (value: boolean) => void
  webAccessEnabled: boolean
  setWebAccessEnabled: (value: boolean) => void
}

/** Lists every cron job declared by installed mini-apps across vaults. */
function useMiniAppCronJobs(enabled: boolean): {
  jobs: MiniAppCronJobView[]
  loading: boolean
  refresh: () => void
} {
  // `null` means "no fetch resolved yet"; loading is derived, so the effect
  // never calls setState synchronously (react-hooks/immutability).
  const [jobs, setJobs] = useState<MiniAppCronJobView[] | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)

  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    const request = isTauri()
      ? invoke<MiniAppCronJobView[]>('list_mini_app_cron_jobs')
      : mockInvoke<MiniAppCronJobView[]>('list_mini_app_cron_jobs', {})
    request
      .then((registered) => {
        if (!cancelled) setJobs(Array.isArray(registered) ? registered : [])
      })
      .catch(() => {
        if (!cancelled) setJobs([])
      })
    return () => {
      cancelled = true
    }
  }, [enabled, refreshKey])

  const refresh = useCallback(() => {
    setJobs(null)
    setRefreshKey((current) => current + 1)
  }, [])
  return { jobs: jobs ?? [], loading: enabled && jobs === null, refresh }
}

/**
 * Mini-apps section: engine toggle, web-access toggle (with the RAM warning
 * the user asked for), and the scheduled-jobs list with a "Run now" action
 * that fires a hidden run window through the Rust scheduler command.
 */
export function MiniAppsSettingsSection(props: MiniAppsSettingsSectionProps) {
  const { t, miniAppsEnabled, setMiniAppsEnabled, webAccessEnabled, setWebAccessEnabled } = props
  const { jobs, loading, refresh } = useMiniAppCronJobs(miniAppsEnabled)
  const [runningJob, setRunningJob] = useState<string | null>(null)

  const runNow = (job: MiniAppCronJobView) => {
    const key = `${job.app_id}:${job.task}`
    setRunningJob(key)
    const request = isTauri()
      ? invoke('run_mini_app_cron_job', {
          vaultPath: job.vault_path,
          appId: job.app_id,
          task: job.task,
        })
      : mockInvoke('run_mini_app_cron_job', {})
    request
      .catch(() => undefined)
      .finally(() => setRunningJob((current) => (current === key ? null : current)))
    trackMiniAppCronJobRunNow(job.app_id)
  }

  return (
    <>
      <SectionHeading icon={<SquaresFour size={16} aria-hidden="true" />} title={t('settings.miniApps.title')} />
      <SettingsGroup>
        <SettingsSwitchRow
          label={t('settings.miniApps.enable')}
          description={t('settings.miniApps.enableDescription')}
          checked={miniAppsEnabled}
          onChange={setMiniAppsEnabled}
          testId="settings-mini-apps-enabled"
        />
        <SettingsSwitchRow
          label={t('settings.miniApps.webAccess')}
          description={t('settings.miniApps.webAccessDescription')}
          checked={webAccessEnabled}
          onChange={setWebAccessEnabled}
          testId="settings-mini-apps-web-access"
        />
        {webAccessEnabled && (
          <div
            className="flex items-start gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2"
            data-testid="settings-mini-apps-web-access-warning"
          >
            <Warning size={15} weight="fill" className="mt-0.5 shrink-0 text-amber-600" aria-hidden />
            <p className="text-[12px] leading-5 text-muted-foreground">
              {t('settings.miniApps.webAccessWarning')}
            </p>
          </div>
        )}
      </SettingsGroup>

      <SettingsGroup>
        <SettingsRow
          label={t('settings.miniApps.scheduledJobs')}
          description={t('settings.miniApps.scheduledJobsDescription')}
        >
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="gap-1.5"
            aria-label={t('settings.miniApps.refreshJobs')}
            data-testid="settings-mini-apps-refresh-jobs"
            onClick={refresh}
          >
            <ArrowsClockwise size={14} className={loading ? 'animate-spin' : undefined} aria-hidden />
            {t('settings.miniApps.refreshJobs')}
          </Button>
        </SettingsRow>
        {miniAppsEnabled && jobs.length > 0 && (
          <div className="rounded-md border border-border" data-testid="settings-mini-apps-cron-jobs">
            {jobs.map((job) => {
              const key = `${job.app_id}:${job.task}`
              return (
                <div
                  key={`${job.vault_path}:${key}`}
                  className="flex items-center justify-between gap-3 border-b border-border px-3 py-2 last:border-b-0"
                >
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium text-foreground">
                      {job.app_name}
                      <span className="ml-2 font-mono text-[11px] font-normal text-muted-foreground">{job.schedule}</span>
                    </div>
                    <div className="truncate text-[12px] text-muted-foreground">
                      {job.task}
                      {job.target_note ? ` → ${job.target_note}` : ''}
                    </div>
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="shrink-0 gap-1.5"
                    disabled={runningJob === key}
                    aria-label={t('settings.miniApps.runNow')}
                    data-testid={`settings-mini-apps-run-${job.app_id}-${job.task}`}
                    onClick={() => runNow(job)}
                  >
                    <Play size={13} weight="fill" aria-hidden />
                    {t('settings.miniApps.runNow')}
                  </Button>
                </div>
              )
            })}
          </div>
        )}
        {miniAppsEnabled && !loading && jobs.length === 0 && (
          <p className="px-3 text-[12px] text-muted-foreground" data-testid="settings-mini-apps-no-jobs">
            {t('settings.miniApps.noScheduledJobs')}
          </p>
        )}
      </SettingsGroup>
    </>
  )
}
