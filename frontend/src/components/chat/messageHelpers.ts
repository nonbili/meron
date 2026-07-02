import {
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Presentation,
} from 'lucide-react'
import type { ComponentType } from 'react'
import type { Account, Attachment, Message } from '../../types'
import { PdfIcon, type IconProps } from '../icons/Icons'

/** Any icon usable in attachment rows: a lucide icon or our custom SVGs. */
export type FileIconComponent = ComponentType<IconProps>

export const MESSAGE_BODY_MAX_HEIGHT = 360

/** Human-readable byte size: B → KB → MB → GB, trimming trailing `.0`. */
export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 KB'
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  // KB stays whole; larger units keep one decimal (e.g. 3.3 MB).
  const rounded = unit === 0 ? Math.round(value) : Math.round(value * 10) / 10
  return `${rounded} ${units[unit]}`
}

const EXT_ICONS: Record<string, FileIconComponent> = {
  pdf: PdfIcon,
  doc: FileText,
  docx: FileText,
  rtf: FileText,
  odt: FileText,
  txt: FileText,
  md: FileText,
  xls: FileSpreadsheet,
  xlsx: FileSpreadsheet,
  csv: FileSpreadsheet,
  ods: FileSpreadsheet,
  ppt: Presentation,
  pptx: Presentation,
  odp: Presentation,
  key: Presentation,
  zip: FileArchive,
  rar: FileArchive,
  '7z': FileArchive,
  tar: FileArchive,
  gz: FileArchive,
  bz2: FileArchive,
  js: FileCode,
  ts: FileCode,
  jsx: FileCode,
  tsx: FileCode,
  json: FileCode,
  html: FileCode,
  css: FileCode,
  py: FileCode,
  rs: FileCode,
  go: FileCode,
  java: FileCode,
  c: FileCode,
  cpp: FileCode,
  sh: FileCode,
  xml: FileCode,
  yml: FileCode,
  yaml: FileCode,
}

/** Pick a file icon from extension first, then mime, falling back to a generic file. */
export function fileIconFor(filename: string, mime: string): FileIconComponent {
  const ext = filename.split('.').pop()?.toLowerCase() ?? ''
  if (EXT_ICONS[ext]) return EXT_ICONS[ext]

  const m = (mime ?? '').toLowerCase()
  if (m.startsWith('image/')) return FileImage
  if (m.startsWith('video/')) return FileVideo
  if (m.startsWith('audio/')) return FileAudio
  if (m.startsWith('text/')) return FileText
  if (m === 'application/pdf') return PdfIcon
  if (m.includes('spreadsheet') || m.includes('excel')) return FileSpreadsheet
  if (m.includes('presentation') || m.includes('powerpoint')) return Presentation
  if (m.includes('zip') || m.includes('compressed') || m.includes('tar')) return FileArchive
  if (m.includes('json') || m.includes('javascript') || m.includes('xml')) return FileCode
  return File
}

export function mediaSrc(media: Attachment): string {
  return media.key ? `/media/${media.key}` : media.url!
}

export function isVideo(a: Attachment): boolean {
  return (a.mime ?? '').toLowerCase().startsWith('video/')
}

export function isImage(a: Attachment): boolean {
  return (a.mime ?? '').toLowerCase().startsWith('image/')
}

export function getVisibleMedia(message: Message, account: Account | undefined, revealed: boolean) {
  const attachments = message.attachments ?? []
  // `data:` URLs (used by optimistic sent-message previews) are local — they
  // need no remote fetch — so they render unconditionally alongside keyed media.
  const isInline = (a: Attachment) => !!a.key || !!a.url?.startsWith('data:')
  const localImages = attachments.filter((a) => isInline(a) && isImage(a))
  const remoteImages = attachments.filter((a) => !isInline(a) && a.url && isImage(a))
  const localVideos = attachments.filter((a) => isInline(a) && isVideo(a))
  const remoteVideos = attachments.filter((a) => !isInline(a) && a.url && isVideo(a))
  const files = attachments.filter((a) => !isImage(a) && !isVideo(a))
  const remoteVisible = (account?.load_remote_images ?? false) || revealed
  const attachmentImages = remoteVisible ? [...localImages, ...remoteImages] : localImages
  const videos = remoteVisible ? [...localVideos, ...remoteVideos] : localVideos
  const hiddenRemoteCount = remoteVisible ? 0 : remoteImages.length + remoteVideos.length
  return { attachmentImages, videos, hiddenRemoteCount, files }
}

// All formatters take a Unix epoch-seconds timestamp (0 = unknown) and render in
// the user's local time.

function dateOnly(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

export function formatMessageTime(epochSeconds: number): string {
  if (!epochSeconds) return ''
  const date = new Date(epochSeconds * 1000)
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })
}

export function formatDateDivider(epochSeconds: number): string {
  if (!epochSeconds) return ''
  const date = dateOnly(new Date(epochSeconds * 1000))
  const today = dateOnly(new Date())
  const yesterday = new Date(today)
  yesterday.setDate(today.getDate() - 1)
  if (date.getTime() === today.getTime()) return 'Today'
  if (date.getTime() === yesterday.getTime()) return 'Yesterday'

  return date.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' })
}

export function formatFullTimestamp(epochSeconds: number): string {
  if (!epochSeconds) return ''
  const date = new Date(epochSeconds * 1000)
  return date.toLocaleString([], {
    weekday: 'short',
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

export function formatMessageStamp(epochSeconds: number, _showDate: boolean): string {
  if (!epochSeconds) return ''
  const date = new Date(epochSeconds * 1000)
  const now = new Date()
  if (date.toDateString() === now.toDateString()) return formatMessageTime(epochSeconds)
  const options: Intl.DateTimeFormatOptions =
    date.getFullYear() === now.getFullYear()
      ? { month: 'short', day: 'numeric' }
      : { month: 'short', day: 'numeric', year: 'numeric' }
  return date.toLocaleDateString([], options)
}

export function normalizeUrl(urlStr: string): string {
  if (/^https?:\/\//i.test(urlStr)) return urlStr
  if (/^[\w.-]+\.[a-z]{2,}(\/|$)/i.test(urlStr)) return `https://${urlStr}`
  return urlStr
}

export function getShortenedLinkText(urlStr: string): string {
  try {
    const url = new URL(normalizeUrl(urlStr))
    let display = url.hostname
    if (display.startsWith('www.')) {
      display = display.substring(4)
    }
    if (url.pathname && url.pathname !== '/') {
      let path = url.pathname
      if (path.length > 24) {
        path = path.substring(0, 24) + '…'
      }
      display += path
    }
    return display
  } catch {
    if (urlStr.length > 30) {
      return urlStr.substring(0, 30) + '…'
    }
    return urlStr
  }
}

export function normalizeBodyText(text: string) {
  return (
    text
      .replace(/\n{3,}/g, '\n\n')
      // The body renderer handles links and bold but not markdown lists, so turn
      // leading list markers (`- `, `* `, `+ `) into bullet glyphs for display.
      .replace(/^[ \t]*[-*+] +/gm, '• ')
      .trim()
  )
}

export function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

export type MessageInlinePart = { type: 'text' | 'link'; content: string; label?: string }
export type MessageContentBlock = { type: 'inline'; parts: MessageInlinePart[] } | { type: 'code'; content: string }

export function splitFencedCodeBlocks(text: string): MessageContentBlock[] {
  const blocks: MessageContentBlock[] = []
  const textBuffer: string[] = []
  const codeBuffer: string[] = []
  let inCode = false

  const flushText = () => {
    const content = textBuffer.join('\n').replace(/\n+$/g, '')
    textBuffer.length = 0
    if (content.trim()) {
      blocks.push({ type: 'inline', parts: parseInlineMessageContent(content) })
    }
  }

  for (const line of text.split('\n')) {
    if (line.trimStart().startsWith('```')) {
      if (inCode) {
        blocks.push({ type: 'code', content: codeBuffer.join('\n').replace(/\n+$/g, '') })
        codeBuffer.length = 0
        inCode = false
      } else {
        flushText()
        inCode = true
      }
      continue
    }

    if (inCode) {
      codeBuffer.push(line)
    } else {
      textBuffer.push(line)
    }
  }

  if (inCode) {
    textBuffer.push('```', ...codeBuffer)
  }
  flushText()
  return blocks
}

export function parseInlineMessageContent(text: string): MessageInlinePart[] {
  if (!text) return []

  const tokenRegex = /(\[[^\]]+\]\([^)]+\)|(?:https?:\/\/|www\.)[^\s<>"']+)/g
  const parts = text.split(tokenRegex)

  const elements: MessageInlinePart[] = []
  for (const part of parts) {
    const markdownLink = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/)
    if (markdownLink) {
      elements.push({ type: 'link', content: normalizeUrl(markdownLink[2]), label: markdownLink[1] })
    } else if (/^(https?:\/\/|www\.)/i.test(part)) {
      elements.push({ type: 'link', content: normalizeUrl(part) })
    } else if (part) {
      elements.push({ type: 'text', content: part })
    }
  }

  return elements
}

/** Extract the bare email address from a single "Name <addr>" entry,
 * or return the input when no angle-bracket form is present. */
export function extractAddr(entry: string): string {
  const match = entry.match(/<([^>]+)>/)
  return (match ? match[1] : entry).trim()
}

export function messageSearchText(message: Message): string {
  return [message.subject, message.from_name, message.from_addr, message.body].join('\n').toLowerCase()
}

export interface AddressItem {
  name: string
  email: string
  original: string
}

export function parseAddressList(raw: string | undefined | null): AddressItem[] {
  if (!raw) return []
  const regex = /(?:"?([^"]*)"?\s*<([^>]+)>)|([^\s,]+@[^\s,]+)/g
  const results: AddressItem[] = []
  let match
  while ((match = regex.exec(raw)) !== null) {
    const displayName = match[1]?.trim()
    const bracketEmail = match[2]?.trim()
    const bareEmail = match[3]?.trim()

    if (bracketEmail) {
      results.push({
        name: displayName || bracketEmail,
        email: bracketEmail,
        original: match[0].trim(),
      })
    } else if (bareEmail) {
      results.push({
        name: bareEmail,
        email: bareEmail,
        original: bareEmail,
      })
    }
  }

  if (results.length === 0 && raw.trim()) {
    return raw.split(',').map((part) => {
      part = part.trim()
      const emailMatch = part.match(/<([^>]+)>/)
      if (emailMatch) {
        const email = emailMatch[1].trim()
        const name = part
          .replace(/<[^>]+>/, '')
          .trim()
          .replace(/^["']|["']$/g, '')
          .trim()
        return { name: name || email, email, original: part }
      }
      return { name: part, email: part, original: part }
    })
  }
  return results
}
