/**
 * registry.test.ts
 *
 * Tests for the command registry module.
 *
 * Requirements: 5.2, 5.6
 */

import { describe, it, expect, beforeEach } from 'vitest'
import {
  resetRegistry,
  registerCommand,
  getCommands,
  unregisterCommand,
  seedCommands,
  type Command
} from '../../src/renderer/src/commands/registry'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let dispatched: unknown[] = []

function mockDispatch(action: unknown): void {
  dispatched.push(action)
}

beforeEach(() => {
  dispatched = []
  resetRegistry()
})

// ---------------------------------------------------------------------------
// Basic registry operations
// ---------------------------------------------------------------------------

describe('registerCommand / getCommands', () => {
  it('returns an empty array when no commands are registered', () => {
    expect(getCommands()).toEqual([])
  })

  it('registers a single command', () => {
    const cmd: Command = { id: 'test.cmd', label: 'Test', run: () => {} }
    registerCommand(cmd)
    expect(getCommands()).toHaveLength(1)
    expect(getCommands()[0].id).toBe('test.cmd')
  })

  it('registers multiple commands in order', () => {
    registerCommand({ id: 'a', label: 'A', run: () => {} })
    registerCommand({ id: 'b', label: 'B', run: () => {} })
    expect(getCommands()).toHaveLength(2)
    expect(getCommands().map((c) => c.id)).toEqual(['a', 'b'])
  })

  it('overwrites a command with the same id', () => {
    registerCommand({ id: 'dup', label: 'First', run: () => {} })
    registerCommand({ id: 'dup', label: 'Second', run: () => {} })
    expect(getCommands()).toHaveLength(1)
    expect(getCommands()[0].label).toBe('Second')
  })

  it('includes keywords when provided', () => {
    const cmd: Command = {
      id: 'test.kw',
      label: 'Test keywords',
      keywords: ['hello', 'world'],
      run: () => {}
    }
    registerCommand(cmd)
    expect(getCommands()[0].keywords).toEqual(['hello', 'world'])
  })
})

// ---------------------------------------------------------------------------
// unregisterCommand
// ---------------------------------------------------------------------------

describe('unregisterCommand', () => {
  it('removes a registered command', () => {
    registerCommand({ id: 'remove.me', label: 'Remove', run: () => {} })
    expect(getCommands()).toHaveLength(1)
    unregisterCommand('remove.me')
    expect(getCommands()).toHaveLength(0)
  })

  it('does nothing when id does not exist', () => {
    registerCommand({ id: 'keep', label: 'Keep', run: () => {} })
    unregisterCommand('nonexistent')
    expect(getCommands()).toHaveLength(1)
  })
})

// ---------------------------------------------------------------------------
// seedCommands
// ---------------------------------------------------------------------------

describe('seedCommands', () => {
  it('registers all built-in commands', () => {
    seedCommands(mockDispatch as unknown as React.Dispatch<never>)
    expect(getCommands().length).toBeGreaterThanOrEqual(9)
  })

  it('includes the "Go to note…" command', () => {
    seedCommands(mockDispatch as unknown as React.Dispatch<never>)
    const cmd = getCommands().find((c) => c.id === 'switcher.open')
    expect(cmd).toBeDefined()
    expect(cmd!.label).toBe('Go to note…')
  })

  it('switcher.open dispatches QUICK_SWITCHER_OPEN when no option passed', () => {
    seedCommands(mockDispatch as unknown as React.Dispatch<never>)
    const cmd = getCommands().find((c) => c.id === 'switcher.open')!
    cmd.run()
    expect(dispatched).toContainEqual({ type: 'QUICK_SWITCHER_OPEN' })
  })

  it('switcher.open calls the option callback when provided', () => {
    let called = false
    seedCommands(mockDispatch as unknown as React.Dispatch<never>, {
      openQuickSwitcher: () => {
        called = true
      }
    })
    const cmd = getCommands().find((c) => c.id === 'switcher.open')!
    cmd.run()
    expect(called).toBe(true)
  })

  it('graph.toggle dispatches GRAPH_VIEW_TOGGLE', () => {
    seedCommands(mockDispatch as unknown as React.Dispatch<never>)
    const cmd = getCommands().find((c) => c.id === 'graph.toggle')!
    cmd.run()
    expect(dispatched).toContainEqual({ type: 'GRAPH_VIEW_TOGGLE' })
  })

  it('vector.reindex calls the reindex option when provided', () => {
    let called = false
    seedCommands(mockDispatch as unknown as React.Dispatch<never>, {
      reindexVector: () => {
        called = true
      }
    })
    const cmd = getCommands().find((c) => c.id === 'vector.reindex')!
    cmd.run()
    expect(called).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// resetRegistry
// ---------------------------------------------------------------------------

describe('resetRegistry', () => {
  it('removes all built-in commands', () => {
    seedCommands(mockDispatch as unknown as React.Dispatch<never>)
    expect(getCommands().length).toBeGreaterThan(0)
    resetRegistry()
    expect(getCommands()).toEqual([])
  })
})
