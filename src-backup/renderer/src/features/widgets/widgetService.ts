/**
 * widgetService.ts
 *
 * Renderer-side widget state owner for the widget UI layer.
 *
 * This module is the single source of truth for widget-rendered state on the
 * renderer. It subscribes to the widget-specific IPC channels (activity log and
 * dictation) and exposes that state to the widget UI components through hooks.
 *
 * Ownership boundary (Phase 3.2 — Persistence Alignment & UI Cleanup):
 *
 *   Registry (WidgetManager, main process)
 *            │  IPC channels
 *            ▼
 *   Widget State (widgetService — this module)
 *            │  hooks
 *            ▼
 *   Renderer (ActivityTimeline / DictationWidget)
 *            │
 *            ▼
 *   UI
 *
 * The widget UI components depend ONLY on this service and its contracts. They
 * no longer reach into the global app context (`useAppContext`) or the raw
 * Electron preload bridge (`window.electron`). Rendering never becomes the
 * owner of widget state — it only reads from this service.
 *
 * Behavior is unchanged: the same IPC channels are consumed, the same
 * ActivityEntry shape is produced, and the same dictation actions are invoked.
 */

import { useEffect, useState } from 'react'
import { ipc } from '../../shared/ipc'
import type { ActivityEntry } from '@shared/models'

export type { ActivityEntry }

// ---------------------------------------------------------------------------
// Activity log state
// ---------------------------------------------------------------------------

const MAX_ACTIVITY_ENTRIES = 100

let activityEntries: ActivityEntry[] = []
const activityListeners = new Set<(entries: ActivityEntry[]) => void>()
let activitySubscribed = false

/**
 * Push a new activity entry and notify subscribers.
 * Mirrors the previous App reducer behavior (prepend, cap at 100).
 */
function pushActivity(entry: ActivityEntry): void {
  activityEntries = [entry, ...activityEntries].slice(0, MAX_ACTIVITY_ENTRIES)
  for (const listener of activityListeners) listener(activityEntries)
}

function ensureActivitySubscription(): void {
  if (activitySubscribed) return
  activitySubscribed = true

  ipc.on.activityLog((entry) => {
    // activity:log messages have { level, message, timestamp } shape.
    // Convert to ActivityEntry for the timeline display.
    const logEntry = entry as unknown as { level: string; message: string; timestamp: number }
    pushActivity({
      filePath: logEntry.message ?? '',
      timestamp: logEntry.timestamp ?? Date.now(),
      isExternal: false
    })
  })
}

/** Record an external activity entry (e.g. external note edit). */
export function recordExternalActivity(filePath: string): void {
  ensureActivitySubscription()
  pushActivity({ filePath, timestamp: Date.now(), isExternal: true })
}

/** Subscribe to activity-log updates. Returns an unsubscribe function. */
export function subscribeActivity(cb: (entries: ActivityEntry[]) => void): () => void {
  ensureActivitySubscription()
  activityListeners.add(cb)
  cb(activityEntries)
  return () => {
    activityListeners.delete(cb)
  }
}

// ---------------------------------------------------------------------------
// Dictation state
// ---------------------------------------------------------------------------

export type WidgetMode = 'clipboard' | 'dictation'

export interface DictationState {
  status: 'idle' | 'listening' | 'complete' | 'error'
  text: string
  error: string | null
  silent: boolean
}

const IDLE_DICTATION: DictationState = {
  status: 'idle',
  text: '',
  error: null,
  silent: false
}

let dictationState: DictationState = { ...IDLE_DICTATION }
const dictationListeners = new Set<(state: DictationState) => void>()
let dictationSubscribed = false

function setDictation(next: DictationState): void {
  dictationState = next
  for (const listener of dictationListeners) listener(dictationState)
}

function ensureDictationSubscription(): void {
  if (dictationSubscribed) return
  dictationSubscribed = true

  ipc.on.widgetModeChanged((data) => {
    if (data.mode === 'dictation') {
      setDictation({ ...IDLE_DICTATION, status: 'listening' })
    }
  })

  ipc.on.widgetDictationStarting(() => {
    setDictation({ ...IDLE_DICTATION, status: 'listening' })
  })

  ipc.on.widgetDictationComplete((data) => {
    setDictation({
      status: 'complete',
      text: data.text,
      error: null,
      silent: data.silent ?? false
    })
  })

  ipc.on.widgetDictationError((data) => {
    setDictation({
      status: 'error',
      text: '',
      error: data.error,
      silent: false
    })
  })
}

/** Subscribe to dictation-state updates. Returns an unsubscribe function. */
export function subscribeDictation(cb: (state: DictationState) => void): () => void {
  ensureDictationSubscription()
  dictationListeners.add(cb)
  cb(dictationState)
  return () => {
    dictationListeners.delete(cb)
  }
}

// ---------------------------------------------------------------------------
// Dictation actions (widget-specific contract only)
// ---------------------------------------------------------------------------

export const widgetDictationActions = {
  start: async (): Promise<void> => {
    await ipc.dictation.start()
  },
  stop: async (): Promise<void> => {
    await ipc.dictation.stop()
  }
}

// ---------------------------------------------------------------------------
// React hooks — the only surface the widget UI consumes
// ---------------------------------------------------------------------------

/** Widget UI hook for the activity timeline. */
export function useWidgetActivity(): ActivityEntry[] {
  const [entries, setEntries] = useState<ActivityEntry[]>(activityEntries)
  useEffect(() => subscribeActivity(setEntries), [])
  return entries
}

/** Widget UI hook for the dictation widget. */
export function useWidgetDictation(): {
  state: DictationState
  start: () => Promise<void>
  stop: () => Promise<void>
} {
  const [state, setState] = useState<DictationState>(dictationState)
  useEffect(() => subscribeDictation(setState), [])
  return {
    state,
    start: widgetDictationActions.start,
    stop: widgetDictationActions.stop
  }
}
