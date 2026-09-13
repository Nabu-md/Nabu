import { useEffect, useState } from 'react'
import type { McpStatus } from '../hooks/useMcpStatus'
import type { ThemeMode } from '../lib/themeMode'
import type { AppLocale } from '../lib/i18n'
import type { GitRemoteStatus, SyncStatus } from '../types'
import { TooltipProvider } from '@/components/ui/tooltip'
import type { GitRepositoryOption } from '../utils/gitRepositories'
import { StatusBarPrimarySection, StatusBarSecondarySection } from './status-bar/StatusBarSections'
import type { VaultOption } from './status-bar/types'

export type { VaultOption } from './status-bar/types'

const COMPACT_STATUS_BAR_MAX_WIDTH = 1000
const STATUS_BAR_STACKING_Z_INDEX = 30

function getWindowWidth() {
  return typeof window === 'undefined' ? Number.POSITIVE_INFINITY : window.innerWidth
}

function getStatusBarLayout(windowWidth: number) {
  const compact = windowWidth <= COMPACT_STATUS_BAR_MAX_WIDTH

  return {
    compact,
    stacked: false,
  }
}

function useStatusBarTicker() {
  const [, setTick] = useState(0)

  useEffect(() => {
    const id = setInterval(() => setTick((tick) => tick + 1), 30_000)
    return () => clearInterval(id)
  }, [])
}

function useStatusBarLayout() {
  const [windowWidth, setWindowWidth] = useState(() => getWindowWidth())

  useEffect(() => {
    if (typeof window === 'undefined') return

    const handleResize = () => setWindowWidth(getWindowWidth())

    window.addEventListener('resize', handleResize)
    return () => window.removeEventListener('resize', handleResize)
  }, [])

  return getStatusBarLayout(windowWidth)
}

interface StatusBarProps {
  noteCount: number
  modifiedCount?: number
  vaultPath: string
  defaultWorkspacePath?: string | null
  vaults: VaultOption[]
  multiWorkspaceEnabled?: boolean
  onSwitchVault: (path: string) => void
  onSetDefaultWorkspace?: (path: string) => void
  onOpenSettings?: () => void
  onOpenVaultSettings?: () => void
  onOpenLocalFolder?: () => void
  onCreateEmptyVault?: () => void
  onCloneVault?: () => void
  onCloneGettingStarted?: () => void
  onClickPending?: () => void
  onClickPulse?: () => void
  onCommitPush?: () => void
  commitActionPending?: boolean
  gitFeaturesEnabled?: boolean
  onInitializeGit?: () => void
  isOffline?: boolean
  isVaultReloading?: boolean
  isGitVault?: boolean
  syncStatus?: SyncStatus
  lastSyncTime?: number | null
  conflictCount?: number
  remoteStatus?: GitRemoteStatus | null
  repositories?: GitRepositoryOption[]
  selectedRepositoryPath?: string
  onRepositoryChange?: (path: string) => void
  onTriggerSync?: () => void
  onPullAndPush?: () => void
  onOpenConflictResolver?: () => void
  zoomLevel?: number
  themeMode?: ThemeMode
  onZoomReset?: () => void
  onToggleThemeMode?: () => void
  onOpenDocs?: () => void
  buildNumber?: string
  onCheckForUpdates?: () => void
  onRemoveVault?: (path: string) => void
  onReorderVaults?: (orderedPaths: string[]) => void
  onUpdateWorkspaceIdentity?: (path: string, patch: Partial<VaultOption>) => void
  aiFeaturesEnabled?: boolean
  mcpStatus?: McpStatus
  onInstallMcp?: () => void
  miniAppsProps?: {
    vaultPath: string | null
    activeNote?: { path?: string | null; title?: string | null } | null
    onToast?: (message: string) => void
  }
  sidebarVisible?: boolean
  onToggleSidebar?: () => void
  locale?: AppLocale
}

interface StatusBarFooterProps extends StatusBarProps {
  compact: boolean
  stacked: boolean
}

function StatusBarPrimaryFromFooter(options: StatusBarFooterProps) {
  return (
    <StatusBarPrimarySection
      {...options}
      modifiedCount={options.modifiedCount ?? 0}
      syncStatus={options.syncStatus ?? 'idle'}
      lastSyncTime={options.lastSyncTime ?? null}
      conflictCount={options.conflictCount ?? 0}
      mcpStatus={options.aiFeaturesEnabled === false ? undefined : options.mcpStatus}
    />
  )
}

function StatusBarSecondaryFromFooter(options: StatusBarFooterProps) {
  const {
    noteCount,
    zoomLevel = 100,
    themeMode = 'light',
    onZoomReset,
    onToggleThemeMode,
    onOpenDocs,
    onOpenSettings,
    locale = 'en',
    miniAppsProps,
    compact,
    stacked,
  } = options
  return (
      <StatusBarSecondarySection
        noteCount={noteCount}
        zoomLevel={zoomLevel}
        themeMode={themeMode}
        onZoomReset={onZoomReset}
        onToggleThemeMode={onToggleThemeMode}
        onOpenDocs={onOpenDocs}
        onOpenSettings={onOpenSettings}
        locale={locale}
        stacked={stacked}
        compact={compact}
        miniAppsProps={miniAppsProps}
      />
  )
}

function StatusBarFooter(props: StatusBarFooterProps) {
  const { compact, stacked } = props

  return (
    <footer
      data-testid="status-bar"
      style={{
        minHeight: 30,
        height: stacked ? 'auto' : 30,
        flexShrink: 0,
        display: 'flex',
        flexWrap: stacked ? 'wrap' : 'nowrap',
        alignItems: stacked ? 'flex-start' : 'center',
        justifyContent: stacked ? 'flex-start' : 'space-between',
        rowGap: stacked ? 4 : 0,
        columnGap: compact ? 8 : 12,
        background: 'var(--sidebar)',
        borderTop: '1px solid var(--border)',
        padding: stacked ? '4px 8px' : '0 8px',
        fontSize: 12,
        color: 'var(--muted-foreground)',
        position: 'relative',
        zIndex: STATUS_BAR_STACKING_Z_INDEX,
      }}
    >
      <StatusBarPrimaryFromFooter {...props} />
      <StatusBarSecondaryFromFooter {...props} />
    </footer>
  )
}

export function StatusBar(props: StatusBarProps) {
  useStatusBarTicker()
  const { compact, stacked } = useStatusBarLayout()

  return (
    <TooltipProvider>
      <StatusBarFooter {...props} compact={compact} stacked={stacked} />
    </TooltipProvider>
  )
}
