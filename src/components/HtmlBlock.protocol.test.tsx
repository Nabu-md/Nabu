import { beforeEach, describe, expect, it, vi } from 'vitest'
import { htmlBlockFrameSource } from '../utils/htmlBlockSandbox'

const { convertFileSrcMock, isTauriMock } = vi.hoisted(() => ({
  convertFileSrcMock: vi.fn((path: string, protocol: string) => `${protocol}://localhost/${path}`),
  isTauriMock: vi.fn(() => true),
}))

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: convertFileSrcMock,
}))

vi.mock('../mock-tauri', () => ({
  isTauri: isTauriMock,
}))

describe('HTML block protocol routing', () => {
  beforeEach(() => {
    convertFileSrcMock.mockClear()
    isTauriMock.mockReturnValue(true)
  })

  it('routes installed sandboxed previews through the isolated protocol', () => {
    const previewSource = htmlBlockFrameSource('Safe preview', 'browser-source', 'sandboxed') ?? ''

    expect(convertFileSrcMock).toHaveBeenCalledWith(expect.stringMatching(/^[A-Za-z0-9_-]+$/u), 'nabu-html-block')
    expect(previewSource).toMatch(/^nabu-html-block:\/\/localhost\//u)
    expect(previewSource).not.toContain('browser-source')
  })

  it('keeps static and browser previews on their existing source', () => {
    expect(htmlBlockFrameSource('Safe preview', 'browser-source', 'blocked')).toBe('browser-source')
    isTauriMock.mockReturnValue(false)
    expect(htmlBlockFrameSource('Safe preview', 'browser-source', 'sandboxed')).toBe('browser-source')
    expect(convertFileSrcMock).not.toHaveBeenCalled()
  })
})
