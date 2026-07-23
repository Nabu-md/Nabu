/**
 * index.ts — IPC bootstrap entry point.
 *
 * This is the single bootstrap entry point for all IPC handler registration.
 * Its responsibility is limited to:
 *   - importing the feature registration functions
 *   - constructing the shared IPCContext (managers + feature services)
 *   - invoking each register*IPC() function
 *   - coordinating startup
 *
 * It must not contain handler implementations.
 *
 * This replaces the previous monolithic `src/main/ipc.ts` `registerIPCHandlers()`
 * entry point. The handler implementations now live in the per-feature modules
 * (vault.ts, notes.ts, search.ts, pdf.ts, settings.ts, widgets.ts, dictation.ts).
 */

import { VaultService } from '../services/vault-service'
import { SearchService } from '../services/search-service'
import { PdfService } from '../services/pdf-service'
import { DictationService } from '../services/dictation-service'

import type { StateManager } from '../services/state'
import type { VectorManager } from '../services/vector'
import type { VaultWatcher } from '../services/watcher'

import type { IPCContext } from './context'
import { registerVaultIPC } from './vault'
import { registerNotesIPC } from './notes'
import { registerSearchIPC } from './search'
import { registerPdfIPC } from './pdf'
import { registerSettingsIPC } from './settings'
import { registerWidgetsIPC } from './widgets'
import { registerDictationIPC } from './dictation'

// Re-export the shared helpers/lifecycle hooks so existing callers
// (src/main/index.ts) do not need to change their import paths.
export {
  setLegacyManagers,
  onWidgetToggle,
  sendToRenderer,
  buildWatcherConfig,
  emitActivityLog
} from './shared'

/**
 * Build the shared IPC context from the core managers.
 *
 * Mirrors the previous `registerIPCHandlers()` body which instantiated the
 * feature services once and shared them across all handlers.
 */
export function createIPCContext(
  stateManager: StateManager,
  vectorManager: VectorManager,
  watcher: VaultWatcher
): IPCContext {
  return {
    stateManager,
    vectorManager,
    watcher,
    vaultService: new VaultService(stateManager, vectorManager, watcher),
    searchService: new SearchService(stateManager),
    pdfService: new PdfService(),
    dictationService: new DictationService()
  }
}

/**
 * Register every feature IPC module.
 *
 * This is the single bootstrap function invoked from `src/main/index.ts`.
 * Each feature module owns exactly its own channels; there is no overlap.
 */
export function registerAllIPC(ctx: IPCContext): void {
  registerVaultIPC(ctx)
  registerNotesIPC(ctx)
  registerSearchIPC(ctx)
  registerPdfIPC(ctx)
  registerSettingsIPC(ctx)
  registerWidgetsIPC(ctx)
  registerDictationIPC(ctx)
}
