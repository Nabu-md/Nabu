import { describe, expect, it } from 'vitest'
import {
  buildMiniAppWindowUrl,
  isMiniAppWindow,
  readMiniAppWindowParams,
} from './miniAppWindow'

describe('miniAppWindow', () => {
  it('detects mini-app windows from the URL query', () => {
    expect(isMiniAppWindow('?window=mini-app&appId=hello-world')).toBe(true)
    expect(isMiniAppWindow('?window=note&path=/vault/a.md')).toBe(false)
    expect(isMiniAppWindow('')).toBe(false)
  })

  it('reads packaged context from the query params', () => {
    const params = readMiniAppWindowParams(
      '?window=mini-app&appId=hello-world&vault=/Users/luca/Vault&note=/Users/luca/Vault/notes/a.md&title=My+Note',
    )
    expect(params).toEqual({
      appId: 'hello-world',
      vaultPath: '/Users/luca/Vault',
      notePath: '/Users/luca/Vault/notes/a.md',
      noteTitle: 'My Note',
      context: undefined,
    })
  })

  it('returns null when the app id is missing', () => {
    expect(readMiniAppWindowParams('?window=mini-app')).toBeNull()
  })

  it('parses JSON context payloads', () => {
    const params = readMiniAppWindowParams(
      `?window=mini-app&appId=demo&context=${encodeURIComponent(JSON.stringify({ seed: 42 }))}`,
    )
    expect(params?.context).toEqual({ seed: 42 })
  })

  it('builds launcher URLs with vault and note context', () => {
    const url = buildMiniAppWindowUrl('hello-world', '/Users/luca/Vault', {
      note_path: '/Users/luca/Vault/notes/a.md',
      note_title: 'My Note',
    })
    const params = new URLSearchParams(url.replace(/^\//, ''))
    expect(params.get('window')).toBe('mini-app')
    expect(params.get('appId')).toBe('hello-world')
    expect(params.get('vault')).toBe('/Users/luca/Vault')
    expect(params.get('note')).toBe('/Users/luca/Vault/notes/a.md')
    expect(params.get('title')).toBe('My Note')
  })
})