import { isTauri } from '../mock-tauri'
import { portableAttachmentPathFromAnyAssetUrl } from './vaultAttachments'
import { uploadImageFile } from '../hooks/useImageDrop'
import type { UploadImageFileResult } from '../hooks/useImageDrop'

const COVER_IMAGE_INPUT_ACCEPT = 'image/*'

/**
 * Open a native/browser file picker, store the chosen image as a vault
 * attachment, and return the portable `attachments/<file>` path to persist in
 * the `cover_image` frontmatter property. In browser mode the image is kept
 * as a data URL. Returns null when the user cancels or saving fails.
 */
export async function pickCoverImageFile({
  vaultPath,
  selectFile = selectImageFile,
}: {
  vaultPath?: string
  selectFile?: () => Promise<File | null>
}): Promise<string | null> {
  const selected = await selectFile()
  if (!selected) return null

  if (isTauri() && vaultPath) {
    try {
      const result = await uploadImageFile(selected, vaultPath)
      const savedPath = savedImagePathFromUploadResult(result)
      if (!savedPath) return null
      return portableAttachmentPathFromAnyAssetUrl({ url: savedPath })
    } catch (error) {
      console.warn('[cover-image] Failed to save cover image into the vault:', error)
      return null
    }
  }

  const dataUrl = await uploadImageFile(selected)
  return typeof dataUrl === 'string' ? dataUrl : dataUrl.props.url
}

function savedImagePathFromUploadResult(result: UploadImageFileResult): string | null {
  return typeof result === 'string' ? result : result.props.url
}

function selectImageFile(): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = COVER_IMAGE_INPUT_ACCEPT
    input.addEventListener('change', () => {
      resolve(input.files?.[0] ?? null)
      input.remove()
    })
    input.addEventListener('cancel', () => {
      resolve(null)
      input.remove()
    })
    input.click()
  })
}
