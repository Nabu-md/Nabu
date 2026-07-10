import React, { forwardRef } from 'react'
import { FileTree, FileTreeHandle } from './FileTree'
import { TagsPanel } from './TagsPanel'
import { OutlinePanel } from './OutlinePanel'
import { FavoritesPanel } from './FavoritesPanel'
import { useAppContext } from '../App'

// ---------------------------------------------------------------------------
// Sidebar
//
// Wraps FileTree in the left-panel container and forwards the ref to
// FileTree's search input so the parent (App.tsx) can focus it on Cmd+Shift+F.
// ---------------------------------------------------------------------------

export interface SidebarHandle {
  focusSearch(): void
}

export const Sidebar = forwardRef<SidebarHandle>(function Sidebar(_props, ref) {
  const fileTreeRef = React.useRef<FileTreeHandle>(null)
  const { state } = useAppContext()

  // Forward focusSearch to FileTree
  React.useImperativeHandle(ref, () => ({
    focusSearch() {
      fileTreeRef.current?.focusSearch()
    }
  }))

  // Compute tag-filtered paths: union all sets for each selected tag, or null if none selected
  const tagFilteredPaths: Set<string> | null = React.useMemo(() => {
    if (state.selectedTags.size === 0) return null
    const union = new Set<string>()
    for (const tag of state.selectedTags) {
      const paths = state.tagIndex.get(tag)
      if (paths) {
        for (const p of paths) union.add(p)
      }
    }
    return union
  }, [state.selectedTags, state.tagIndex])

  return (
    <aside className="sidebar flex flex-col h-full overflow-hidden" aria-label="Sidebar">
      <FavoritesPanel />
      <FileTree ref={fileTreeRef} tagFilteredPaths={tagFilteredPaths} />
      <TagsPanel />
      <OutlinePanel />
    </aside>
  )
})
