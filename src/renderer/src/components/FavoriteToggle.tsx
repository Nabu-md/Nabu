/**
 * FavoriteToggle.tsx
 *
 * A star button that toggles the current note's favorite state.
 * Used in NoteView toolbar and FileTree context menu.
 *
 * Requirements: 18.2, 18.6
 */

import React, { useEffect, useState } from 'react'
import { useAppContext } from '../App'

interface FavoriteToggleProps {
  filePath: string
  size?: 'sm' | 'md'
}

export function FavoriteToggle({ filePath, size = 'sm' }: FavoriteToggleProps): React.JSX.Element {
  const { state } = useAppContext()
  const [isFavorite, setIsFavorite] = useState(false)

  // Check if current file is favorited
  useEffect(() => {
    const check = async (): Promise<void> => {
      if (!state.vault) return
      try {
        const result = await window.electron.favorites.get(state.vault.path)
        const favs = (result as { favorites: string[] }).favorites ?? []
        setIsFavorite(favs.includes(filePath))
      } catch {
        setIsFavorite(false)
      }
    }
    check()
  }, [state.vault, filePath])

  const handleToggle = async (e: React.MouseEvent): Promise<void> => {
    e.stopPropagation()
    if (!state.vault) return
    try {
      const result = await window.electron.favorites.toggle(state.vault.path, filePath)
      const favs = (result as { favorites: string[] }).favorites ?? []
      setIsFavorite(favs.includes(filePath))
    } catch (err) {
      console.error('[FavoriteToggle] Failed to toggle:', err)
    }
  }

  const sizeClass = size === 'md' ? 'text-base' : 'text-xs'

  return (
    <button
      type="button"
      aria-label={isFavorite ? 'Remove from favorites' : 'Add to favorites'}
      onClick={handleToggle}
      className={`${sizeClass} transition-colors ${
        isFavorite
          ? 'text-yellow-500 hover:text-yellow-400'
          : 'text-nabu-text-muted hover:text-yellow-500'
      }`}
    >
      {isFavorite ? '★' : '☆'}
    </button>
  )
}
