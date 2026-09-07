import { memo, useCallback, useMemo, useState } from 'react'
import {
  CaretDown,
  CaretRight,
  File as FileIcon,
  FilePlus,
  FileMinus,
  MagnifyingGlass,
  Graph,
} from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { translate, type AppLocale } from '../lib/i18n'
import type { ModifiedFile } from '../types'
import { DiffView } from './DiffView'

export interface GitDiffSidebarProps {
  vaultPath: string
  changedFiles: ModifiedFile[]
  selectedFile: string | null
  diff: string | null
  diffLoading?: boolean
  onFileSelect: (path: string) => void | Promise<void>
  onCommit: (message: string, push: boolean) => void | Promise<void>
  remoteConfigured?: boolean
  locale?: AppLocale
  onOpenGraphView?: () => void
  graphViewOpen?: boolean
}

interface ChangedFileRow {
  relativePath: string
  status: ModifiedFile['status']
}

interface ChangedFolderGroup {
  folder: string
  files: ChangedFileRow[]
}

const STATUS_COLOR: Record<ModifiedFile['status'], string> = {
  modified: 'text-[var(--accent-orange)]',
  added: 'text-[var(--accent-green)]',
  untracked: 'text-[var(--accent-green)]',
  deleted: 'text-[var(--destructive)]',
  renamed: 'text-[var(--accent-blue)]',
}

function statusLetter(status: ModifiedFile['status']): string {
  if (status === 'added' || status === 'untracked') return 'A'
  if (status === 'deleted') return 'D'
  if (status === 'renamed') return 'R'
  return 'M'
}

function StatusIcon({ status }: { status: ModifiedFile['status'] }) {
  if (status === 'deleted') return <FileMinus size={13} className={STATUS_COLOR[status]} aria-hidden />
  if (status === 'added' || status === 'untracked') {
    return <FilePlus size={13} className={STATUS_COLOR[status]} aria-hidden />
  }
  return <FileIcon size={13} className={STATUS_COLOR[status]} aria-hidden />
}

function fileName(relativePath: string): string {
  return relativePath.split('/').pop() ?? relativePath
}

function folderOf(relativePath: string): string {
  const parts = relativePath.split('/')
  return parts.length <= 1 ? '' : parts.slice(0, -1).join('/')
}

function groupChangedFilesByFolder(changedFiles: ModifiedFile[]): ChangedFolderGroup[] {
  const byFolder = new Map<string, ChangedFileRow[]>()
  for (const file of changedFiles) {
    const folder = folderOf(file.relativePath)
    const rows = byFolder.get(folder) ?? []
    rows.push({ relativePath: file.relativePath, status: file.status })
    byFolder.set(folder, rows)
  }
  return [...byFolder.entries()]
    .map(([folder, files]) => ({ folder, files }))
    .sort((left, right) => left.folder.localeCompare(right.folder))
}

function matchesSearch(relativePath: string, query: string): boolean {
  if (!query) return true
  return relativePath.toLowerCase().includes(query.toLowerCase())
}

function ChangedFileButton({
  file,
  selected,
  depth,
  onSelect,
}: {
  file: ChangedFileRow
  selected: boolean
  depth: number
  onSelect: (relativePath: string) => void
}) {
  return (
    <button
      type="button"
      className={cn(
        'flex w-full items-center gap-2 rounded-md py-1 pr-2 text-left text-[13px] transition-colors hover:bg-[var(--hover)]',
        selected ? 'bg-[var(--hover)] font-medium text-foreground' : 'text-foreground/90',
      )}
      style={{ paddingLeft: 8 + depth * 14 }}
      onClick={() => onSelect(file.relativePath)}
      data-testid={`git-diff-file-${file.relativePath}`}
    >
      <StatusIcon status={file.status} />
      <span className="min-w-0 flex-1 truncate">{fileName(file.relativePath)}</span>
      <span className={cn('shrink-0 text-[10px] font-semibold', STATUS_COLOR[file.status])}>
        {statusLetter(file.status)}
      </span>
    </button>
  )
}

function ChangedFolderSection({
  group,
  expanded,
  onToggle,
  selectedFile,
  onSelect,
  locale,
}: {
  group: ChangedFolderGroup
  expanded: boolean
  onToggle: (folder: string) => void
  selectedFile: string | null
  onSelect: (relativePath: string) => void
  locale: AppLocale
}) {
  const depth = group.folder ? group.folder.split('/').length : 0
  const label = group.folder || translate(locale, 'gitDiff.sidebar.vaultRoot')
  return (
    <div>
      <button
        type="button"
        className="flex w-full items-center gap-1.5 rounded-md py-1 pr-2 text-left text-[13px] font-medium text-foreground transition-colors hover:bg-[var(--hover)]"
        style={{ paddingLeft: 8 }}
        onClick={() => onToggle(group.folder)}
        aria-expanded={expanded}
        data-testid={`git-diff-folder-${group.folder || 'root'}`}
      >
        {expanded ? (
          <CaretDown size={12} className="shrink-0 text-muted-foreground" aria-hidden />
        ) : (
          <CaretRight size={12} className="shrink-0 text-muted-foreground" aria-hidden />
        )}
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <span className="shrink-0 rounded-full bg-[var(--accent-orange)]/15 px-1.5 text-[10px] font-semibold text-[var(--accent-orange)]">
          {group.files.length}
        </span>
      </button>
      {expanded && (
        <div className="flex flex-col gap-0.5 pb-1">
          {group.files.map((file) => (
            <ChangedFileButton
              key={file.relativePath}
              file={file}
              depth={depth + 1}
              selected={file.relativePath === selectedFile}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  )
}

/**
 * Phase 4 sidebar: a pure folder tree of git-changed files with an inline diff
 * viewer and a commit message box. Replaces the full navigation sidebar; the
 * bottom-ribbon Changes button opens this panel.
 */
export const GitDiffSidebar = memo(function GitDiffSidebar(options: GitDiffSidebarProps) {
  const {
    vaultPath,
    changedFiles,
    selectedFile,
    diff,
    diffLoading = false,
    onFileSelect,
    onCommit,
    remoteConfigured = false,
    locale = 'en',
    onOpenGraphView,
    graphViewOpen = false,
  } = options
  const [search, setSearch] = useState('')
  const [message, setMessage] = useState('')
  const [internalSelectedFile, setInternalSelectedFile] = useState<string | null>(null)
  const [expandedFolders, setExpandedFolders] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(groupChangedFilesByFolder(changedFiles).map((group) => [group.folder, true])),
  )
  const [committing, setCommitting] = useState(false)
  const activeSelectedFile = selectedFile ?? internalSelectedFile

  const filteredFiles = useMemo(
    () => changedFiles.filter((file) => matchesSearch(file.relativePath, search.trim())),
    [changedFiles, search],
  )
  const groups = useMemo(() => groupChangedFilesByFolder(filteredFiles), [filteredFiles])

  const toggleFolder = useCallback((folder: string) => {
    setExpandedFolders((current) => ({ ...current, [folder]: !current[folder] }))
  }, [])

  const handleSelectFile = useCallback(
    (relativePath: string) => {
      const absolute = vaultPath ? `${vaultPath.replace(/\/+$/, '')}/${relativePath}` : relativePath
      setInternalSelectedFile(absolute)
      void onFileSelect(absolute)
    },
    [onFileSelect, vaultPath],
  )

  const runCommit = useCallback(
    (push: boolean) => {
      const trimmed = message.trim()
      if (!trimmed || committing) return
      setCommitting(true)
      void Promise.resolve(onCommit(trimmed, push)).finally(() => {
        setCommitting(false)
        setMessage('')
      })
    },
    [committing, message, onCommit],
  )

  const commitDisabled = message.trim().length === 0 || changedFiles.length === 0 || committing

  return (
    <aside
      className="flex h-full flex-col overflow-hidden border-r border-[var(--sidebar-border)] bg-sidebar text-sidebar-foreground git-diff-sidebar"
      data-testid="git-diff-sidebar"
    >
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-2">
        <div className="relative flex-1">
          <MagnifyingGlass
            size={13}
            className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder={translate(locale, 'gitDiff.sidebar.searchPlaceholder')}
            className="h-7 pl-7 text-[12px]"
            aria-label={translate(locale, 'gitDiff.sidebar.searchPlaceholder')}
            data-testid="git-diff-search"
          />
        </div>
        {onOpenGraphView && (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            className={cn('h-7 w-7 shrink-0', graphViewOpen && 'bg-[var(--hover)] text-foreground')}
            onClick={onOpenGraphView}
            aria-label={translate(locale, 'gitDiff.sidebar.openGraph')}
            title={translate(locale, 'gitDiff.sidebar.openGraph')}
            data-testid="graph-view-toggle"
          >
            <Graph size={15} aria-hidden />
          </Button>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 py-1">
        {groups.length === 0 ? (
          <p className="px-2 py-6 text-center text-[12px] text-muted-foreground">
            {translate(locale, 'gitDiff.sidebar.noChanges')}
          </p>
        ) : (
          groups.map((group) => (
            <ChangedFolderSection
              key={group.folder}
              group={group}
              expanded={expandedFolders[group.folder] ?? true}
              onToggle={toggleFolder}
              selectedFile={selectedFile ? selectedFile.split('/').slice(-1)[0] && selectedFile : null}
              onSelect={handleSelectFile}
              locale={locale}
            />
          ))
        )}
        {activeSelectedFile && (
          <div className="mt-2 border-t border-border pt-2">
            {diffLoading ? (
              <p className="px-2 py-4 text-center text-[12px] text-muted-foreground">
                {translate(locale, 'gitDiff.sidebar.loadingDiff')}
              </p>
            ) : (
              <div
                className="overflow-x-auto rounded-lg border border-border bg-background/60"
                data-testid="git-diff-view"
              >
                <DiffView diff={diff ?? ''} />
              </div>
            )}
          </div>
        )}
      </div>

      <div className="shrink-0 border-t border-border px-3 py-2.5">
        <div className="mb-1.5 flex items-center justify-between">
          <span className="text-[11px] font-medium text-muted-foreground" data-testid="git-diff-file-count">
            {changedFiles.length}
          </span>
          <span className="text-[11px] text-muted-foreground">
            {translate(locale, 'gitDiff.sidebar.filesStaged', { count: changedFiles.length })}
          </span>
        </div>
        <Input
          value={message}
          onChange={(event) => setMessage(event.target.value)}
          placeholder={translate(locale, 'gitDiff.sidebar.commitPlaceholder')}
          className="mb-1.5 h-8 text-[12px]"
          aria-label={translate(locale, 'gitDiff.sidebar.commitPlaceholder')}
          data-testid="git-diff-commit-message"
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.nativeEvent.isComposing) runCommit(Boolean(remoteConfigured))
          }}
        />
        <div className="flex items-center gap-1.5">
          <Button
            type="button"
            variant="default"
            size="sm"
            className="h-7 flex-1 text-[12px]"
            disabled={commitDisabled}
            onClick={() => runCommit(false)}
            data-testid="git-diff-commit-btn"
          >
            {translate(locale, 'gitDiff.sidebar.commit')}
          </Button>
          {remoteConfigured && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 flex-1 text-[12px]"
              disabled={commitDisabled}
              onClick={() => runCommit(true)}
              data-testid="git-diff-commit-push-btn"
            >
              {translate(locale, 'gitDiff.sidebar.commitPush')}
            </Button>
          )}
        </div>
      </div>
    </aside>
  )
})
