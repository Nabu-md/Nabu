/**
 * registry.ts
 *
 * Shared command registry extension point for the Command Palette.
 * Feature modules register their commands at import time via
 * `registerCommand()`. The Command Palette reads all commands
 * via `getCommands()` and fuzzy-filters them.
 *
 * Requirements: 5.2, 5.3, 5.6
 */

import type { AppAction } from '../App'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Command {
  /** Unique identifier (e.g. "note.toggle-edit", "app.open-settings"). */
  id: string
  /** Human-readable label displayed in the Command Palette. */
  label: string
  /** Extra keywords for fuzzy matching beyond id/label. */
  keywords?: string[]
  /** Action to execute when the command is selected. */
  run: () => void
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

const commands = new Map<string, Command>()

/**
 * Register a command. If a command with the same `id` already exists it will
 * be overwritten, allowing later modules to replace built-in commands.
 */
export function registerCommand(cmd: Command): void {
  commands.set(cmd.id, cmd)
}

/**
 * Return all registered commands as an array (order is registration order).
 */
export function getCommands(): Command[] {
  return Array.from(commands.values())
}

/**
 * Remove a previously registered command by `id`.
 * Useful for feature-toggle teardown.
 */
export function unregisterCommand(id: string): void {
  commands.delete(id)
}

// ---------------------------------------------------------------------------
// Built-in seed commands
// ---------------------------------------------------------------------------

/**
 * Seed the registry with built-in v1 and v2 commands that map to
 * existing AppState actions.
 *
 * Call this once from App.tsx when the app mounts.
 *
 * @param dispatch  The `useReducer` dispatch for AppAction.
 * @param options   Optional overrides (see below).
 */
export function seedCommands(
  dispatch: React.Dispatch<AppAction>,
  options?: {
    /** Called when the Quick Switcher should open (Cmd+O). */
    openQuickSwitcher?: () => void
    /** Called when "Create daily note" is invoked. */
    createDailyNote?: () => void
    /** Called when "Open random note" is invoked. */
    openRandomNote?: () => void
    /** Called to trigger a full vector reindex. */
    reindexVector?: () => void
    /** Called to toggle favorite state for the current file. */
    toggleFavorite?: () => void
  }
): void {
  const o = options ?? {}

  registerCommand({
    id: 'favorites.toggle',
    label: 'Toggle favorite',
    keywords: ['favorite', 'star', 'bookmark'],
    run: () => {
      if (o.toggleFavorite) {
        o.toggleFavorite()
      }
    }
  })

  registerCommand({
    id: 'edit.toggle',
    label: 'Toggle edit / view mode',
    keywords: ['edit', 'view', 'toggle'],
    run: () => dispatch({ type: 'EDIT_MODE_EXIT' })
  })

  registerCommand({
    id: 'graph.toggle',
    label: 'Toggle graph view',
    keywords: ['graph', 'toggle'],
    run: () => dispatch({ type: 'GRAPH_VIEW_TOGGLE' })
  })

  registerCommand({
    id: 'search.toggle',
    label: 'Toggle search panel',
    keywords: ['search', 'find'],
    run: () => dispatch({ type: 'SEARCH_PANEL_TOGGLE' })
  })

  registerCommand({
    id: 'switcher.open',
    label: 'Go to note…',
    keywords: ['switcher', 'quick', 'open', 'navigate'],
    run: () => {
      if (o.openQuickSwitcher) {
        o.openQuickSwitcher()
      } else {
        dispatch({ type: 'QUICK_SWITCHER_OPEN' })
      }
    }
  })

  registerCommand({
    id: 'settings.open',
    label: 'Open settings',
    keywords: ['settings', 'preferences', 'config'],
    run: () => dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
  })

  registerCommand({
    id: 'note.create',
    label: 'Create new note',
    keywords: ['new', 'create', 'note'],
    run: () => {
      // Dispatch a no-op placeholder — the actual file-creation flow
      // requires user input (name, template). The file-tree context
      // menu handles the interactive flow.
      dispatch({ type: 'SETUP_TOGGLE' })
    }
  })

  registerCommand({
    id: 'note.daily',
    label: 'Open daily note',
    keywords: ['daily', 'today', 'journal'],
    run: () => {
      if (o.createDailyNote) {
        o.createDailyNote()
      }
    }
  })

  registerCommand({
    id: 'note.random',
    label: 'Open random note',
    keywords: ['random', 'serendipity'],
    run: () => {
      if (o.openRandomNote) {
        o.openRandomNote()
      }
    }
  })

  registerCommand({
    id: 'vector.reindex',
    label: 'Reindex vector search',
    keywords: ['reindex', 'vector', 'embedding', 'rebuild'],
    run: () => {
      if (o.reindexVector) {
        o.reindexVector()
      } else {
        // Fallback — open settings so the user can trigger reindex
        dispatch({ type: 'SETTINGS_PANEL_TOGGLE' })
      }
    }
  })
}

// ---------------------------------------------------------------------------
// Pane commands (Req 24.3, 24.6, 24.7)
// ---------------------------------------------------------------------------

export function seedPaneCommands(
  _dispatch: React.Dispatch<AppAction>,
  options?: {
    /** Close the current tab. */
    closeCurrentTab?: () => void
    /** Move tab to a new pane. */
    moveToNewPane?: () => void
    /** Focus next pane. */
    focusNextPane?: () => void
  }
): void {
  const o = options ?? {}

  registerCommand({
    id: 'pane.close-tab',
    label: 'Close tab',
    keywords: ['tab', 'close', 'pane'],
    run: () => {
      if (o.closeCurrentTab) {
        o.closeCurrentTab()
      }
    }
  })

  registerCommand({
    id: 'pane.next',
    label: 'Next pane',
    keywords: ['pane', 'next', 'focus'],
    run: () => {
      if (o.focusNextPane) {
        o.focusNextPane()
      }
    }
  })

  registerCommand({
    id: 'pane.move-tab',
    label: 'Move tab to new pane',
    keywords: ['tab', 'move', 'pane', 'split'],
    run: () => {
      if (o.moveToNewPane) {
        o.moveToNewPane()
      }
    }
  })
}

/**
 * Remove all built-in seed commands.
 * Used for testing or full reset.
 */
export function clearSeedCommands(): void {
  const ids = [
    'edit.toggle',
    'graph.toggle',
    'search.toggle',
    'switcher.open',
    'settings.open',
    'note.create',
    'note.daily',
    'note.random',
    'vector.reindex'
  ]
  for (const id of ids) {
    unregisterCommand(id)
  }
}

/**
 * Remove ALL registered commands. Useful for testing.
 */
export function resetRegistry(): void {
  commands.clear()
}
