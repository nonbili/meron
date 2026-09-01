import { describe, expect, it } from 'bun:test'
import { applyBubbleTheme, prepareBubbleHtml } from './bubbleHtml'
import { DEFAULT_BUBBLE_THEME, bubbleThemeFromTokens, frameVar, frameVarPrefix, type BubbleTheme } from './frameTheme'
import { builtinTheme } from '../../lib/themes'

const DARK_TOKENS = builtinTheme('dark')!.tokens
const DARK_IN = bubbleThemeFromTokens('dark', DARK_TOKENS, false)
const DARK_OUT = bubbleThemeFromTokens('dark', DARK_TOKENS, true)

const frameStyle = (prepared: string) =>
  [...new DOMParser().parseFromString(prepared, 'text/html').querySelectorAll('head style')]
    .map((style) => style.textContent ?? '')
    .join('\n')

// The theme is applied to the rendered frame, not baked into its HTML: the
// decision needs to know what the message's own CSS resolves to.
const themedFrame = (html: string, theme: BubbleTheme) => {
  const doc = new DOMParser().parseFromString(prepareBubbleHtml(html), 'text/html')
  applyBubbleTheme(doc, theme)
  // The variable names are per document and unguessable; the frame's own
  // stylesheet is where the name they share travels.
  const prefix = frameVarPrefix(doc)
  return {
    getPropertyValue: (name: string) => doc.documentElement.style.getPropertyValue(frameVar(prefix, name)),
    // A canvas the message declared is restored as its own inline declaration;
    // one the frame chose is painted from the frame's stylesheet.
    canvas: () =>
      doc.body.style.getPropertyValue('background-color') ||
      doc.documentElement.style.getPropertyValue(frameVar(prefix, 'body-bg')),
  }
}

// A newsletter's own reset is `html, body { height: 100% !important }`, so the
// override only wins as an inline declaration — those outrank every stylesheet
// rule of the same importance, wherever the sender's `<style>` happens to sit.
const sizing = (prepared: string) => {
  const doc = new DOMParser().parseFromString(prepared, 'text/html')
  return [doc.documentElement, doc.body].map((el) => el.getAttribute('style') ?? '')
}

describe('prepareBubbleHtml', () => {
  it('lets newsletter documents grow beyond the placeholder frame', () => {
    const html = `
      <html>
        <head>
          <style>html, body { height: 100% !important; }</style>
        </head>
        <body><p>Visible message</p></body>
      </html>
    `

    const prepared = prepareBubbleHtml(html)

    for (const style of sizing(prepared)) {
      expect(style).toContain('height: auto !important')
      expect(style).toContain('min-height: 0 !important')
    }
    expect(prepared).toContain('Visible message')
  })

  it('outranks a reset that the sender put inside the body', () => {
    // ESP templates commonly emit their reset/media-query block after <body>
    // starts; the parser leaves it there, so a head-only override would lose.
    const html = `
      <html>
        <body>
          <style>html, body { height: 100% !important; }</style>
          <p>Visible message</p>
        </body>
      </html>
    `

    const prepared = prepareBubbleHtml(html)

    for (const style of sizing(prepared)) {
      expect(style).toContain('height: auto !important')
    }
    expect(prepared).toContain('Visible message')
  })

  it('only loosens the baked remote-content policy when asked', () => {
    const csp = "default-src 'none'; script-src 'none'; img-src 'self' data:;"
    const html = `<html><head><meta http-equiv="Content-Security-Policy" content="${csp}"></head><body><img src="https://cdn.example/a.png"></body></html>`
    const policyOf = (prepared: string) =>
      [...new DOMParser().parseFromString(prepared, 'text/html').querySelectorAll('meta[http-equiv]')]
        .map((meta) => meta.getAttribute('content') ?? '')
        .join(' | ')

    expect(policyOf(prepareBubbleHtml(html))).toContain("img-src 'self' data:;")
    const revealed = policyOf(prepareBubbleHtml(html, undefined, true))
    expect(revealed).toContain("img-src 'self' data: http: https:")
    // The email's own scripts stay blocked either way.
    expect(revealed).toContain("script-src 'none'")
  })

  it('tightens a policy baked while the sender was allowed', () => {
    // What core bakes for a message read while its sender was on the allowlist.
    const csp = "default-src 'none'; script-src 'none'; img-src 'self' data: http: https:;"
    const html = `<html><head><meta http-equiv="Content-Security-Policy" content="${csp}"></head><body><img src="https://cdn.example/a.png"></body></html>`

    // Taking the allowance back re-blocks it without waiting for a re-read: no
    // meta may leave a remote source open, since all of them are enforced.
    const policies = [
      ...new DOMParser().parseFromString(prepareBubbleHtml(html), 'text/html').querySelectorAll('meta[http-equiv]'),
    ].map((meta) => meta.getAttribute('content') ?? '')

    expect(policies.length).toBeGreaterThan(1)
    for (const policy of policies) {
      expect(policy).toContain("img-src 'self' data:;")
      expect(policy).not.toContain('http:')
    }
  })

  it('leaves the body structure untouched', () => {
    const html = '<html><body><p>First</p><table><tr><td>Last</td></tr></table></body></html>'

    const doc = new DOMParser().parseFromString(prepareBubbleHtml(html), 'text/html')

    expect(doc.body.lastElementChild?.tagName).toBe('TABLE')
    expect(doc.querySelector('body > table:last-child')).not.toBeNull()
  })

  describe('theming', () => {
    it('paints a message with no colors of its own in the bubble it sits in', () => {
      const html = '<html><body><p>Just words</p></body></html>'

      const inbound = themedFrame(html, DARK_IN)
      expect(inbound.getPropertyValue('text')).toBe(DARK_TOKENS.bubbleInText)
      expect(inbound.canvas()).toBe('')

      // An outgoing bubble carries a different background, so a different text color.
      expect(themedFrame(html, DARK_OUT).getPropertyValue('text')).toBe(DARK_TOKENS.bubbleOutText)
    })

    it('gives a self-styled message a light card inside a dark bubble', () => {
      const html =
        '<html><head><meta name="meron-body-bg" content="#f5f4f2"></head><body><p style="color:#333">Hi</p></body></html>'

      const style = themedFrame(html, DARK_IN)

      // Its own page color, and the light palette its text was authored against.
      // The frame doesn't paint it: it is restored as the message's own inline
      // declaration, so the sender's stylesheets still outrank it where they did.
      expect(style.canvas()).toBe('#f5f4f2')
      expect(style.getPropertyValue('body-bg')).toBe('')
      expect(style.getPropertyValue('text')).toBe(DEFAULT_BUBBLE_THEME.text)
    })

    it('falls back to white for a message that styles dark text but declares no page color', () => {
      const html = '<html><body><p style="color:#333">Hi</p></body></html>'

      // This canvas is the frame's own choice, so it reads as a card, inset
      // from the bubble's edges.
      const style = themedFrame(html, DARK_IN)
      expect(style.canvas()).toBe('#ffffff')
      expect(style.getPropertyValue('canvas-pad')).toBe('10px')
    })

    it('leaves a message written for a dark canvas on the dark bubble', () => {
      // Light text with no background of its own was authored for a dark client:
      // a white card under it would be the light-on-light mirror image.
      const html = '<html><body><p style="color:white">Hi</p></body></html>'

      const style = themedFrame(html, DARK_IN)
      expect(style.canvas()).toBe('')
      expect(style.getPropertyValue('text')).toBe(DARK_TOKENS.bubbleInText)
    })

    it('restores a declared dark canvas with the text color that goes on it', () => {
      const html =
        '<html><head><meta name="meron-body-bg" content="#000000"><meta name="meron-body-fg" content="white"></head><body><p>Hi</p></body></html>'

      // Even in a light theme: the message's light text needs its dark canvas.
      const style = themedFrame(html, DEFAULT_BUBBLE_THEME)
      expect(style.canvas()).toBe('#000000')
      expect(style.getPropertyValue('text')).toBe('white')
    })

    it('leaves a light appearance exactly as it was', () => {
      const html = '<html><body><p style="color:#333">Hi</p></body></html>'

      const style = themedFrame(html, DEFAULT_BUBBLE_THEME)

      expect(style.canvas()).toBe('')
      expect(style.getPropertyValue('text')).toBe(DEFAULT_BUBBLE_THEME.text)
    })

    it('never declares a background of its own in the stylesheet', () => {
      // A stylesheet declaration is in the cascade whether the frame wants one
      // or not — `var(--unset, transparent)` resolves to transparent, and an
      // unresolved `var()` computes to `initial`, which is the same — so it
      // would wipe a background the message paints for itself.
      const style = frameStyle(prepareBubbleHtml('<html><body><p>Hi</p></body></html>'))
      const body = style.slice(style.indexOf('body {'), style.indexOf('*, *::before'))

      expect(body).not.toContain('background')
    })

    it('leaves a background the message paints for itself alone', () => {
      // Its own rule, its own canvas: the frame chooses none and paints none.
      const html =
        '<html><head><style>body { background: #000 } p { color: #fff }</style></head><body><p>Hi</p></body></html>'

      const style = themedFrame(html, DEFAULT_BUBBLE_THEME)
      expect(style.canvas()).toBe('')
    })

    it('stamps the generation the host asked for', () => {
      // The host wires a frame as soon as its srcDoc changes, while the document
      // it replaces is still loaded; this is how it tells them apart.
      const prepared = prepareBubbleHtml('<html><body><p>Hi</p></body></html>', undefined, false, 'gen-1')
      const doc = new DOMParser().parseFromString(prepared, 'text/html')

      expect(doc.documentElement.getAttribute('data-meron-generation')).toBe('gen-1')
    })

    it('keeps the light values as the stylesheet fallbacks', () => {
      // An unthemed frame renders exactly as it did before it was themeable.
      const style = frameStyle(prepareBubbleHtml('<html><body><p>Hi</p></body></html>'))

      expect(style).toMatch(new RegExp(`var\\(--meron-[a-z0-9]+-text, ${DEFAULT_BUBBLE_THEME.text}\\)`))
      expect(style).toMatch(new RegExp(`var\\(--meron-[a-z0-9]+-link, ${DEFAULT_BUBBLE_THEME.link}\\)`))
    })
  })
})
