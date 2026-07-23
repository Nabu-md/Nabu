/**
 * events.ts
 *
 * Canonical typed event registry for the internal event bus (Phase 1.5).
 *
 * Each entry declares:
 *  - the event name (a string-literal key)
 *  - the payload type
 *  - the publisher ownership (which layer/service may emit it)
 *  - the subscriber ownership (which layer/service may listen)
 *
 * These events describe *background, asynchronous internal workflows* only.
 * Synchronous renderer ↔ main communication continues to flow through the
 * typed IPC layer (`src/shared/ipc`); it is NOT routed through this bus.
 *
 * Ownership rules (Phase 1.1 / NRP §8 Layer Dependency Flow):
 *   Main ↓ Services ↓ Shared   (and Renderer ↓ Shared, IPC ↓ Services)
 *   Shared must not depend upward on Main or Renderer.
 * Therefore every event defined here lives in `shared` and is emitted by
 * main-process services; the renderer never imports or subscribes to this bus.
 */

import type { FilePath, VaultId, IndexBuildPayload, WhisperResult } from '../models'

/**
 * Ownership descriptor for an event.
 * - `publisher` — the layer/service allowed to emit the event.
 * - `subscribers` — the layer(s)/service(s) allowed to listen.
 */
export interface EventOwnership {
  publisher: string
  subscribers: string
}

/**
 * The canonical event map: event name → payload type.
 *
 * Declared as a `type` (not `interface`) so it satisfies the `EventMap`
 * constraint in `bus.ts` without requiring an explicit index signature.
 *
 * Payloads reuse shared domain models where possible to avoid duplication.
 */
export type AppEvents = {
  /** A vault was successfully opened and its session registered. */
  VaultOpened: {
    vaultId: VaultId
    path: FilePath
    fileCount: number
  }

  /** A vault session was closed (watcher stopped, state released). */
  VaultClosed: {
    vaultId: VaultId
    path: FilePath
  }

  /** A full-text / tag / extended index build completed for a vault. */
  IndexUpdated: {
    vaultId: VaultId
    path: FilePath
    payload: IndexBuildPayload
  }

  /** A (semantic / vector) search completed. */
  SearchCompleted: {
    vaultId: VaultId
    query: string
    resultCount: number
  }

  /** A widget window was registered/created and is ready. */
  WidgetRegistered: {
    widgetId: string
    kind: 'clipboard' | 'dictation' | string
  }

  /** A dictation capture finished and a transcript is available. */
  DictationFinished: {
    widgetId: string
    result: WhisperResult
  }

  /** A note file was saved to disk by the application. */
  NoteSaved: {
    vaultId: VaultId
    path: FilePath
  }

  /** A note file was deleted from the vault. */
  NoteDeleted: {
    vaultId: VaultId
    path: FilePath
  }
}

/**
 * Human-readable ownership metadata for every canonical event.
 * Used by documentation and (optionally) runtime assertions in dev builds.
 */
export const EVENT_OWNERSHIP: { [K in keyof AppEvents]: EventOwnership } = {
  VaultOpened: {
    publisher: 'Services (VaultService)',
    subscribers: 'Services (VectorManager, VaultWatcher)'
  },
  VaultClosed: {
    publisher: 'Services (VaultService)',
    subscribers: 'Services (VectorManager, VaultWatcher)'
  },
  IndexUpdated: {
    publisher: 'Services (StateManager / VaultWatcher)',
    subscribers: 'Services (VectorManager, SearchService)'
  },
  SearchCompleted: {
    publisher: 'Services (VectorManager / SearchService)',
    subscribers: 'Services (internal logging / activity)'
  },
  WidgetRegistered: {
    publisher: 'Services (WidgetManager)',
    subscribers: 'Services (DictationService)'
  },
  DictationFinished: {
    publisher: 'Services (DictationService / Whisper)',
    subscribers: 'Services (WidgetManager)'
  },
  NoteSaved: {
    publisher: 'Services (StateManager / IPC handlers)',
    subscribers: 'Services (VaultWatcher, VectorManager)'
  },
  NoteDeleted: {
    publisher: 'Services (StateManager / IPC handlers)',
    subscribers: 'Services (VaultWatcher, VectorManager)'
  }
}

/** The set of all known event names. */
export type AppEventName = keyof AppEvents
