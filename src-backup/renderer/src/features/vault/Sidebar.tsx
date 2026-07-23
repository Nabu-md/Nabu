/**
 * Sidebar.tsx
 *
 * Obsidian-style icon ribbon sidebar with expandable panels.
 * Left column: 46px SVG icon ribbon (always visible).
 * Right column: Active panel content (file tree, tags, outline, favorites)
 * that slides in when an icon is clicked.
 *
 * Requirements: 18.1, 24.1, 38.1
 */

import React, { forwardRef, useState, useCallback } from 'react'
import { FileTree, FileTreeHandle } from './FileTree'
import { TagsPanel } from './TagsPanel'
import { OutlinePanel } from '../../shared/components/OutlinePanel'
import { FavoritesPanel } from './FavoritesPanel'
import { useAppContext } from '../../shared/store'
import {
  FilesIcon,
  SearchIcon,
  GraphIcon,
  StarIcon,
  TagIcon,
  OutlineIcon,
  SettingsIcon,
  ChevronLeftIcon
} from '../../shared/components/icons'

// ---------------------------------------------------------------------------
// Sidebar section configuration
// ---------------------------------------------------------------------------

type SectionId = 'files' | 'search' | 'graph' | 'favorites' | 'tags' | 'outline' | 'settings'

interface Section {
  id: SectionId
  icon: React.FC<{ className?: string; size?: number }>
  label: string
  action: 'panel' | 'toggle' | 'dispatch'
}

const SECTIONS: Section[] = [
  { id: 'files', icon: FilesIcon, label: 'Files', action: 'panel' },
  { id: 'search', icon: SearchIcon, label: 'Search', action: 'toggle' },
  { id: 'graph', icon: GraphIcon, label: 'Graph', action: 'toggle' },
  { id: 'favorites', icon: StarIcon, label: 'Favorites', action: 'panel' },
  { id: 'tags', icon: TagIcon, label: 'Tags', action: 'panel' },
  { id: 'outline', icon: OutlineIcon, label: 'Outline', action: 'panel' },
  { id: 'settings', icon: SettingsIcon, label: 'Settings', action: 'toggle' },
]

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

export interface SidebarHandle {
  focusSearch(): void
}

export const Sidebar = forwardRef<SidebarHandle>(function Sidebar(_props, ref) {
  const { state, dispatch } = useAppContext()
  const [activeSection, setActiveSection] = useState<SectionId | null>('files')
  const fileTreeRef = React.useRef<FileTreeHandle>(null)

  // Forward focusSearch to FileTree
  React.useImperativeHandle(ref, () => ({
    focusSearch() {
      setActiveSection('files')
      fileTreeRef.current?.focusSearch()
    }
  }))

  // Compute tag-filtered paths
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

  const handleSectionClick = useCallback(
    (section: Section) => {
      switch (section.action) {
        case 'panel':
          // Toggle the panel on/off
          setActiveSection((prev) => (prev === section.id ? null : section.id))
          break
        case 'toggle':
          // Dispatch the appropriate action
          if (section.id === 'graph') {
            dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
          } else if (section.id === 'search') {
            dispatch({ type: 'SEARCH_PANEL_TOGGLE' })
          } else if (section.id === 'settings') {
            dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
          }
          break
      }
    },
    [dispatch]
  )

  return (
    <aside className="sidebar" aria-label="Sidebar">
      {/* Icon ribbon */}
      <div className="sidebar-ribbon">
        {SECTIONS.map((section) => {
          const Icon = section.icon
          const isActive = activeSection === section.id
          return (
            <button
              key={section.id}
              className={`sidebar-ribbon__btn ${isActive ? 'sidebar-ribbon__btn--active' : ''}`}
              onClick={() => handleSectionClick(section)}
              title={section.label}
              aria-label={section.label}
              type="button"
            >
              <Icon size={18} />
            </button>
          )
        })}

        {/* Spacer pushes collapse to bottom */}
        <div className="flex-1 min-h-0" />

        {/* Collapse panel button */}
        {activeSection && (
          <button
            className="sidebar-ribbon__btn sidebar-ribbon__btn--collapse"
            onClick={() => setActiveSection(null)}
            title="Collapse sidebar"
            aria-label="Collapse sidebar"
            type="button"
          >
            <ChevronLeftIcon size={16} />
          </button>
        )}
      </div>

      {/* Active panel content */}
      {activeSection && (
        <div className="sidebar-panel">
          {/* Panel header */}
          <div className="sidebar-panel__header">
            <span className="sidebar-panel__title">
              {SECTIONS.find((s) => s.id === activeSection)?.label ?? ''}
            </span>
          </div>

          {/* Panel body */}
          <div className="sidebar-panel__body">
            {activeSection === 'files' && (
              <FileTree ref={fileTreeRef} tagFilteredPaths={tagFilteredPaths} />
            )}
            {activeSection === 'favorites' && <FavoritesPanel />}
            {activeSection === 'tags' && <TagsPanel />}
            {activeSection === 'outline' && <OutlinePanel />}
          </div>
        </div>
      )}
    </aside>
  )
})
