import { describe, expect, it } from 'vitest'
import type { VaultOption } from '../components/status-bar/types'
import { activeGitRepositories, validGitRepositoryPath } from './gitRepositories'

const vaults: VaultOption[] = [
  { label: 'Default', path: '/default', available: true, mounted: true },
  { label: 'Research', path: '/research', available: true, mounted: true },
  { label: 'Archive', path: '/archive', available: true, mounted: false },
  { label: 'Missing', path: '/missing', available: false, mounted: true },
]

describe('activeGitRepositories', () => {
  it('keeps Git single-repository until multiple workspaces are enabled', () => {
    expect(activeGitRepositories({
      defaultVaultPath: '/default',
      multiWorkspaceEnabled: false,
      vaults,
    }).map((repository) => repository.path)).toEqual(['/default'])
  })

  it('returns default plus mounted available workspaces when enabled', () => {
    expect(activeGitRepositories({
      defaultVaultPath: '/default',
      multiWorkspaceEnabled: true,
      vaults,
    }).map((repository) => repository.path)).toEqual(['/default', '/research'])
  })

  it('includes mounted managed defaults in the repository picker', () => {
    expect(activeGitRepositories({
      defaultVaultPath: '/research',
      multiWorkspaceEnabled: true,
      vaults: [
        { ...vaults[0], managedDefault: true },
        vaults[1],
      ],
    }).map((repository) => repository.path)).toEqual(['/research', '/default'])
  })

  it('keeps the default repository even if its stored mounted flag is false', () => {
    expect(activeGitRepositories({
      defaultVaultPath: '/archive',
      multiWorkspaceEnabled: true,
      vaults,
    }).map((repository) => repository.path)).toEqual(['/archive', '/default', '/research'])
  })

  it('falls back to the path name when native omits a repository label', () => {
    expect(activeGitRepositories({
      defaultVaultPath: '/Users/demo/workspace/laputa',
      multiWorkspaceEnabled: true,
      vaults: [{ path: '/Users/demo/workspace/laputa', available: true, mounted: true } as VaultOption],
    })).toEqual([{
      path: '/Users/demo/workspace/laputa',
      label: 'laputa',
      defaultForNewNotes: true,
    }])
  })
})

describe('validGitRepositoryPath', () => {
  const repositories = [
    { path: '/default', label: 'Default', defaultForNewNotes: true },
    { path: '/research', label: 'Research', defaultForNewNotes: false },
  ]

  it('keeps a selected repository when it is still available', () => {
    expect(validGitRepositoryPath('/research', repositories, '/default')).toBe('/research')
  })

  it('falls back to the default repository when selection disappears', () => {
    expect(validGitRepositoryPath('/missing', repositories, '/default')).toBe('/default')
  })
})
