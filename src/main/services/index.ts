/**
 * index.ts
 *
 * Public API entry point for the main services module.
 *
 * This module re-exports only the intentional public APIs of each service.
 * Internal implementation details are not exported.
 *
 * Import rules:
 *   - Main process: import { X } from '@main/services'
 *   - Preload/Renderer: Must use IPC channels, not direct service imports
 */

// Service classes (exported for instantiation in index.ts)
export { StateManager } from './state'
export { VectorManager } from './vector'
export { VaultWatcher } from './watcher'
export { VaultService } from './vault-service'
export { VaultRegistry, vaultRegistry } from './vault-registry'
export { SearchService } from './search-service'
export { widgetManager } from './widget-manager'
export { WorkspaceService } from './workspace-service'

// Settings functions
export { loadSettings, saveSettings, DEFAULT_SETTINGS, settingsPath } from './settings'

// Types
export type { WatcherConfig } from './watcher'
export type { VaultOpenOptions, VaultOpenResult } from './vault-service'

// Note: Individual service methods are accessed via IPC channels in the renderer.
// Direct service imports should only be used in the main process bootstrap (index.ts).