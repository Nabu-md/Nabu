import { isTauri } from '../mock-tauri'
import {
  isPortableAttachmentPath,
  isVaultAttachmentUrl,
  vaultAttachmentAssetUrl,
} from './vaultAttachments'

/** Frontmatter key storing the note cover image source. */
export const COVER_IMAGE_PROPERTY_KEY = 'cover_image'

/**
 * True when the value can be rendered as a note cover image source.
 * Accepts http(s)/data/asset URLs and portable `attachments/...` paths.
 */
export function isCoverImageSource(value: unknown): boolean {
  if (typeof value !== 'string') return false
  const trimmed = value.trim()
  if (trimmed === '') return false
  if (hasUnsafeRelativeSegment(trimmed)) return false
  if (/^(https?:|data:image\/|asset:)/iu.test(trimmed)) return true
  return isVaultAttachmentUrl({ url: trimmed })
}

function hasUnsafeRelativeSegment(path: string): boolean {
  return path.split(/[\\/]/u).some((segment) => segment === '..')
}

/**
 * Resolve a `cover_image` frontmatter value to a displayable image source.
 * URLs pass through unchanged; portable `attachments/...` paths resolve to
 * Tauri asset URLs inside the app and stay portable outside it. Returns null
 * when the value cannot be rendered safely.
 */
export function coverImageSource({ value, vaultPath }: { value: unknown; vaultPath?: string }): string | null {
  if (!isCoverImageSource(value) || typeof value !== 'string') return null
  const trimmed = value.trim()

  if (/^(https?:|data:image\/|asset:)/iu.test(trimmed)) return trimmed
  if (!isPortableAttachmentPath({ path: trimmed })) return null
  if (!isTauri() || !vaultPath) return trimmed

  return vaultAttachmentAssetUrl({ attachmentPath: trimmed, vaultPath })
}
