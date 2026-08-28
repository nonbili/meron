import { describe, expect, it } from 'bun:test'
import { parseMailto } from './mailto'

describe('parseMailto', () => {
  it('parses a bare recipient', () => {
    expect(parseMailto('mailto:a@x.com')).toEqual({
      to: 'a@x.com',
      cc: '',
      bcc: '',
      subject: '',
      body: '',
    })
  })

  it('parses query params and decodes percent-encoding', () => {
    const fields = parseMailto('mailto:a@x.com?subject=Hello%20World&body=line1%0Aline2&cc=c@x.com&bcc=d@x.com')
    expect(fields).toEqual({
      to: 'a@x.com',
      cc: 'c@x.com',
      bcc: 'd@x.com',
      subject: 'Hello World',
      body: 'line1\nline2',
    })
  })

  it('merges path and ?to= recipients', () => {
    const fields = parseMailto('mailto:a@x.com,b@x.com?to=c@x.com,d@x.com')
    expect(fields?.to).toBe('a@x.com, b@x.com, c@x.com, d@x.com')
  })

  it('decodes percent-encoded path recipients', () => {
    expect(parseMailto('mailto:a%40x.com')?.to).toBe('a@x.com')
  })

  it('returns null for non-mailto URLs and garbage', () => {
    expect(parseMailto('https://example.com')).toBeNull()
    expect(parseMailto('not a url')).toBeNull()
    expect(parseMailto('')).toBeNull()
  })

  it('accepts uppercase scheme', () => {
    expect(parseMailto('MAILTO:a@x.com')?.to).toBe('a@x.com')
  })
})
