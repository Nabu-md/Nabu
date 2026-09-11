import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Mock } from 'vitest'
import { coverImageSource, isCoverImageSource } from './coverImage'
import { convertFileSrc } from '@tauri-apps/api/core'
import { isTauri } from '../mock-tauri'

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn((path: string) => `asset://localhost/${encodeURIComponent(path)}`),
}))

vi.mock('../mock-tauri', () => ({
  isTauri: vi.fn(() => false),
}))

const isTauriMock = isTauri as unknown as Mock
const convertFileSrcMock = convertFileSrc as unknown as Mock

describe('coverImage utils', () => {
  beforeEach(() => {
    convertFileSrcMock.mockClear()
    isTauriMock.mockReturnValue(false)
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('isCoverImageSource', () => {
    it('accepts http(s) URLs', () => {
      expect(isCoverImageSource('https://example.com/photo.jpg')).toBe(true)
      expect(isCoverImageSource('http://example.com/photo.jpg')).toBe(true)
    })

    it('accepts vault attachment paths', () => {
      expect(isCoverImageSource('attachments/photo.png')).toBe(true)
      expect(isCoverImageSource('./attachments/photo.png')).toBe(true)
      expect(isCoverImageSource('/attachments/photo.png')).toBe(true)
    })

    it('accepts data URLs and tauri asset URLs', () => {
      expect(isCoverImageSource('data:image/png;base64,abc')).toBe(true)
      expect(isCoverImageSource('asset://localhost/%2Fvault%2Fattachments%2Fphoto.png')).toBe(true)
    })

    it('rejects non-image values', () => {
      expect(isCoverImageSource('')).toBe(false)
      expect(isCoverImageSource('   ')).toBe(false)
      expect(isCoverImageSource('just-some-text')).toBe(false)
      expect(isCoverImageSource(undefined)).toBe(false)
    })
  })

  describe('coverImageSource', () => {
    it('passes through http(s) URLs unchanged', () => {
      expect(coverImageSource({ value: 'https://example.com/photo.jpg' })).toBe('https://example.com/photo.jpg')
    })

    it('passes through data URLs unchanged', () => {
      expect(coverImageSource({ value: 'data:image/png;base64,abc' })).toBe('data:image/png;base64,abc')
    })

    it('resolves vault attachment paths against the vault in Tauri', () => {
      isTauriMock.mockReturnValue(true)
      expect(
        coverImageSource({ value: 'attachments/photo.png', vaultPath: '/Users/me/vault' }),
      ).toBe('asset://localhost/' + encodeURIComponent('/Users/me/vault/attachments/photo.png'))
      expect(convertFileSrcMock).toHaveBeenCalledWith('/Users/me/vault/attachments/photo.png')
    })

    it('returns portable attachment paths unchanged outside Tauri', () => {
      expect(coverImageSource({ value: 'attachments/photo.png', vaultPath: '/Users/me/vault' })).toBe(
        'attachments/photo.png',
      )
    })

    it('returns null for unsafe values', () => {
      expect(coverImageSource({ value: 'not-a-cover', vaultPath: '/v' })).toBeNull()
      expect(coverImageSource({ value: 'attachments/../../secrets.png', vaultPath: '/v' })).toBeNull()
    })
  })
})
