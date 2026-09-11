import { Image as ImageIcon } from '@phosphor-icons/react'
import { translate, type AppLocale } from '../lib/i18n'
import { COVER_IMAGE_PROPERTY_KEY, coverImageSource } from '../utils/coverImage'
import type { VaultEntry } from '../types'
import { Button } from './ui/button'
import {
  PROPERTY_PANEL_LABEL_CLASS_NAME,
  PROPERTY_PANEL_ROW_STYLE,
} from './propertyPanelLayout'

interface CoverImagePropertySectionProps {
  entry: VaultEntry
  locale?: AppLocale
  vaultPath?: string
  onPickCover?: () => void
  onRemoveCover?: () => void
}

/**
 * Properties panel row for the note cover image. Shows a thumbnail with
 * change/remove controls when set, or an add control when unset.
 */
export function CoverImagePropertySection({
  entry,
  locale = 'en',
  vaultPath,
  onPickCover,
  onRemoveCover,
}: CoverImagePropertySectionProps) {
  const source = coverImageSource({ value: entry.properties?.[COVER_IMAGE_PROPERTY_KEY], vaultPath })

  return (
    <div
      className="group/prop grid min-h-7 min-w-0 grid-cols-2 items-center gap-2 rounded px-1.5"
      style={PROPERTY_PANEL_ROW_STYLE}
      data-testid="cover-property"
    >
      <span className={PROPERTY_PANEL_LABEL_CLASS_NAME}>
        <span className="min-w-0 flex-1 truncate">{translate(locale, 'editor.cover.propertyLabel')}</span>
      </span>
      <div className="flex min-w-0 items-center justify-end gap-1">
        {source ? (
          <>
            <img
              src={source}
              alt=""
              className="h-6 w-9 rounded object-cover"
              data-testid="cover-property-thumbnail"
            />
            {onPickCover && (
              <Button type="button" variant="ghost" size="sm" onClick={onPickCover} data-testid="cover-property-change">
                {translate(locale, 'editor.cover.change')}
              </Button>
            )}
            {onRemoveCover && (
              <Button type="button" variant="ghost" size="sm" onClick={onRemoveCover} data-testid="cover-property-remove">
                {translate(locale, 'editor.cover.remove')}
              </Button>
            )}
          </>
        ) : (
          onPickCover && (
            <Button type="button" variant="ghost" size="sm" onClick={onPickCover} data-testid="cover-property-add">
              <ImageIcon size={14} aria-hidden />
              {translate(locale, 'editor.cover.add')}
            </Button>
          )
        )}
      </div>
    </div>
  )
}
