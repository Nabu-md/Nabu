/**
 * context.ts — Shared IPC registration context.
 *
 * Carries the core managers and instantiated feature services to every
 * `register*IPC()` function. This is the single object that the previous
 * monolithic `registerIPCHandlers(stateManager, vectorManager, watcher)` used
 * to build internally; it is now constructed once in `index.ts` (the bootstrap)
 * and threaded through the feature modules.
 */

import type { StateManager } from '../services/state'
import type { VectorManager } from '../services/vector'
import type { VaultWatcher } from '../services/watcher'
import type { VaultService } from '../services/vault-service'
import type { SearchService } from '../services/search-service'
import type { PdfService } from '../services/pdf-service'
import type { DictationService } from '../services/dictation-service'

/**
 * Context passed to every feature IPC registration function.
 */
export interface IPCContext {
  stateManager: StateManager
  vectorManager: VectorManager
  watcher: VaultWatcher
  vaultService: VaultService
  searchService: SearchService
  pdfService: PdfService
  dictationService: DictationService
}
