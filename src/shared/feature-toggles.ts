/**
 * feature-toggles.ts
 *
 * Feature toggle registry for optional Nabu features.
 * Each feature registers an entry with setup/teardown callbacks that are invoked
 * when the feature is toggled on/off.
 *
 * Requirements: 37.1, 37.4, 37.5, 37.6, 37.8
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FeatureToggle {
  /** Unique identifier for the feature (e.g. "daily-notes", "slash-commands"). */
  id: string
  /** Human-readable label displayed in the Settings UI. */
  label: string
  /** Short description explaining what the feature does. */
  description: string
  /** Called when the feature is toggled ON. Registers commands, panels, shortcuts. */
  setup?: () => void
  /** Called when the feature is toggled OFF. Unregisters commands, hides panels. */
  teardown?: () => void
}

// ---------------------------------------------------------------------------
// Default feature states
// ---------------------------------------------------------------------------

/**
 * Default toggle states for features.
 * - v1 established features (templates, word count) default ON
 * - New v2 features default OFF (Req 37.7)
 */
export const DEFAULT_FEATURE_STATES: Record<string, boolean> = {
  'daily-notes': true,
  templates: true,
  'random-note': true,
  'unique-note': false,
  'slash-commands': false,
  'page-preview': false,
  'audio-recorder': false,
  'word-count': true,
  'file-recovery': false,
  'format-converter': false,
  'format-import': false
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

const toggles = new Map<string, FeatureToggle>()

/**
 * Register a feature toggle. If a toggle with the same `id` already exists,
 * it will be overwritten.
 *
 * Requirements: 37.6
 */
export function registerFeatureToggle(toggle: FeatureToggle): void {
  toggles.set(toggle.id, toggle)
}

/**
 * Get all registered feature toggles.
 */
export function getFeatureToggles(): FeatureToggle[] {
  return Array.from(toggles.values())
}

/**
 * Get a specific feature toggle by id.
 */
export function getFeatureToggle(id: string): FeatureToggle | undefined {
  return toggles.get(id)
}

/**
 * Check if a feature is registered.
 */
export function hasFeatureToggle(id: string): boolean {
  return toggles.has(id)
}

/**
 * Remove a registered feature toggle.
 */
export function unregisterFeatureToggle(id: string): void {
  toggles.delete(id)
}

/**
 * Initialize a feature's setup if it's enabled. Called on app startup
 * to restore enabled features.
 */
export function initializeEnabledFeatures(): void {
  for (const toggle of toggles.values()) {
    const isEnabled = getDefaultState(toggle.id)
    if (isEnabled && toggle.setup) {
      try {
        toggle.setup()
      } catch (err) {
        console.error(`[FeatureToggles] Setup failed for "${toggle.id}":`, err)
      }
    }
  }
}

/**
 * Execute setup or teardown for a feature based on the enabled state.
 */
export function setFeatureEnabled(id: string, enabled: boolean): void {
  const toggle = toggles.get(id)
  if (!toggle) return

  if (enabled) {
    if (toggle.setup) {
      try {
        toggle.setup()
      } catch (err) {
        console.error(`[FeatureToggles] Setup failed for "${id}":`, err)
      }
    }
  } else {
    if (toggle.teardown) {
      try {
        toggle.teardown()
      } catch (err) {
        console.error(`[FeatureToggles] Teardown failed for "${id}":`, err)
      }
    }
  }
}

/**
 * Get the default state for a feature.
 * Requirements: 37.3, 37.7
 */
export function getDefaultState(id: string): boolean {
  return DEFAULT_FEATURE_STATES[id] ?? false
}

// ---------------------------------------------------------------------------
// Reset (for testing)
// ---------------------------------------------------------------------------

/**
 * Remove all registered toggles. Useful for testing.
 */
export function resetFeatureToggles(): void {
  toggles.clear()
}
