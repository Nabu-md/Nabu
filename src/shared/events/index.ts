/**
 * index.ts
 *
 * Public entry point for the internal typed event bus (Phase 1.5).
 *
 * Re-exports the generic `EventBus` and the canonical `AppEvents` registry,
 * and provides a single shared `appEventBus` instance typed to `AppEvents`.
 *
 * Usage (main process services only):
 *   import { appEventBus } from '@shared/events'
 *   appEventBus.publish('VaultOpened', { vaultId, path, fileCount })
 *   const off = appEventBus.subscribe('VaultOpened', (p) => { ... })
 *
 * The renderer process must NOT import this module. Renderer ↔ main
 * communication remains the responsibility of the typed IPC layer.
 */

export { EventBus } from './bus'
export type { EventMap, EventListener } from './bus'
export { EVENT_OWNERSHIP } from './events'
export type { AppEvents, AppEventName, EventOwnership } from './events'

import { EventBus } from './bus'
import type { AppEvents } from './events'

/**
 * The application-wide internal event bus.
 *
 * A single instance is shared across main-process services. It is created
 * here (in `shared`) so it carries no Electron or React dependency and can be
 * imported by any service without violating layer ownership rules.
 */
export const appEventBus = new EventBus<AppEvents>()
