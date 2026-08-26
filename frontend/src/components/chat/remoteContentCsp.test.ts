import { describe, expect, it } from 'bun:test'
import {
  allowRemoteContent,
  allowRemoteInCsp,
  applyRemoteContentPolicy,
  blockRemoteContent,
  blockRemoteInCsp,
} from './remoteContentCsp'

// The policy the sidecar bakes into a blocked message (see `prepare_html`).
const BLOCKED_CSP =
  "default-src 'none'; script-src 'none'; img-src 'self' data:; media-src 'self' data: blob:; style-src 'unsafe-inline';"

// What it bakes for a message read while its sender was on the allowlist.
const ALLOWED_CSP =
  "default-src 'none'; script-src 'none'; img-src 'self' data: http: https:; media-src 'self' data: blob: http: https:; style-src 'unsafe-inline';"

describe('allowRemoteInCsp', () => {
  it('opens img-src and media-src to remote hosts', () => {
    const relaxed = allowRemoteInCsp(BLOCKED_CSP)

    expect(relaxed).toContain("img-src 'self' data: http: https:")
    expect(relaxed).toContain("media-src 'self' data: blob: http: https:")
  })

  it('leaves every other directive alone', () => {
    const relaxed = allowRemoteInCsp(BLOCKED_CSP)

    expect(relaxed).toContain("script-src 'none'")
    expect(relaxed).toContain("default-src 'none'")
  })

  it('does not repeat sources it already has', () => {
    const relaxed = allowRemoteInCsp("img-src 'self' data: http: https:;")

    expect(relaxed).toBe("img-src 'self' data: http: https:;")
  })
})

describe('allowRemoteContent', () => {
  it('rewrites the baked meta in place', () => {
    const doc = new DOMParser().parseFromString(
      `<html><head><meta http-equiv="Content-Security-Policy" content="${BLOCKED_CSP}"></head><body></body></html>`,
      'text/html',
    )

    allowRemoteContent(doc)

    const content = doc.querySelector('meta[http-equiv]')?.getAttribute('content') ?? ''
    expect(content).toContain('img-src')
    expect(content).toContain('https:')
    expect(content).toContain("script-src 'none'")
  })
})

describe('blockRemoteInCsp', () => {
  it('takes remote hosts back out of img-src and media-src', () => {
    const tightened = blockRemoteInCsp(ALLOWED_CSP)

    expect(tightened).toContain("img-src 'self' data:;")
    expect(tightened).toContain("media-src 'self' data: blob:;")
    expect(tightened).not.toContain('http:')
    expect(tightened).not.toContain('https:')
  })

  it('drops a wildcard source too', () => {
    expect(blockRemoteInCsp('img-src * data: blob:;')).toBe('img-src data: blob:;')
  })

  it('leaves an already blocked policy and its other directives alone', () => {
    const tightened = blockRemoteInCsp(BLOCKED_CSP)

    expect(tightened).toContain("img-src 'self' data:;")
    expect(tightened).toContain("script-src 'none'")
    expect(tightened).toContain("default-src 'none'")
  })

  it('round-trips with allowRemoteInCsp', () => {
    expect(blockRemoteInCsp(allowRemoteInCsp(BLOCKED_CSP)).replace(/\s+/g, ' ')).toBe(BLOCKED_CSP.replace(/\s+/g, ' '))
  })
})

describe('blockRemoteContent', () => {
  it('tightens a meta baked while the sender was allowed', () => {
    const doc = new DOMParser().parseFromString(
      `<html><head><meta http-equiv="Content-Security-Policy" content="${ALLOWED_CSP}"></head><body></body></html>`,
      'text/html',
    )

    blockRemoteContent(doc)

    const content = doc.querySelector('meta[http-equiv]')?.getAttribute('content') ?? ''
    expect(content).not.toContain('https:')
    expect(content).toContain("img-src 'self' data:")
    expect(content).toContain("script-src 'none'")
  })
})

describe('applyRemoteContentPolicy', () => {
  const bodyWith = (csp: string) =>
    `<html><head><meta http-equiv="Content-Security-Policy" content="${csp}"></head><body><img src="https://cdn.example/a.png"></body></html>`

  it('opens a blocked body once the message is revealed', () => {
    const revealed = applyRemoteContentPolicy(bodyWith(BLOCKED_CSP), true)

    expect(revealed).toContain("img-src 'self' data: http: https:")
    expect(revealed).toContain('cdn.example/a.png')
  })

  it('re-blocks a body baked while the sender was allowed', () => {
    const blocked = applyRemoteContentPolicy(bodyWith(ALLOWED_CSP), false)

    expect(blocked).toContain("img-src 'self' data:;")
    expect(blocked).not.toContain('http:')
    expect(blocked).toContain("script-src 'none'")
  })
})
