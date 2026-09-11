import { useMemo } from 'react'
import { Image as ImageIcon, X } from '@phosphor-icons/react'
import { translate, type AppLocale } from '../lib/i18n'
import { coverImageSource, COVER_IMAGE_PROPERTY_KEY } from '../utils/coverImage'
import type { VaultEntry } from '../types'
import { ActionTooltip } from './ui/action-tooltip'
import { Button } from './ui/button'

interface NoteCoverProps {
  entry: VaultEntry
  locale?: AppLocale
  vaultPath?: string
  onPickCover?: () => void
  onRemoveCover?: () => void
}

/**
 * Notion-style cover image banner rendered at the top of the rich editor.
 * The source comes from the `cover_image` frontmatter property; controls are
 * rendered only when the matching callbacks are provided.
 */
export function NoteCover({ entry, locale = 'en', vaultPath, onPickCover, onRemoveCover }: NoteCoverProps) {
  const source = useMemo(
    () => coverImageSource({ value: entry.properties?.[COVER_IMAGE_PROPERTY_KEY], vaultPath }),
    [entry.properties, vaultPath],
  )

  if (source) {
    return (
      <div className="note-cover" data-testid="note-cover">
        <img className="note-cover__image" src={source} alt="" data-testid="note-cover-image" />
        {onRemoveCover && (
          <div className="note-cover__actions">
            {onPickCover && (
              <ActionTooltip copy={{ label: translate(locale, 'editor.cover.change') }} side="left">
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={onPickCover}
                  data-testid="note-cover-change"
                >
                  <ImageIcon size={14} aria-hidden />
                  {translate(locale, 'editor.cover.change')}
                </Button>
              </ActionTooltip>
            )}
            <ActionTooltip copy={{ label: translate(locale, 'editor.cover.remove') }} side="left">
              <Button
                type="button"
                variant="secondary"
                size="icon-sm"
                aria-label={translate(locale, 'editor.cover.remove')}
                title={translate(locale, 'editor.cover.remove')}
                onClick={onRemoveCover}
                data-testid="note-cover-remove"
              >
                <X size={14} aria-hidden />
              </Button>
            </ActionTooltip>
          </div>
        )}
      </div>
    )
  }

  if (!onPickCover) return null

  return (
    <div className="note-cover note-cover--empty" data-testid="note-cover-empty">
      <ActionTooltip copy={{ label: translate(locale, 'editor.cover.add') }} side="right">
        <Button type="button" variant="ghost" size="sm" onClick={onPickCover} data-testid="note-cover-add">
          <ImageIcon size={14} aria-hidden />
          {translate(locale, 'editor.cover.add')}
        </Button>
      </ActionTooltip>
    </div>
  )
}
