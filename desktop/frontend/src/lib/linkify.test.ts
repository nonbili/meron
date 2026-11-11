import { describe, expect, it } from 'bun:test'
import { findBareUrls, linkifyLine, linkifyTextNodes } from './linkify'

describe('findBareUrls', () => {
  it('finds http(s) and www URLs, trimming trailing punctuation', () => {
    expect(findBareUrls('see https://x.example/a. and www.example.com/b?c=1, ok').map((u) => u.href)).toEqual([
      'https://x.example/a',
      'http://www.example.com/b?c=1',
    ])
    expect(findBareUrls('(https://en.wikipedia.org/wiki/Foo_(bar))')[0].text).toBe(
      'https://en.wikipedia.org/wiki/Foo_(bar)',
    )
  })

  it('trims a long run of unmatched brackets in linear time', () => {
    const started = performance.now()
    const [url] = findBareUrls(`https://x.example/${')'.repeat(200_000)}`)
    expect(performance.now() - started).toBeLessThan(1000)
    expect(url.text).toBe('https://x.example/')
  })

  it('keeps a trailing underscore, which tokens end in', () => {
    expect(findBareUrls('https://x.example/?token=abc_ ok')[0].text).toBe('https://x.example/?token=abc_')
  })

  it('ignores non-URLs', () => {
    expect(findBareUrls('https:// www.nodot me@www.example.com xhttps://x.example example.com')).toEqual([])
  })
})

describe('linkifyLine', () => {
  it('escapes the text around a link', () => {
    expect(linkifyLine('<a> https://x.example/?a=1&b=2')).toBe(
      '&lt;a&gt; <a href="https://x.example/?a=1&amp;b=2">https://x.example/?a=1&amp;b=2</a>',
    )
  })
})

describe('linkifyTextNodes', () => {
  const linkified = (html: string) => {
    const doc = new DOMParser().parseFromString(html, 'text/html')
    linkifyTextNodes(doc.body)
    return doc.body.innerHTML
  }

  it('links a URL ending a paragraph, before an image', () => {
    expect(linkified('<p>same as https://x.example/tag/android%2Fv0.5.0</p><p><img src="cid:a"></p>')).toBe(
      '<p>same as <a href="https://x.example/tag/android%2Fv0.5.0">https://x.example/tag/android%2Fv0.5.0</a></p><p><img src="cid:a"></p>',
    )
  })

  it('leaves links and code alone', () => {
    const html =
      '<p><a href="https://a.example/">https://a.example/</a> <code>https://b.example</code></p><pre>https://c.example</pre>'
    expect(linkified(html)).toBe(html)
  })
})
