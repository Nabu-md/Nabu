import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import type { Mock } from 'vitest'
import { isTauri } from '../mock-tauri'
import { portableAttachmentPathFromAnyAssetUrl } from './vaultAttachments'
import { pickCoverImageFile } from './coverImagePicker'

vi.mock('../mock-tauri', () => ({
  isTauri: vi.fn(() => false),
}))

vi.mock('./vaultAttachments', async (importOriginal) => {
  const original = await importOriginal<typeof import('./vaultAttachments')>()
  return {
    ...original,
    portableAttachmentPathFromAnyAssetUrl: vi.fn(() => null),
  }
})

vi.mock('../hooks/useImageDrop', () => ({
  uploadImageFile: vi.fn(),
}))

const isTauriMock = isTauri as unknown as Mock
const portablePathMock = portableAttachmentPathFromAnyAssetUrl as unknown as Mock
const uploadImageFileMock = (await import('../hooks/useImageDrop')).uploadImageFile as unknown as Mock

function pickedFile(): File {
  return new File(['fake-image-bytes'], 'photo.png', { type: 'image/png' })
}

describe('pickCoverImageFile', () => {
  beforeEach(() => {
    isTauriMock.mockReturnValue(false)
    portablePathMock.mockReset()
    portablePathMock.mockReturnValue(null)
    uploadImageFileMock.mockReset()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('returns the portable attachments path when the user picks a file in Tauri', async () => {
    isTauriMock.mockReturnValue(true)
    uploadImageFileMock.mockResolvedValue('asset://localhost/%2Fvault%2Fattachments%2F123-photo.png')
    portablePathMock.mockReturnValueOnce('attachments/123-photo.png')

    const path = await pickCoverImageFile({
      vaultPath: '/vault',
      selectFile: async () => pickedFile(),
    })

    expect(uploadImageFileMock).toHaveBeenCalledTimes(1)
    expect(portablePathMock).toHaveBeenCalledWith({
      url: 'asset://localhost/%2Fvault%2Fattachments%2F123-photo.png',
    })
    expect(path).toBe('attachments/123-photo.png')
  })

  it('returns null when the saved path cannot be mapped into the vault', async () => {
    isTauriMock.mockReturnValue(true)
    uploadImageFileMock.mockResolvedValue('asset://localhost/%2Felsewhere%2F123-photo.png')

    const path = await pickCoverImageFile({
      vaultPath: '/vault',
      selectFile: async () => pickedFile(),
    })

    expect(path).toBeNull()
  })

  it('returns null when the upload fails', async () => {
    isTauriMock.mockReturnValue(true)
    uploadImageFileMock.mockRejectedValue(new Error('write failed'))

    const path = await pickCoverImageFile({
      vaultPath: '/vault',
      selectFile: async () => pickedFile(),
    })

    expect(path).toBeNull()
  })

  it('returns a data URL in browser mode', async () => {
    uploadImageFileMock.mockResolvedValue('data:image/png;base64,abc')

    const path = await pickCoverImageFile({
      vaultPath: '/vault',
      selectFile: async () => pickedFile(),
    })

    expect(path).toBe('data:image/png;base64,abc')
  })

  it('returns null when the file picker is cancelled', async () => {
    const path = await pickCoverImageFile({
      vaultPath: '/vault',
      selectFile: async () => null,
    })

    expect(path).toBeNull()
    expect(uploadImageFileMock).not.toHaveBeenCalled()
  })
})
