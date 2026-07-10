/**
 * pane-commands.test.ts
 *
 * Unit tests for pane interaction commands (Req 24.3, 24.6, 24.7).
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import {
  getCommands,
  resetRegistry,
  seedPaneCommands
} from '../../src/renderer/src/commands/registry'

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Pane commands', () => {
  beforeEach(() => {
    resetRegistry()
  })

  afterEach(() => {
    resetRegistry()
  })

  it('registers close tab command', () => {
    let closed = false
    seedPaneCommands(
      () => undefined, // dispatch noop
      {
        closeCurrentTab: () => {
          closed = true
        }
      }
    )

    const cmds = getCommands()
    const closeCmd = cmds.find((c) => c.id === 'pane.close-tab')
    expect(closeCmd).toBeDefined()
    expect(closeCmd?.label).toBe('Close tab')
    closeCmd?.run()
    expect(closed).toBe(true)
  })

  it('registers next pane command', () => {
    let focused = false
    seedPaneCommands(
      () => undefined, // dispatch noop
      {
        focusNextPane: () => {
          focused = true
        }
      }
    )

    const cmds = getCommands()
    const nextCmd = cmds.find((c) => c.id === 'pane.next')
    expect(nextCmd).toBeDefined()
    expect(nextCmd?.label).toBe('Next pane')
    nextCmd?.run()
    expect(focused).toBe(true)
  })

  it('registers move tab to new pane command', () => {
    let moved = false
    seedPaneCommands(
      () => undefined, // dispatch noop
      {
        moveToNewPane: () => {
          moved = true
        }
      }
    )

    const cmds = getCommands()
    const moveCmd = cmds.find((c) => c.id === 'pane.move-tab')
    expect(moveCmd).toBeDefined()
    expect(moveCmd?.label).toBe('Move tab to new pane')
    moveCmd?.run()
    expect(moved).toBe(true)
  })

  it('commands have proper keywords for fuzzy matching', () => {
    seedPaneCommands(() => undefined)

    const cmds = getCommands()
    const closeCmd = cmds.find((c) => c.id === 'pane.close-tab')!
    const nextCmd = cmds.find((c) => c.id === 'pane.next')!

    expect(closeCmd.keywords).toContain('tab')
    expect(nextCmd.keywords).toContain('pane')
  })
})
