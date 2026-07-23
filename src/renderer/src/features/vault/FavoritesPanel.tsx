/**
 * FavoritesPanel.tsx
 *
 * Sidebar panel showing favorited notes. Favorites are persisted per-vault
 * in `.nabu/favorites.json` and loaded on mount.
 *
 * Requirements: 18.1, 18.2, 18.3, 18.5, 18.6
 */

import React, { useEffect, useState, useCallback } from 'react'
import { useAppContext } from '../../shared/store'

export function FavoritesPanel(): React.JSX.Element {
  const { state, dispatch } = useAppContext()
  const [favorites, setFavorites] = useState<string[]>([])
  const [loading, setLoading] = useState(true)

  // Load favorites when vault changes
  useEffect(() => {
    const loadFavorites = async (): Promise<void> => {
      if (!state.vault) {
        setFavorites([])
        setLoading(false)
        return
      }
      try {
        const result = await window.ipc.favorites.get(state.vault.path)
        setFavorites((result as { favorites: string[] }).favorites ?? [])
      } catch (err) {
        console.error('[FavoritesPanel] Failed to load favorites:', err)
        setFavorites([])
      } finally {
        setLoading(false)
      }
    }
    loadFavorites()
  }, [state.vault])

  const handleClick = useCallback(
    async (filePath: string) => {
      try {
        const result = await window.ipc.file.get(filePath)
        const { path, ast } = result as { path: string; ast: import('mdast').Root }
        dispatch({ type: 'FILE_LOADED', payload: { path, ast } })
      } catch (err) {
        console.error('[FavoritesPanel] Failed to open note:', err)
      }
    },
    [dispatch]
  )

  if (loading) return <div className="favorites-panel px-3 py-2" />

  return (
    <div className="favorites-panel">
      <div className="px-3 py-2 text-xs font-medium uppercase tracking-wider text-nabu-text-muted">
        Favorites
      </div>
      {favorites.length === 0 ? (
        <div className="px-3 py-2 text-xs text-nabu-text-muted italic">No favorites yet</div>
      ) : (
        <ul className="flex flex-col gap-0.5 px-2 pb-2">
          {favorites.map((filePath) => {
            const name = filePath.split('/').pop()?.replace(/\.md$/, '') ?? filePath
            return (
              <li key={filePath}>
                <button
                  onClick={() => handleClick(filePath)}
                  className="w-full px-2 py-1 text-xs text-left rounded
                             text-nabu-text hover:bg-nabu-bg-mute
                             transition-colors truncate"
                  title={filePath}
                >
                  <span className="mr-1 text-yellow-500">★</span>
                  {name}
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
