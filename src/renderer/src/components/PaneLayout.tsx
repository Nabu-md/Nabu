/**
 * PaneLayout.tsx
 *
 * Multi-pane layout component for split views (Req 24.2, 24.4, 24.5).
 * Supports single, horizontal split, vertical split, and grid layouts.
 * Each pane is bound to an open tab with independent scroll/mode/cursor.
 */

import React, { useCallback, useMemo } from 'react'
import { useAppContext } from '../App'
import { NoteView } from './NoteView'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type PaneLayout = 'single' | 'split-horizontal' | 'split-vertical' | 'grid'

export interface Pane {
  id: string
  tabId: string | null
}

// ---------------------------------------------------------------------------
// Helper: get pane tabs based on active tab
// ---------------------------------------------------------------------------

function usePaneTabs(activeTabId: string | null, openTabs: { id: string }[]): { tabId: string | null }[] {
  return useMemo(() => {
    // For now, return single pane with active tab
    // TODO: Implement grid layout logic for multiple panes
    return [{ tabId: activeTabId }]
  }, [activeTabId])
}

// ---------------------------------------------------------------------------
// PaneLayout component
// ---------------------------------------------------------------------------

export function PaneLayout(): React.JSX.Element {
  const { state } = useAppContext()
  const { activeTabId, openTabs, paneLayout = 'single' } = state as any // TODO: Add paneLayout to AppState

  // Get panes for current layout
  const panes = usePaneTabs(activeTabId, openTabs)

  // For now, render single pane (Task 72)
  // Future: implement split view rendering
  const activeTab = openTabs.find(t => t.id === activeTabId)

  return (
    <div className="pane-layout h-full flex">
      {/* Single pane mode - render NoteView directly */}
      <div className="flex-1 overflow-hidden">
        <NoteView />
      </div>
    </div>
  )
}