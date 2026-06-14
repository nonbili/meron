import type { Account, Message } from '../../types'
import type { GalleryItem } from './Gallery'
import type { ConversationMediaItem, ConversationFileItem, Participant } from './ConversationDetailsPanel'
import { getVisibleMedia, mediaSrc, normalizeBodyText, parseAddressList } from './messageHelpers'

// Flat list of every visible image across the thread, plus the starting index
// for each message so a bubble can map its local image index to the global one.
export function buildGalleryItems(messages: Message[], accounts: Account[], revealedRemote: Record<string, boolean>) {
  const items: GalleryItem[] = []
  const offsets = new Map<string, number>()
  for (const message of messages) {
    offsets.set(message.id, items.length)
    const account = accounts.find((acc) => acc.id === message.account_id)
    const { attachmentImages } = getVisibleMedia(message, account, !!revealedRemote[message.id])
    const caption = normalizeBodyText(message.body).replace(/\s+/g, ' ').trim()
    for (const image of attachmentImages) {
      items.push({ src: mediaSrc(image), filename: image.filename, caption })
    }
  }
  return { galleryItems: items, galleryOffsets: offsets }
}

// Thread-wide media (images + videos) and files for the shared-media panel.
export function buildThreadMedia(messages: Message[], accounts: Account[], revealedRemote: Record<string, boolean>) {
  const media: ConversationMediaItem[] = []
  const files: ConversationFileItem[] = []
  let imageIndex = 0
  for (const message of messages) {
    const account = accounts.find((acc) => acc.id === message.account_id)
    const {
      attachmentImages,
      videos,
      files: msgFiles,
    } = getVisibleMedia(message, account, !!revealedRemote[message.id])
    for (const image of attachmentImages) {
      media.push({ type: 'image', src: mediaSrc(image), filename: image.filename, galleryIndex: imageIndex })
      imageIndex++
    }
    for (const video of videos) {
      media.push({ type: 'video', src: mediaSrc(video), filename: video.filename, url: video.url ?? mediaSrc(video) })
    }
    for (const file of msgFiles) {
      files.push({ filename: file.filename, size: file.size, key: file.key })
    }
  }
  media.reverse()
  files.reverse()
  return { mediaItems: media, fileItems: files }
}

// Thread participants: the union of every From/To/Cc across the thread, keyed
// by lowercased address. Email threads don't have a fixed roster the way a
// group chat does, so this is a "who has appeared on this conversation" list.
// RSS feeds have no real participants, so it stays empty there.
export function buildParticipants(messages: Message[], accounts: Account[], isRSS: boolean): Participant[] {
  if (isRSS) return []
  const ownEmails = new Set(accounts.map((acc) => acc.email.trim().toLowerCase()))
  const byEmail = new Map<string, Participant>()
  const add = (name: string, email: string) => {
    const key = email.trim().toLowerCase()
    if (!key) return
    const existing = byEmail.get(key)
    if (existing) {
      existing.count++
      // Upgrade a bare-address display name to a real one if we find one.
      if ((existing.name === existing.email || !existing.name) && name && name !== email) {
        existing.name = name
      }
      return
    }
    byEmail.set(key, {
      name: name && name !== email ? name : key,
      email: key,
      count: 1,
      isSelf: ownEmails.has(key),
    })
  }
  for (const message of messages) {
    add(message.from_name, message.from_addr)
    for (const entry of parseAddressList(message.to)) add(entry.name, entry.email)
    for (const entry of parseAddressList(message.cc)) add(entry.name, entry.email)
  }
  // Most-active first; keep "you" out of the top slot so the other party leads.
  return [...byEmail.values()].sort((a, b) => {
    if (a.isSelf !== b.isSelf) return a.isSelf ? 1 : -1
    return b.count - a.count
  })
}
