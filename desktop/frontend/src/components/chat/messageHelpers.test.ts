import { afterEach, beforeEach, describe, expect, it, setSystemTime } from 'bun:test'
import {
  bodyContentKey,
  extractAddr,
  formatFileSize,
  formatMessageStamp,
  formatRecipientSummary,
  getShortenedLinkText,
  getVisibleMedia,
  htmlHasRemoteMedia,
  htmlReferencesMedia,
  isImage,
  isVideo,
  mediaSrc,
  messageSearchText,
  normalizeBodyText,
  normalizeUrl,
  readerAttachmentImages,
  parseAddressList,
  parseInlineMessageContent,
  standaloneAttachmentImages,
  splitFencedCodeBlocks,
} from './messageHelpers'

const NOW = new Date(2026, 5, 10, 15, 30, 0)
const sec = (d: Date) => Math.floor(d.getTime() / 1000)

beforeEach(() => {
  setSystemTime(NOW)
})

afterEach(() => {
  setSystemTime()
})

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

  it('detects media already present in html bodies', () => {
    expect(
      htmlReferencesMedia('<img src="/media/account/image.png">', {
        key: 'account/image.png',
        url: null,
      } as any),
    ).toBe(true)
    expect(
      htmlReferencesMedia('<img src="https://example.com/image.jpg?width=600&amp;height=400">', {
        key: null,
        url: 'https://example.com/image.jpg?width=600&height=400',
      } as any),
    ).toBe(true)
    expect(
      htmlReferencesMedia('<img src="https://example.com/other.jpg">', {
        key: null,
        url: 'https://example.com/image.jpg',
      } as any),
    ).toBe(false)
  })

  it('keeps only HTML message image attachments that are not embedded in the body', () => {
    const embedded = { key: 'account/embedded.jpg', mime: 'image/jpeg' } as any
    const attached = { key: 'account/attached.jpg', mime: 'image/jpeg' } as any
    const html = '<p>Message body</p><img src="/media/account/embedded.jpg">'

    expect(standaloneAttachmentImages([embedded], true, html)).toEqual([])
    expect(standaloneAttachmentImages([attached], true, html)).toEqual([attached])
    expect(standaloneAttachmentImages([embedded, attached], true, html)).toEqual([attached])
    expect(standaloneAttachmentImages([embedded], false, html)).toEqual([embedded])
  })

  it('hides remote media until account settings or reveal allow them', () => {
    const message = {
      from_addr: 'news@example.com',
      attachments: [
        { key: 'local.png', mime: 'image/png' },
        { url: 'data:image/png;base64,abc', mime: 'image/png' },
        { url: 'https://example.com/remote.png', mime: 'image/png' },
        { key: 'local.mp4', mime: 'video/mp4' },
        { url: 'https://example.com/remote.mp4', mime: 'video/mp4' },
        { filename: 'doc.pdf', mime: 'application/pdf' },
      ],
    } as any

    const hidden = getVisibleMedia(message, { load_remote_images: false } as any, false, [])
    expect(hidden.attachmentImages).toHaveLength(2)
    expect(hidden.videos).toHaveLength(1)
    expect(hidden.hiddenRemoteCount).toBe(2)
    expect(hidden.files).toHaveLength(1)

    const revealed = getVisibleMedia(message, { load_remote_images: false } as any, true, [])
    expect(revealed.attachmentImages).toHaveLength(3)
    expect(revealed.videos).toHaveLength(2)
    expect(revealed.hiddenRemoteCount).toBe(0)

    const accountAllowed = getVisibleMedia(message, { load_remote_images: true } as any, false, [])
    expect(accountAllowed.attachmentImages).toHaveLength(3)
    expect(accountAllowed.videos).toHaveLength(2)

    // An allowed sender lifts the block for that sender only.
    const senderAllowed = getVisibleMedia(message, { load_remote_images: false } as any, false, ['news@example.com'])
    expect(senderAllowed.attachmentImages).toHaveLength(3)
    expect(senderAllowed.hiddenRemoteCount).toBe(0)
    const otherSender = getVisibleMedia(message, { load_remote_images: false } as any, false, ['someone@else.test'])
    expect(otherSender.hiddenRemoteCount).toBe(2)
  })

  it('spots remote references in an HTML body', () => {
    expect(htmlHasRemoteMedia('<img src="https://tracker.example/pixel.gif">')).toBe(true)
    expect(htmlHasRemoteMedia('<td background="//cdn.example/bg.png">')).toBe(true)
    expect(htmlHasRemoteMedia('<div style="background:url(http://cdn.example/bg.png)">')).toBe(true)
    // A poster is fetched under `img-src`, so it is blocked and needs a reveal.
    expect(htmlHasRemoteMedia('<video poster="https://cdn.example/poster.jpg" src="/media/abc"></video>')).toBe(true)
    expect(htmlHasRemoteMedia('<img src="/media/abc123"><a href="https://example.com">link</a>')).toBe(false)
    expect(htmlHasRemoteMedia(undefined)).toBe(false)
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

  it('normalizes body text', () => {
    expect(normalizeBodyText('\n- one\n* two\n+ three\n\n\n')).toBe('• one\n• two\n• three')
  })

  it('keys body content by more than its length', () => {
    expect(bodyContentKey('abc')).toBe(bodyContentKey('abc'))
    expect(bodyContentKey('abc')).not.toBe(bodyContentKey('abd'))
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
      {
        type: 'inline',
        parts: [
          { type: 'text', content: 'before ' },
          { type: 'link', content: 'https://example.com' },
        ],
      },
      { type: 'code', content: 'const x = 1' },
      { type: 'inline', parts: [{ type: 'text', content: 'after' }] },
    ])
    expect(splitFencedCodeBlocks('```\nunclosed')).toEqual([
      { type: 'inline', parts: [{ type: 'text', content: '```\nunclosed' }] },
    ])
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
    expect(parseAddressList('Alice <alice@example.com>, Bob Jones <bob@example.com>')).toEqual([
      { name: 'Alice', email: 'alice@example.com', original: 'Alice <alice@example.com>' },
      { name: 'Bob Jones', email: 'bob@example.com', original: 'Bob Jones <bob@example.com>' },
    ])
    expect(parseAddressList('"Doe, Jane" <jane@example.com>, john@example.com')).toEqual([
      { name: 'Doe, Jane', email: 'jane@example.com', original: '"Doe, Jane" <jane@example.com>' },
      { name: 'john@example.com', email: 'john@example.com', original: 'john@example.com' },
    ])
    expect(parseAddressList("O'Brien <obrien@example.com>, Pat <pat@example.com>")).toHaveLength(2)
    expect(
      messageSearchText({ subject: 'Hello', from_name: 'Ada', from_addr: 'ada@example.com', body: 'World' } as any),
    ).toBe('hello\nada\nada@example.com\nworld')
  })
})

describe('messageHelpers timestamp helpers', () => {
  it('formats bubble stamps with Gmail-style dates', () => {
    expect(formatMessageStamp(0, false)).toBe('')
    expect(formatMessageStamp(sec(new Date(2026, 5, 10, 9, 5)), false)).toBe('09:05')
    expect(formatMessageStamp(sec(new Date(2026, 5, 9, 9, 0)), false)).toMatch(/Jun 9/)
    expect(formatMessageStamp(sec(new Date(2025, 11, 31, 9, 0)), false)).toMatch(/2025/)
  })
})

describe('messageHelpers recipient summary', () => {
  it('summarizes To and Cc the way an outgoing bubble header shows them', () => {
    // The reply: named recipients, To plus Cc, in order.
    expect(
      formatRecipientSummary('"nonbili/meron" <reply+abc@reply.github.com>', '"Comment" <comment@noreply.github.com>'),
    ).toBe('nonbili/meron, Comment')
    // The forward: empty display name falls back to the address local part.
    expect(formatRecipientSummary('"" <ping.eminel@gmail.com>', undefined)).toBe('ping.eminel')
    expect(formatRecipientSummary('ada@example.com, "Ada" <ada@example.com>')).toBe('ada')
    expect(formatRecipientSummary('Alice <alice@example.com>, Bob Jones <bob@example.com>')).toBe('Alice, Bob Jones')
    expect(formatRecipientSummary('', null)).toBe('')
  })
})

describe('readerAttachmentImages', () => {
  const attachments = [
    { key: 'inline.png', mime: 'image/png', filename: 'inline.png' },
    { url: 'data:image/png;base64,abc', mime: 'image/png', filename: 'pasted.png' },
    { url: 'https://tracking.example/pixel.png', mime: 'image/png', filename: 'pixel.png' },
    { key: 'doc.pdf', mime: 'application/pdf', filename: 'doc.pdf' },
  ] as any[]

  it('holds remote images back while remote content is blocked', () => {
    // They render outside the iframe, so the body's CSP does not cover them.
    expect(readerAttachmentImages(attachments, undefined, false).map((a) => a.filename)).toEqual([
      'inline.png',
      'pasted.png',
    ])
  })

  it('shows them once remote content is allowed', () => {
    expect(readerAttachmentImages(attachments, undefined, true).map((a) => a.filename)).toEqual([
      'inline.png',
      'pasted.png',
      'pixel.png',
    ])
  })

  it('skips images the body already renders', () => {
    const html = '<img src="/media/inline.png">'
    expect(readerAttachmentImages(attachments, html, true).map((a) => a.filename)).toEqual(['pasted.png', 'pixel.png'])
  })
})
