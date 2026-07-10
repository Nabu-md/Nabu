/**
 * feature-registrations.ts
 *
 * Registers all optional features in the feature toggle system.
 * Each feature wires its commands, panels, and shortcuts into the toggle lifecycle.
 *
 * Requirements: 37.2, 37.3, 37.7, 37.9
 */

import { registerFeatureToggle } from '../../../shared/feature-toggles'
import { registerCommand, unregisterCommand } from './registry'

// Track registered command IDs per feature for teardown
const featureCommandMap = new Map<string, string[]>()

// ---------------------------------------------------------------------------
// Daily Notes Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'daily-notes',
  label: 'Daily Notes',
  description: "Open or create today's daily journal note",
  setup: () => {
    registerCommand({
      id: 'note.daily',
      label: 'Open daily note',
      keywords: ['daily', 'today', 'journal'],
      run: () => {
        // The actual run is provided at runtime via options in seedCommands
        // But we register the command here for the toggle system
        const customEvent = new CustomEvent('note:daily')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('daily-notes', ['note.daily'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('daily-notes') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('daily-notes')
  }
})

// ---------------------------------------------------------------------------
// Templates Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'templates',
  label: 'Templates',
  description: 'Create notes from templates in _templates/ folder',
  setup: () => {
    // Templates are used via the note:create flow - no dedicated command
    // Just register a placeholder for toggle state
  },
  teardown: () => {
    // No commands to unregister
  }
})

// ---------------------------------------------------------------------------
// Random Note Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'random-note',
  label: 'Random Note',
  description: 'Open a random note from your vault',
  setup: () => {
    registerCommand({
      id: 'note.random',
      label: 'Open random note',
      keywords: ['random', 'serendipity'],
      run: () => {
        const customEvent = new CustomEvent('note:random')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('random-note', ['note.random'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('random-note') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('random-note')
  }
})

// ---------------------------------------------------------------------------
// Unique Note Creator Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'unique-note',
  label: 'Unique Note Creator',
  description: 'Create notes with timestamp-based unique names',
  setup: () => {
    registerCommand({
      id: 'note.unique',
      label: 'Create unique note',
      keywords: ['unique', 'timestamp', 'zettelkasten'],
      run: () => {
        const customEvent = new CustomEvent('note:unique')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('unique-note', ['note.unique'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('unique-note') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('unique-note')
  }
})

// ---------------------------------------------------------------------------
// Slash Commands Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'slash-commands',
  label: 'Slash Commands',
  description: 'Inline autocomplete for markdown structures (headings, lists, callouts, etc.)',
  setup: () => {
    // Slash commands are handled via keyboard event handler in NoteView
    // Register a command to indicate the feature is active
    registerCommand({
      id: 'slash-commands.info',
      label: 'Type / for slash commands',
      keywords: ['slash', 'commands', 'autocomplete'],
      run: () => {
        // No-op - slash commands are always available in edit mode when enabled
      }
    })
    featureCommandMap.set('slash-commands', ['slash-commands.info'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('slash-commands') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('slash-commands')
  }
})

// ---------------------------------------------------------------------------
// Page Preview Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'page-preview',
  label: 'Page Preview on Hover',
  description: 'Preview linked notes on hover without navigating away',
  setup: () => {
    // Page preview is handled via mouse events in NoteView
    // Register a command to indicate the feature is active
    registerCommand({
      id: 'page-preview.info',
      label: 'Hover over links for preview',
      keywords: ['preview', 'hover', 'page'],
      run: () => {
        // No-op - page preview is always available in view mode when enabled
      }
    })
    featureCommandMap.set('page-preview', ['page-preview.info'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('page-preview') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('page-preview')
  }
})

// ---------------------------------------------------------------------------
// Audio Recorder Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'audio-recorder',
  label: 'Audio Recorder',
  description: 'Record audio directly into notes',
  setup: () => {
    registerCommand({
      id: 'audio-recorder.insert',
      label: 'Insert audio recording',
      keywords: ['audio', 'record', 'voice'],
      run: () => {
        const customEvent = new CustomEvent('audio-recorder:insert')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('audio-recorder', ['audio-recorder.insert'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('audio-recorder') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('audio-recorder')
  }
})

// ---------------------------------------------------------------------------
// Word Count Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'word-count',
  label: 'Word Count',
  description: 'Display word and character count in status bar',
  setup: () => {
    // Word count is displayed in NoteView when enabled
  },
  teardown: () => {
    // No commands to unregister
  }
})

// ---------------------------------------------------------------------------
// File Recovery Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'file-recovery',
  label: 'File Recovery',
  description: 'Automatic snapshots for file recovery and version history',
  setup: () => {
    registerCommand({
      id: 'file-recovery.open',
      label: 'Open file recovery',
      keywords: ['recovery', 'snapshot', 'version'],
      run: () => {
        const customEvent = new CustomEvent('file-recovery:open')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('file-recovery', ['file-recovery.open'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('file-recovery') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('file-recovery')
  }
})

// ---------------------------------------------------------------------------
// Format Converter Feature
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'format-converter',
  label: 'Format Converter',
  description: 'Import notes from Notion, Roam, and Evernote',
  setup: () => {
    registerCommand({
      id: 'format-converter.import',
      label: 'Import notes',
      keywords: ['import', 'notion', 'roam', 'evernote', 'convert'],
      run: () => {
        const customEvent = new CustomEvent('format-converter:import')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('format-converter', ['format-converter.import'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('format-converter') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('format-converter')
  }
})

// ---------------------------------------------------------------------------
// Format Import Feature (PDF, DOCX, CSV)
// ---------------------------------------------------------------------------

registerFeatureToggle({
  id: 'format-import',
  label: 'Format Import',
  description: 'Import PDF, DOCX, and CSV files as markdown notes',
  setup: () => {
    registerCommand({
      id: 'format-import.open',
      label: 'Import file…',
      keywords: ['import', 'pdf', 'docx', 'csv'],
      run: () => {
        const customEvent = new CustomEvent('format-import:open')
        window.dispatchEvent(customEvent)
      }
    })
    featureCommandMap.set('format-import', ['format-import.open'])
  },
  teardown: () => {
    const ids = featureCommandMap.get('format-import') ?? []
    for (const id of ids) {
      unregisterCommand(id)
    }
    featureCommandMap.delete('format-import')
  }
})

// ---------------------------------------------------------------------------
// Export function for initialization
// ---------------------------------------------------------------------------

export function initializeFeatureToggles(): void {
  // Import and initialize enabled features on app startup
  const { initializeEnabledFeatures } = require('../../shared/feature-toggles')
  initializeEnabledFeatures()
}

export function resetFeatureRegistrations(): void {
  // Clear all feature registrations (for testing)
  const features = [
    'daily-notes',
    'templates',
    'random-note',
    'unique-note',
    'slash-commands',
    'page-preview',
    'audio-recorder',
    'word-count',
    'file-recovery',
    'format-converter',
    'format-import'
  ]
  for (const id of features) {
    const ids = featureCommandMap.get(id) ?? []
    for (const cmdId of ids) {
      unregisterCommand(cmdId)
    }
    featureCommandMap.delete(id)
  }
  const { resetFeatureToggles } = require('../../shared/feature-toggles')
  resetFeatureToggles()
}
