/**
 * index.ts
 *
 * Public API entry point for the shared module.
 *
 * This module re-exports all public types, schemas, and utilities that are
 * safe for both main and renderer processes to import.
 *
 * Import rules:
 *   - Main process: import { X } from '@shared/*'
 *   - Renderer process: import { X } from '@shared/*' (types/schemas only)
 *
 * The renderer must NOT import:
 *   - @shared/events (main-process only event bus)
 *   - @shared/plugins (remark plugins for main process)
 */

// Types (canonical source - types.ts)
export * from './types'

// Channels
export * from './channels'

// Schemas
export * from './schemas'

// Indexing utilities
export * from './indexing'

// Extended indexing utilities
export * from './extended-indexing'

// Graph utilities
export * from './graph'

// Search query utilities
export * from './search-query'

// Path utilities
export * from './path'

// Markdown utilities
export * from './markdown'

// Note: @shared/events is NOT exported - it is main-process only
// Note: @shared/plugins is NOT exported - it is main-process only
// Note: @shared/contracts is NOT exported - use @shared/schemas instead
// Note: @shared/models is NOT re-exported - types are in types.ts
// Note: @shared/feature-toggles is NOT re-exported - use @shared/feature-toggles directly