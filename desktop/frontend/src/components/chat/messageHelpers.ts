import { splitAddressList } from '../../lib/address'
import {
  Calendar,
  Contact,
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileKey,
  FileSpreadsheet,
  FileText,
  FileType,
  FileVideo,
  Mail,
  Package,
  Presentation,
} from 'lucide-react'
import type { ComponentType } from 'react'
import type { Account, Attachment, Message } from '../../types'
import { normalizeSenderAddr } from '../../states/settings'
import { PdfIcon, type IconProps } from '../icons/Icons'
import { splitQuotedBody } from './quoteFold'

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

/** Extensions sharing one icon, flattened into `EXT_ICONS` below. */
const EXT_GROUPS: [FileIconComponent, string[]][] = [
  [PdfIcon, ['pdf']],
  [FileText, ['doc', 'docx', 'rtf', 'odt', 'pages', 'txt', 'md', 'log']],
  [FileSpreadsheet, ['xls', 'xlsx', 'xlsm', 'csv', 'tsv', 'ods', 'numbers']],
  [Presentation, ['ppt', 'pptx', 'odp', 'key']],
  [FileArchive, ['zip', 'rar', '7z', 'tar', 'gz', 'tgz', 'bz2', 'xz', 'zst']],
  [FileImage, ['jpg', 'jpeg', 'png', 'gif', 'webp', 'heic', 'heif', 'bmp', 'tif', 'tiff', 'svg', 'avif']],
  [FileVideo, ['mp4', 'm4v', 'mov', 'mkv', 'webm', 'avi', 'wmv']],
  [FileAudio, ['mp3', 'm4a', 'wav', 'flac', 'ogg', 'oga', 'opus', 'aac']],
  // Meeting invites and contact cards, the two structured files mail carries most.
  [Calendar, ['ics', 'ical', 'ifb', 'vcs']],
  [Contact, ['vcf', 'vcard']],
  // A forwarded message attached whole.
  [Mail, ['eml', 'msg']],
  // Signatures and keys: S/MIME's smime.p7s rides on every signed message.
  [FileKey, ['p7s', 'p7m', 'p7c', 'asc', 'sig', 'gpg', 'pgp', 'pem', 'crt', 'cer']],
  [Package, ['apk', 'dmg', 'exe', 'msi', 'deb', 'rpm', 'pkg', 'appimage', 'iso']],
  [FileType, ['ttf', 'otf', 'woff', 'woff2']],
  // prettier-ignore
  [FileCode, [
    'js', 'ts', 'jsx', 'tsx', 'json', 'html', 'htm', 'css', 'py', 'rs', 'go', 'java', 'kt', 'swift',
    'c', 'h', 'cpp', 'rb', 'php', 'sh', 'sql', 'xml', 'yml', 'yaml', 'toml',
  ]],
]

const EXT_ICONS: Record<string, FileIconComponent> = Object.fromEntries(
  EXT_GROUPS.flatMap(([icon, exts]) => exts.map((ext) => [ext, icon])),
)

/** Pick a file icon from extension first, then mime, falling back to a generic file. */
export function fileIconFor(filename: string, mime: string): FileIconComponent {
  const ext = filename.includes('.') ? (filename.split('.').pop()?.toLowerCase() ?? '') : ''
  if (EXT_ICONS[ext]) return EXT_ICONS[ext]

  // Specific types before the `text/` catch-all: invites and contact cards are
  // text/calendar and text/vcard.
  const m = (mime ?? '').toLowerCase().split(';')[0].trim()
  if (m === 'text/calendar' || m === 'application/ics') return Calendar
  if (m === 'text/vcard' || m === 'text/x-vcard' || m === 'text/directory') return Contact
  if (m === 'message/rfc822' || m === 'application/vnd.ms-outlook') return Mail
  if (m.includes('pkcs7') || m.includes('pgp') || m.includes('x509')) return FileKey
  if (m.startsWith('image/')) return FileImage
  if (m.startsWith('video/')) return FileVideo
  if (m.startsWith('audio/')) return FileAudio
  if (m.startsWith('font/')) return FileType
  if (m.startsWith('text/')) return FileText
  if (m === 'application/pdf') return PdfIcon
  if (m.includes('spreadsheet') || m.includes('excel')) return FileSpreadsheet
  if (m.includes('presentation') || m.includes('powerpoint')) return Presentation
  if (m.includes('wordprocessing') || m.includes('msword') || m.includes('opendocument.text')) return FileText
  if (m.includes('zip') || m.includes('compressed') || m.includes('tar')) return FileArchive
  if (m.includes('android.package') || m.includes('x-msdownload') || m.includes('diskimage')) return Package
  if (m.includes('json') || m.includes('javascript') || m.includes('xml')) return FileCode
  return File
}

export function mediaSrc(media: Attachment): string {
  return media.key ? `/media/${media.key}` : media.url!
}

/** The media key behind an image src served from the media cache: `/media/<key>`,
 * or that path made absolute, as a frame document's `img.src` reports it. */
export function mediaKeyFromSrc(src: string): string | null {
  const prefix = '/media/'
  if (src.startsWith(prefix)) return decodeURIComponent(src.slice(prefix.length))
  try {
    const url = new URL(src)
    if (url.origin !== window.location.origin || !url.pathname.startsWith(prefix)) return null
    return decodeURIComponent(url.pathname.slice(prefix.length))
  } catch {
    return null
  }
}

/** The name to save cached media under: the attachment's own when the message
 * lists it, else the key's file name (an inline picture's alt text is a caption,
 * not a name). */
export function mediaFilename(key: string, attachments: Attachment[] | undefined): string {
  return attachments?.find((attachment) => attachment.key === key)?.filename ?? key.split('/').pop() ?? key
}

/** Sources the native save/copy flow can handle. Do not fetch until selected. */
export function imageSource(src: string): { key: string | null; url?: string; filename: string } | null {
  const key = mediaKeyFromSrc(src)
  if (key) return { key, filename: mediaFilename(key, undefined) }
  try {
    const url = new URL(src)
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return null
    let filename = url.pathname.split('/').pop() || 'image'
    try {
      filename = decodeURIComponent(filename)
    } catch {
      // Some image servers use legacy or literal percent escapes.
    }
    filename = filename.replace(/[\\/:\x00-\x1f]/g, '_')
    return { key: null, url: url.href, filename }
  } catch {
    return null
  }
}

export function htmlReferencesMedia(html: string | undefined, media: Attachment): boolean {
  if (!html) return false
  if (media.key && html.includes(`/media/${media.key}`)) return true
  if (!media.url) return false

  const escapedUrl = escapeHtmlAttribute(media.url)
  return html.includes(media.url) || html.includes(escapedUrl)
}

/** Images that need a separate grid because the active body does not already render them. */
export function standaloneAttachmentImages(
  attachmentImages: Attachment[],
  useHtmlBody: boolean,
  html: string | undefined,
): Attachment[] {
  return useHtmlBody ? attachmentImages.filter((image) => !htmlReferencesMedia(html, image)) : attachmentImages
}

function escapeHtmlAttribute(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/'/g, '&#39;').replace(/</g, '&lt;')
}

export function isVideo(a: Attachment): boolean {
  return (a.mime ?? '').toLowerCase().startsWith('video/')
}

/** Whether an attachment renders without a remote fetch: a cached `/media/<key>`
 *  file, or a `data:` URL (used by optimistic sent-message previews). Anything
 *  else is fetched from the network, so the remote-content policy gates it. */
export function isInlineMedia(a: Attachment): boolean {
  return !!a.key || !!a.url?.startsWith('data:')
}

export function isImage(a: Attachment): boolean {
  return (a.mime ?? '').toLowerCase().startsWith('image/')
}

// Remote references the sidecar's CSP blocks when the account (or sender) is not
// trusted: an absolute or protocol-relative `src`/`srcset`/`background`/`poster`
// (a video poster is fetched under `img-src`, so it is blocked like any image),
// and CSS `url(...)` references. Inline images are `/media/<key>` or `data:`, so
// they never match.
const REMOTE_MEDIA_REF =
  /<[^>]+\s(?:src|srcset|background|poster)\s*=\s*["']?\s*(?:https?:)?\/\/|url\(\s*["']?\s*(?:https?:)?\/\//i

/** Whether an HTML body references remote media that the CSP is holding back. */
export function htmlHasRemoteMedia(html: string | undefined): boolean {
  return !!html && REMOTE_MEDIA_REF.test(html)
}

/** Whether a message's remote content may load without the user revealing it:
 *  the account-wide setting, or the sender being on the app-wide allowlist
 *  (`settings$.remoteImageSenders`, passed in so callers stay reactive). */
export function remoteContentAllowed(
  message: Message,
  account: Account | undefined,
  allowedSenders: string[],
): boolean {
  // Decrypted mail never loads remote content (EFAIL; the core's CSP blocks
  // it too, whatever this says).
  if (message.crypto?.encrypted) return false
  if (account?.load_remote_images ?? false) return true
  const sender = normalizeSenderAddr(message.from_addr)
  return !!sender && allowedSenders.includes(sender)
}

/** The images a reader view shows beside the body: those the HTML does not
 *  already reference, minus the remote ones while remote content is blocked —
 *  they render outside the iframe, so the body's CSP does not cover them. */
export function readerAttachmentImages(
  attachments: Attachment[] | undefined,
  html: string | undefined,
  allowRemote: boolean,
): Attachment[] {
  return (attachments ?? []).filter((attachment) => {
    if (!isImage(attachment) || (!attachment.key && !attachment.url)) return false
    if (!allowRemote && !isInlineMedia(attachment)) return false
    return !htmlReferencesMedia(html, attachment)
  })
}

/** Videos omitted from the HTML body still need a player in the reader. */
export function readerAttachmentVideos(
  attachments: Attachment[] | undefined,
  html: string | undefined,
  allowRemote: boolean,
): Attachment[] {
  return (attachments ?? []).filter((attachment) => {
    if (!isVideo(attachment) || (!attachment.key && !attachment.url)) return false
    if (!allowRemote && !isInlineMedia(attachment)) return false
    return !htmlReferencesMedia(html, attachment)
  })
}

export function getVisibleMedia(
  message: Message,
  account: Account | undefined,
  revealed: boolean,
  allowedSenders: string[],
) {
  const attachments = message.attachments ?? []
  const localImages = attachments.filter((a) => isInlineMedia(a) && isImage(a))
  const remoteImages = attachments.filter((a) => !isInlineMedia(a) && a.url && isImage(a))
  const localVideos = attachments.filter((a) => isInlineMedia(a) && isVideo(a))
  const remoteVideos = attachments.filter((a) => !isInlineMedia(a) && a.url && isVideo(a))
  const files = attachments.filter((a) => !isImage(a) && !isVideo(a))
  const remoteVisible =
    remoteContentAllowed(message, account, allowedSenders) || (revealed && !message.crypto?.encrypted)
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

/** Cheap content key for work cached per message body: two bodies of the same
 *  length that differ still get different keys, which a length alone would not. */
export function bodyContentKey(body: string): string {
  let hash = 0
  for (let index = 0; index < body.length; index += 1) {
    hash = (Math.imul(hash, 31) + body.charCodeAt(index)) | 0
  }
  return `${body.length}:${hash}`
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

/** The blocks a normalized plain body renders as: fenced code split out, the
 *  rest parsed into inline text and links. */
export function messageContentBlocks(bodyText: string): MessageContentBlock[] {
  const hasCodeFence = bodyText.split('\n').some((line) => line.trimStart().startsWith('```'))
  return hasCodeFence
    ? splitFencedCodeBlocks(bodyText)
    : [{ type: 'inline', parts: parseInlineMessageContent(bodyText) }]
}

export type InlineMarkupChunk = { type: 'plain' | 'bold' | 'italic' | 'code'; text: string }

/** Split a run of body text into its inline markup chunks, with the markers
 *  stripped: the renderer wraps each in its element, and the search counter
 *  reads the same texts, so the two agree on what is highlightable. */
export function splitInlineMarkup(content: string): InlineMarkupChunk[] {
  return content.split(/(`[^`\n]+`|\*\*[^*]+\*\*|\*[^*\n]+\*)/g).map((chunk): InlineMarkupChunk => {
    const bold = chunk.match(/^\*\*([^*]+)\*\*$/)
    if (bold) return { type: 'bold', text: bold[1] }
    const italic = chunk.match(/^\*([^*\n]+)\*$/)
    if (italic) return { type: 'italic', text: italic[1] }
    const inlineCode = chunk.match(/^`([^`\n]+)`$/)
    if (inlineCode) return { type: 'code', text: inlineCode[1] }
    return { type: 'plain', text: chunk }
  })
}

/**
 * Every string the plain-text body renderer can highlight, in render order.
 * Fenced code blocks and shortened link URLs are rendered as-is and never
 * marked, so they are left out — counting them would make the search bar
 * promise matches no <mark> ever lands on.
 */
export function plainHighlightTexts(body: string, quoteStart?: number | null): string[] {
  // The reply and its quoted tail are normalized and rendered as two sections
  // (MessageBubbleBody), so they are walked the same way here.
  const { reply, quote } = splitQuotedBody(body, quoteStart)
  return quote ? [...sectionHighlightTexts(reply), ...sectionHighlightTexts(quote)] : sectionHighlightTexts(reply)
}

function sectionHighlightTexts(body: string): string[] {
  const blocks = messageContentBlocks(normalizeBodyText(body))

  const texts: string[] = []
  for (const block of blocks) {
    if (block.type === 'code') continue
    for (const part of block.parts) {
      const content = part.type === 'link' ? part.label : part.content
      if (!content) continue
      for (const chunk of splitInlineMarkup(content)) texts.push(chunk.text)
    }
  }
  return texts
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

/** Gmail-style recipient summary for a bubble header ("to nonbili/meron, Comment").
 * Display name when the address carries one, otherwise the local part. To and Cc
 * are merged and de-duplicated: an outgoing reply and an outgoing forward can
 * carry the same subject and the same text, so who received it is what tells
 * them apart. */
export function formatRecipientSummary(...lists: (string | undefined | null)[]): string {
  const seen = new Set<string>()
  const names: string[] = []
  for (const list of lists) {
    for (const item of parseAddressList(list)) {
      const key = item.email.trim().toLowerCase()
      if (!key || seen.has(key)) continue
      seen.add(key)
      names.push(item.name === item.email ? item.email.split('@')[0] : item.name)
    }
  }
  return names.join(', ')
}

export interface AddressItem {
  name: string
  email: string
  original: string
}

export function parseAddressList(raw: string | undefined | null): AddressItem[] {
  if (!raw) return []
  const results: AddressItem[] = []
  const entries = splitAddressList(raw, true)
  for (const entry of entries) {
    const bracketMatch = entry.match(/^(.*?)\s*<([^>]+)>$/)
    const bracketEmail = bracketMatch?.[2]?.trim()
    const bareEmail = bracketEmail ? undefined : entry.match(/[^\s,]+@[^\s,]+/)?.[0]?.trim()

    if (bracketEmail) {
      const rawName = bracketMatch?.[1]?.trim() ?? ''
      const displayName =
        (rawName.startsWith('"') && rawName.endsWith('"')) || (rawName.startsWith("'") && rawName.endsWith("'"))
          ? rawName.slice(1, -1).trim()
          : rawName
      results.push({
        name: displayName || bracketEmail,
        email: bracketEmail,
        original: entry,
      })
    } else if (bareEmail) {
      results.push({
        name: bareEmail,
        email: bareEmail,
        original: bareEmail,
      })
    }
  }
  return results.length > 0 ? results : entries.map((entry) => ({ name: entry, email: entry, original: entry }))
}
