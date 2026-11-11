import { describe, expect, it } from 'bun:test'
import { inlineRichStyles, withoutInlineRichStyles } from './composerHelpers'
import { inertHtmlToText, htmlToText } from '../../lib/html'
import { reopenedSignatureTracking } from '../../lib/signature'

describe('withoutInlineRichStyles', () => {
  const html =
    '<p>Hi <code>x</code></p><blockquote><p>q</p></blockquote><p></p><p><a href="https://example.com">Me</a></p>'

  it('takes off what a save put on, however often it was saved', () => {
    expect(withoutInlineRichStyles(inlineRichStyles(html))).toBe(html)
    expect(withoutInlineRichStyles(inlineRichStyles(inlineRichStyles(html)))).toBe(html)
  })

  it('keeps styles of any other origin', () => {
    const own = '<p><a href="https://example.com" style="font-weight: bold;">Me</a></p>'
    expect(withoutInlineRichStyles(own)).toBe(own)
    expect(withoutInlineRichStyles(inlineRichStyles(own))).toBe(own)
  })

  it('lets a reopened draft be recognised by a signature holding a link', () => {
    const signature = { html: '<p><a href="https://example.com">Me</a></p>', text: 'Me' }
    const saved = inlineRichStyles(`<p>Hi</p><p></p>${signature.html}`)
    expect(reopenedSignatureTracking({ rich: true, html: saved, text: '' }, signature)).toBeUndefined()
    expect(
      reopenedSignatureTracking({ rich: true, html: withoutInlineRichStyles(saved), text: '' }, signature),
    ).toEqual({ ...signature, placement: 'belowText' })
  })
})

describe('inertHtmlToText', () => {
  it('reads as htmlToText does, an image by its alt text', () => {
    const html = '<p>Me</p><p>Phone</p><p><img src="https://example.com/logo.png" alt="logo"></p>'
    expect(inertHtmlToText(html)).toBe(htmlToText(html))
  })
})
