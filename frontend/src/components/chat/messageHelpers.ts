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
import { normalizeSenderAddr } from '../../states/settings'
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
  const remoteVisible = remoteContentAllowed(message, account, allowedSenders) || revealed
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
export function plainHighlightTexts(body: string): string[] {
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

function splitAddressEntries(raw: string): string[] {
  const entries: string[] = []
  let quoted = false
  let angleDepth = 0
  let start = 0

  for (let index = 0; index < raw.length; index += 1) {
    const char = raw[index]
    if (char === '"' && raw[index - 1] !== '\\') {
      quoted = !quoted
    } else if (quoted) {
      continue
    } else if (char === '<') {
      angleDepth += 1
    } else if (char === '>' && angleDepth > 0) {
      angleDepth -= 1
    } else if (char === ',' && angleDepth === 0) {
      const entry = raw.slice(start, index).trim()
      if (entry) entries.push(entry)
      start = index + 1
    }
  }

  const entry = raw.slice(start).trim()
  if (entry) entries.push(entry)
  return entries
}

export function parseAddressList(raw: string | undefined | null): AddressItem[] {
  if (!raw) return []
  const results: AddressItem[] = []
  const entries = splitAddressEntries(raw)
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
