import { describe, expect, it } from 'bun:test'
import {
  escapeRegExp,
  extractAddr,
  formatFileSize,
  getShortenedLinkText,
  getVisibleMedia,
  isImage,
  isVideo,
  mediaSrc,
  messageSearchText,
  normalizeBodyText,
  normalizeUrl,
  parseAddressList,
  parseInlineMessageContent,
  splitFencedCodeBlocks,
} from './messageHelpers'

describe('messageHelpers file and media helpers', () => {
  it('formats byte sizes across units', () => {
    expect(formatFileSize(0)).toBe('0 KB')
    expect(formatFileSize(Number.NaN)).toBe('0 KB')
    expect(formatFileSize(12)).toBe('12 B')
    expect(formatFileSize(1536)).toBe('2 KB')
    expect(formatFileSize(3.25 * 1024 * 1024)).toBe('3.3 MB')
    expect(formatFileSize(2 * 1024 * 1024 * 1024)).toBe('2 GB')
  })

  it('classifies and resolves media sources', () => {
    expect(isImage({ mime: 'IMAGE/PNG' } as any)).toBe(true)
    expect(isVideo({ mime: 'video/mp4' } as any)).toBe(true)
    expect(isImage({ mime: 'application/pdf' } as any)).toBe(false)
    expect(mediaSrc({ key: 'acct/file.png' } as any)).toBe('/media/acct/file.png')
    expect(mediaSrc({ url: 'https://example.com/file.png' } as any)).toBe('https://example.com/file.png')
  })

  it('hides remote media until account settings or reveal allow them', () => {
    const message = {
      attachments: [
        { key: 'local.png', mime: 'image/png' },
        { url: 'data:image/png;base64,abc', mime: 'image/png' },
        { url: 'https://example.com/remote.png', mime: 'image/png' },
        { key: 'local.mp4', mime: 'video/mp4' },
        { url: 'https://example.com/remote.mp4', mime: 'video/mp4' },
        { filename: 'doc.pdf', mime: 'application/pdf' },
      ],
    } as any

    const hidden = getVisibleMedia(message, { load_remote_images: false } as any, false)
    expect(hidden.attachmentImages).toHaveLength(2)
    expect(hidden.videos).toHaveLength(1)
    expect(hidden.hiddenRemoteCount).toBe(2)
    expect(hidden.files).toHaveLength(1)

    const revealed = getVisibleMedia(message, { load_remote_images: false } as any, true)
    expect(revealed.attachmentImages).toHaveLength(3)
    expect(revealed.videos).toHaveLength(2)
    expect(revealed.hiddenRemoteCount).toBe(0)

    const accountAllowed = getVisibleMedia(message, { load_remote_images: true } as any, false)
    expect(accountAllowed.attachmentImages).toHaveLength(3)
    expect(accountAllowed.videos).toHaveLength(2)
  })
})

describe('messageHelpers text and link helpers', () => {
  it('normalizes and shortens URLs for display', () => {
    expect(normalizeUrl('example.com/path')).toBe('https://example.com/path')
    expect(normalizeUrl('www.example.com')).toBe('https://www.example.com')
    expect(normalizeUrl('mailto:me@example.com')).toBe('mailto:me@example.com')
    expect(getShortenedLinkText('https://www.example.com/a/short/path')).toBe('example.com/a/short/path')
    expect(getShortenedLinkText('https://example.com/this/path/is/definitely/longer/than/twenty-four')).toBe(
      'example.com/this/path/is/definitely…',
    )
    expect(getShortenedLinkText('not a url that is long enough to shorten')).toBe('not a url that is long enough …')
  })

  it('normalizes body text and escapes regular expression syntax', () => {
    expect(normalizeBodyText('\n- one\n* two\n+ three\n\n\n')).toBe('• one\n• two\n• three')
    expect(escapeRegExp('[a+b].*')).toBe('\\[a\\+b\\]\\.\\*')
  })

  it('parses inline links and markdown links', () => {
    expect(parseInlineMessageContent('See [docs](example.com/docs) and https://example.com/raw')).toEqual([
      { type: 'text', content: 'See ' },
      { type: 'link', content: 'https://example.com/docs', label: 'docs' },
      { type: 'text', content: ' and ' },
      { type: 'link', content: 'https://example.com/raw' },
    ])
    expect(parseInlineMessageContent('')).toEqual([])
  })

  it('splits fenced code blocks while keeping inline text parsed', () => {
    expect(splitFencedCodeBlocks('before https://example.com\n```\nconst x = 1\n```\nafter')).toEqual([
      { type: 'inline', parts: [{ type: 'text', content: 'before ' }, { type: 'link', content: 'https://example.com' }] },
      { type: 'code', content: 'const x = 1' },
      { type: 'inline', parts: [{ type: 'text', content: 'after' }] },
    ])
    expect(splitFencedCodeBlocks('```\nunclosed')).toEqual([{ type: 'inline', parts: [{ type: 'text', content: '```\nunclosed' }] }])
  })

  it('extracts address data and searchable message text', () => {
    expect(extractAddr('Ada Lovelace <ada@example.com>')).toBe('ada@example.com')
    expect(extractAddr('plain@example.com')).toBe('plain@example.com')
    expect(parseAddressList('"Ada Lovelace" <ada@example.com>, bob@example.com')).toEqual([
      { name: 'Ada Lovelace', email: 'ada@example.com', original: '"Ada Lovelace" <ada@example.com>' },
      { name: 'bob@example.com', email: 'bob@example.com', original: 'bob@example.com' },
    ])
    expect(parseAddressList('Display Name <display@example.com>')).toEqual([
      { name: 'Display Name', email: 'display@example.com', original: 'Display Name <display@example.com>' },
    ])
    expect(messageSearchText({ subject: 'Hello', from_name: 'Ada', from_addr: 'ada@example.com', body: 'World' } as any)).toBe(
      'hello\nada\nada@example.com\nworld',
    )
  })
})
