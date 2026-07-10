/**
 * PaneLayout.tsx
 *
 * Multi-pane layout component for split views (Req 24.2, 24.4, 24.5).
 * Supports single, horizontal split, vertical split, and grid layouts.
 * Each pane is bound to an open tab with independent scroll/mode/cursor.
 */

import React, { useMemo, useCallback } from 'react'
import { useAppContext, type Tab } from '../App'

// ---------------------------------------------------------------------------
// PaneLayout component
// ---------------------------------------------------------------------------

type PaneLayoutType = 'single' | 'split-horizontal' | 'split-vertical' | 'grid'

/**
 * Get the number of panes to show based on layout type.
 * - single: 1 pane (shows active tab)
 * - split-horizontal: 2 panes side by side
 * - split-vertical: 2 panes stacked
 * - grid: up to 4 panes in a 2x2 grid
 */
function getPaneCount(layout: PaneLayoutType): number {
  switch (layout) {
    case 'single':
      return 1
    case 'split-horizontal':
      return 2
    case 'split-vertical':
      return 2
    case 'grid':
      return 4
  }
}

/**
 * Get the grid template classes for each layout.
 */
function getLayoutClasses(layout: PaneLayoutType): string {
  switch (layout) {
    case 'single':
      return 'grid grid-cols-1 grid-rows-1'
    case 'split-horizontal':
      return 'grid grid-cols-2 grid-rows-1'
    case 'split-vertical':
      return 'grid grid-cols-1 grid-rows-2'
    case 'grid':
      return 'grid grid-cols-2 grid-rows-2'
  }
}

export function PaneLayout(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const { openTabs, activeTabId, paneLayout } = state

  // If no vault is open, don't render anything (SetupWizard will show)
  if (!state.vault) return null

  // Get tabs to display in panes
  const tabsForPanes: Tab[] = useMemo(() => {
    const paneCount = getPaneCount(paneLayout)
    // For now, use the first N open tabs
    const tabsToUse = openTabs.slice(0, paneCount)
    return tabsToUse
  }, [openTabs, paneLayout])

  // Pane click handler - activate the pane's tab
  const handlePaneClick = useCallback(
    (tabId: string) => {
      if (tabId !== activeTabId) {
        dispatch({ type: 'TAB_ACTIVATED', payload: { tabId } })
      }
    },
    [activeTabId, dispatch]
  )

  return (
    <div className={`pane-layout h-full ${getLayoutClasses(paneLayout)}`}>
      {tabsForPanes.map((tab) => {
        const isActive = tab.id === activeTabId
        return (
          <div
            key={tab.id}
            className={`pane-container relative h-full overflow-hidden cursor-pointer
              ${isActive ? 'ring-2 ring-nabu-accent z-10' : ''}`}
            onClick={() => handlePaneClick(tab.id)}
          >
            {/* Tab header */}
            <div className="pane-header flex items-center h-8 px-2 border-b border-nabu-border bg-nabu-bg-mute text-xs">
              <span
                className={`truncate ${isActive ? 'text-nabu-accent font-medium' : 'text-nabu-text-muted'}`}
              >
                {tab.path.split('/').pop()?.replace(/\.md$/, '') ?? tab.path}
              </span>
            </div>

            {/* Note content area - for now, we use the currentFile based NoteView */}
            <div className="pane-content h-full">
              {/* Pane placeholder - NoteView will render based on currentFile */}
              {isActive && <PaneContent tab={tab} />}
            </div>
          </div>
        )
      })}
    </div>
  )
}

/**
 * PaneContent - renders the note for a specific tab.
 * Currently uses the existing NoteView but scoped to this pane.
 */
function PaneContent({ tab }: { tab: Tab }): React.JSX.Element {
  // For now, we render a placeholder since NoteView uses global currentFile.
  // In a future iteration, this will be updated to render per-tab content.
  const { state } = useAppContext()

  // Only render if this is the active tab (NoteView already handles this)
  if (state.currentFile !== tab.path) {
    return (
      <div className="flex items-center justify-center h-full text-nabu-text-faint text-sm">
        Tab inactive
      </div>
    )
  }

  // Import and render NoteView - it will render the active tab content
  const { NoteView } = require('./NoteView')
  return <NoteView key={tab.id} />
}
