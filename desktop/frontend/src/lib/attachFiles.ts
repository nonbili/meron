import type { ComposerAttachment } from '../types'
import { showToast } from '../states/ui'
import { t } from './i18n'

// What a message can carry, however the files get there: picked, pasted or
// dropped. Past these sizes the app stalls on the file, and no mail server
// takes it. Go applies the same limits to the dropped files it reads for the
// page (filedrop.go).
export const MAX_ATTACHMENT_BYTES = 100 * 1024 * 1024
export const MAX_ATTACHMENTS_TOTAL_BYTES = 150 * 1024 * 1024

// Bytes taken for a message but still being read, by message (`key`). Without
// them, files added in two goes before the first has landed (images and other
// files of one drop) would each be measured against an empty message.
const pendingBytes = new Map<string, number>()

/** The files that still fit the message `key`, which already carries
 *  `attachedBytes`. Each one taken is counted until settleAttachment says it
 *  landed, or failed. */
export function takeWithinAttachmentLimits(key: string, files: File[], attachedBytes: number): File[] {
  const pending = pendingBytes.get(key) ?? 0
  let total = attachedBytes + pending
  const taken = files.filter((file) => {
    if (file.size > MAX_ATTACHMENT_BYTES || total + file.size > MAX_ATTACHMENTS_TOTAL_BYTES) return false
    total += file.size
    return true
  })
  if (taken.length > 0) pendingBytes.set(key, total - attachedBytes)
  return taken
}

/** A file taken by takeWithinAttachmentLimits is attached now, or won't be. */
export function settleAttachment(key: string, file: File) {
  const pending = (pendingBytes.get(key) ?? 0) - file.size
  if (pending > 0) pendingBytes.set(key, pending)
  else pendingBytes.delete(key)
}

/** Tells the reader that some files were left out: too large, a folder,
 *  unreadable, or meant for a message that is no longer open. */
export function reportUnattachedFiles() {
  showToast(t('chat.filesNotAttached'), 'error')
}

/** Reads a file into an attachment, and its data: URL for showing it. Rejects
 *  for a folder or a file that can't be read. */
export function readAttachment(file: File): Promise<{ attachment: ComposerAttachment; dataUrl: string }> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(reader.error ?? new Error('Could not read file'))
    reader.onload = () => {
      const dataUrl = String(reader.result || '')
      const ext = file.type.startsWith('image/') ? file.type.slice(6) : ''
      resolve({
        dataUrl,
        attachment: {
          id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
          filename: file.name || (ext ? `pasted-image-${Date.now()}.${ext}` : `pasted-${Date.now()}`),
          mime: file.type || 'application/octet-stream',
          size: file.size,
          data: dataUrl.split(',')[1] ?? '',
        },
      })
    }
    reader.readAsDataURL(file)
  })
}
