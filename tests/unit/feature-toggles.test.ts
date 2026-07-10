/**
 * feature-toggles.test.ts
 *
 * Unit tests for the feature toggle registry.
 *
 * Requirements: 37.1, 37.4, 37.5, 37.6, 37.8
 */

import { describe, it, expect, beforeEach } from 'vitest'
import {
  registerFeatureToggle,
  getFeatureToggles,
  getFeatureToggle,
  hasFeatureToggle,
  unregisterFeatureToggle,
  resetFeatureToggles,
  getDefaultState
} from '../../src/shared/feature-toggles'

// ---------------------------------------------------------------------------
// Setup/Teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  resetFeatureToggles()
})

// ---------------------------------------------------------------------------
// Basic registry operations
// ---------------------------------------------------------------------------

describe('registerFeatureToggle / getFeatureToggles', () => {
  it('returns an empty array when no toggles are registered', () => {
    expect(getFeatureToggles()).toEqual([])
  })

  it('registers a single feature toggle', () => {
    registerFeatureToggle({
      id: 'test-feature',
      label: 'Test Feature',
      description: 'A test feature toggle'
    })
    expect(getFeatureToggles()).toHaveLength(1)
    expect(getFeatureToggles()[0].id).toBe('test-feature')
  })

  it('registers multiple feature toggles in order', () => {
    registerFeatureToggle({ id: 'a', label: 'A', description: 'Feature A' })
    registerFeatureToggle({ id: 'b', label: 'B', description: 'Feature B' })
    expect(getFeatureToggles()).toHaveLength(2)
    expect(getFeatureToggles().map((t) => t.id)).toEqual(['a', 'b'])
  })

  it('includes setup and teardown when provided', () => {
    let setupCalled = false
    let teardownCalled = false

    registerFeatureToggle({
      id: 'with-callbacks',
      label: 'With Callbacks',
      description: 'Feature with callbacks',
      setup: () => {
        setupCalled = true
      },
      teardown: () => {
        teardownCalled = true
      }
    })

    const toggle = getFeatureToggle('with-callbacks')
    expect(toggle?.setup).toBeDefined()
    expect(toggle?.teardown).toBeDefined()
  })
})

describe('getFeatureToggle', () => {
  it('returns undefined for non-existent toggle', () => {
    expect(getFeatureToggle('nonexistent')).toBeUndefined()
  })

  it('returns the toggle for existing id', () => {
    registerFeatureToggle({
      id: 'existing',
      label: 'Existing',
      description: 'An existing feature'
    })
    const toggle = getFeatureToggle('existing')
    expect(toggle?.id).toBe('existing')
    expect(toggle?.label).toBe('Existing')
  })
})

describe('hasFeatureToggle', () => {
  it('returns false when toggle not registered', () => {
    expect(hasFeatureToggle('unknown')).toBe(false)
  })

  it('returns true when toggle is registered', () => {
    registerFeatureToggle({ id: 'known', label: 'Known', description: 'A feature' })
    expect(hasFeatureToggle('known')).toBe(true)
  })
})

describe('unregisterFeatureToggle', () => {
  it('removes a registered toggle', () => {
    registerFeatureToggle({ id: 'remove.me', label: 'Remove', description: 'Feature' })
    expect(getFeatureToggles()).toHaveLength(1)
    unregisterFeatureToggle('remove.me')
    expect(getFeatureToggles()).toHaveLength(0)
  })

  it('does nothing when id does not exist', () => {
    registerFeatureToggle({ id: 'keep', label: 'Keep', description: 'Feature' })
    unregisterFeatureToggle('nonexistent')
    expect(getFeatureToggles()).toHaveLength(1)
  })
})

describe('getDefaultState', () => {
  it('returns false for unknown features', () => {
    expect(getDefaultState('unknown-feature')).toBe(false)
  })

  it('returns true for v1 established features (templates, word-count)', () => {
    expect(getDefaultState('templates')).toBe(true)
    expect(getDefaultState('word-count')).toBe(true)
    expect(getDefaultState('daily-notes')).toBe(true)
    expect(getDefaultState('random-note')).toBe(true)
  })

  it('returns false for new v2 features by default', () => {
    expect(getDefaultState('unique-note')).toBe(false)
    expect(getDefaultState('slash-commands')).toBe(false)
    expect(getDefaultState('page-preview')).toBe(false)
    expect(getDefaultState('audio-recorder')).toBe(false)
    expect(getDefaultState('file-recovery')).toBe(false)
    expect(getDefaultState('format-converter')).toBe(false)
    expect(getDefaultState('format-import')).toBe(false)
  })
})

describe('setup and teardown callbacks', () => {
  it('calls setup callback when setFeatureEnabled is called with true', async () => {
    let setupCalled = false
    registerFeatureToggle({
      id: 'callback-test',
      label: 'Callback Test',
      description: 'Feature with callbacks',
      setup: () => {
        setupCalled = true
      }
    })

    const { setFeatureEnabled } = await import('../../src/shared/feature-toggles')
    setFeatureEnabled('callback-test', true)
    expect(setupCalled).toBe(true)
  })

  it('calls teardown callback when setFeatureEnabled is called with false', async () => {
    let teardownCalled = false
    registerFeatureToggle({
      id: 'callback-test-2',
      label: 'Callback Test 2',
      description: 'Feature with callbacks',
      teardown: () => {
        teardownCalled = true
      }
    })

    const { setFeatureEnabled } = await import('../../src/shared/feature-toggles')
    setFeatureEnabled('callback-test-2', false)
    expect(teardownCalled).toBe(true)
  })
})
