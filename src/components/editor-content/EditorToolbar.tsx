import { ArrowLeft, ArrowRight, MagnifyingGlass, X } from '@phosphor-icons/react'
import { memo, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ActionTooltip, type ActionTooltipCopy } from '@/components/ui/action-tooltip'
import { cn } from '@/lib/utils'
import { translate, type AppLocale } from '../../lib/i18n'
import type { VaultEntry } from '../../types'

const HISTORY_BACK_SHORTCUT = { shortcut: 'H' }
const HISTORY_FORWARD_SHORTCUT = { shortcut: 'L' }

export interface EditorToolbarProps {
  entry: VaultEntry | null
  vaultPath?: string
  search?: string
  onSearchChange?: (value: string) => void
  canGoBack?: boolean
  canGoForward?: boolean
  onGoBack?: () => void
  onGoForward?: () => void
  locale?: AppLocale
  isVaultLoading?: boolean
}

function pathParts(entryPath: string): string[] {
  const cleaned = entryPath.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
  return cleaned.split('/').filter(Boolean)
}

function vaultNameFromPath(vaultPath: string): string {
  const cleaned = vaultPath.replace(/\\/g, '/').replace(/\/+$/, '')
  const parts = cleaned.split('/')
  return parts[parts.length - 1] || 'Vault'
}

function BreadcrumbPath({ entryPath, vaultPath }: { entryPath: string; vaultPath?: string }) {
  const parts = pathParts(entryPath)
  const filenameStem = parts.length > 0 ? parts[parts.length - 1].replace(/\.md$/, '') : ''
  const folderParts = parts.length > 1 ? parts.slice(0, -1) : []
  const crumbs = [
    ...(vaultPath ? [vaultNameFromPath(vaultPath)] : []),
    ...folderParts,
    filenameStem,
  ]

  return (
    <div className="flex min-w-0 items-center gap-1.5 text-sm text-muted-foreground">
      {crumbs.map((crumb, index) => {
        const isLast = index === crumbs.length - 1
        return (
          <span key={index} className="flex items-center gap-1.5">
            {index > 0 && <span aria-hidden="true">›</span>}
            <span className={cn('truncate', isLast ? 'text-foreground font-medium' : 'text-muted-foreground')} title={crumb}>
              {crumb}
            </span>
          </span>
        )
      })}
    </div>
  )
}

export const EditorToolbar = memo(function EditorToolbar({
  entry,
  vaultPath,
  search,
  onSearchChange,
  canGoBack = false,
  canGoForward = false,
  onGoBack,
  onGoForward,
  locale = 'en',
  isVaultLoading = false,
}: EditorToolbarProps) {
  const searchInputRef = useRef<HTMLInputElement>(null)

  if (isVaultLoading) {
    return (
      <div className="breadcrumb-bar flex shrink-0 items-center border-b border-border bg-background" style={{ height: 52, padding: '6px 16px' }}>
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-sm text-muted-foreground">{translate(locale, 'status.vault.loading')}</span>
        </div>
      </div>
    )
  }

  if (!entry) {
    return null
  }

  const backCopy: ActionTooltipCopy = {
    label: translate(locale, 'command.navigation.goBack'),
    shortcut: HISTORY_BACK_SHORTCUT.shortcut,
  }
  const forwardCopy: ActionTooltipCopy = {
    label: translate(locale, 'command.navigation.goForward'),
    shortcut: HISTORY_FORWARD_SHORTCUT.shortcut,
  }
  const searchCopy: ActionTooltipCopy = { label: translate(locale, 'noteList.searchAction') }

  return (
    <div className="breadcrumb-bar flex shrink-0 items-center border-b border-border bg-background" style={{ height: 52, padding: '6px 16px', boxSizing: 'border-box' }}>
      <div className="flex items-center gap-1" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
        {onGoBack && (
          <ActionTooltip copy={backCopy} side="top">
            <Button
              type="button"
              variant="ghost"
              size="xs"
              className="h-6 w-6 shrink-0 !min-w-0 !rounded !p-0 text-muted-foreground"
              onClick={onGoBack}
              disabled={!canGoBack}
              aria-label={translate(locale, 'command.navigation.goBack')}
            >
              <ArrowLeft size={14} weight="regular" />
            </Button>
          </ActionTooltip>
        )}
        {onGoForward && (
          <ActionTooltip copy={forwardCopy} side="top">
            <Button
              type="button"
              variant="ghost"
              size="xs"
              className="h-6 w-6 shrink-0 !min-w-0 !rounded !p-0 text-muted-foreground"
              onClick={onGoForward}
              disabled={!canGoForward}
              aria-label={translate(locale, 'command.navigation.goForward')}
            >
              <ArrowRight size={14} weight="regular" />
            </Button>
          </ActionTooltip>
        )}
        {onSearchChange && (
          <ActionTooltip copy={searchCopy} side="top">
            <Button
              type="button"
              variant="ghost"
              size="xs"
              className="h-6 w-6 shrink-0 !min-w-0 !rounded !p-0 text-muted-foreground"
              onClick={() => searchInputRef.current?.focus()}
              aria-label={translate(locale, 'noteList.searchAction')}
            >
              <MagnifyingGlass size={14} weight="regular" />
            </Button>
          </ActionTooltip>
        )}
      </div>
      <div className="min-w-0 flex-1 px-3">
        <BreadcrumbPath entryPath={entry.path} vaultPath={vaultPath} />
      </div>
      {(search !== undefined || onSearchChange) && (
        <div className="flex shrink-0 items-center gap-1" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <div className="relative" data-testid="editor-toolbar-search">
            <Input
              ref={searchInputRef}
              value={search ?? ''}
              onChange={(event) => onSearchChange?.(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') onSearchChange?.('')
              }}
              placeholder={translate(locale, 'noteList.searchPlaceholder')}
              className="h-7 w-36 text-[12px]"
            />
            {search && (
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="absolute inset-y-1 right-0 !h-5 !w-5 !min-w-0 !rounded !p-0 !text-muted-foreground hover:!bg-accent hover:!text-foreground [&_svg]:!size-3"
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => onSearchChange?.('')}
                title={translate(locale, 'noteList.clearSearch')}
                aria-label={translate(locale, 'noteList.clearSearch')}
              >
                <X size={10} />
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  )
})
