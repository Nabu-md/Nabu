import { describe, expect, it } from 'vitest'
import { canWritePathToVault, isPathInsideVaultRoot } from './vaultPathContainment'

describe('vault path containment', () => {
  it('matches expanded note paths against tilde vault roots', () => {
    expect(canWritePathToVault(
      '/Users/demo/workspace/engineering-vault/notes/example.md',
      '~/Workspace/engineering-vault',
    )).toBe(true)
  })

  it('rejects expanded paths outside a tilde vault root', () => {
    expect(isPathInsideVaultRoot(
      '/Users/demo/workspace/engineering-vault-archive/notes/example.md',
      '~/Workspace/engineering-vault',
    )).toBe(false)
  })
})
