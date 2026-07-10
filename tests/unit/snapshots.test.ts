/**
 * snapshots.test.ts
 *
 * Unit tests for file snapshot logic.
 *
 * Requirements: 29.1, 29.2, 29.3, 29.4, 29.9
 */

import { describe, it, expect } from 'vitest'
import { getSnapshotPath, getSnapshotDir } from '../../src/main/snapshots'

describe('Snapshot paths', () => {
  it('constructs snapshot directory path', () => {
    const result = getSnapshotDir('/vault')
    expect(result).toBe('/vault/.nabu/snapshots')
  })

  it('constructs snapshot file path with timestamp', () => {
    const result = getSnapshotPath('/vault', '/vault/notes/note.md', 1234567890)
    expect(result).toContain('notes')
    expect(result).toContain('1234567890')
  })

  it('handles root-level notes', () => {
    const result = getSnapshotPath('/vault', '/vault/Welcome.md', 1234567890)
    expect(result).toContain('Welcome-1234567890.json')
  })
})
