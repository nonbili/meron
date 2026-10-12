import { describe, expect, it } from 'bun:test'
import {
  applyBubbleTheme,
  prepareBubbleHtml,
  reloadFailedImages,
  reserveImageBox,
  releaseFailedImageBox,
  restoreImageBox,
  applyBubbleThemeAsync,
} from './bubbleHtml'
import {
  CAPPED_ATTR,
  DARKENED_ATTR,
  DEFAULT_BUBBLE_THEME,
  FRAME_STYLE_MARKER,
  PICTURE_ATTR,
  bubbleThemeFromTokens,
  darkenedFloor,
  frameVar,
  frameVarPrefix,
  type BubbleTheme,
} from './frameTheme'
import { formatColor, mix, parseColor, toHex } from '../../lib/color'
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

    it('ignores a stylesheet that arrived claiming to be the frame stylesheet', () => {
      const html =
        '<html><head><style data-meron-frame-style="stolen">p{color:red}</style></head><body><p>Hi</p></body></html>'
      const doc = new DOMParser().parseFromString(prepareBubbleHtml(html), 'text/html')
      const marked = [...doc.querySelectorAll(`style[${FRAME_STYLE_MARKER}]`)]

      expect(marked).toHaveLength(1)
      expect(marked[0]?.getAttribute(FRAME_STYLE_MARKER)).not.toBe('stolen')
      expect(marked[0]?.textContent).toContain('overflow-wrap')
    })

    it('holds the place of a picture that declares its size', () => {
      const doc = new DOMParser().parseFromString(
        '<img width="600" height="400" src="a.png"><img width="100%" height="40" src="b.png"><img width="600" height="400" style="aspect-ratio: 1 / 1" src="c.png">',
        'text/html',
      )
      const [sized, relative, styled] = [...doc.querySelectorAll('img')]

      expect(reserveImageBox(sized!)).toBe(true)
      expect(sized?.style.aspectRatio).toBe('auto 600 / 400')
      // A percentage is not a length the box can be built from.
      expect(reserveImageBox(relative!)).toBe(false)
      expect(relative?.style.aspectRatio).toBe('')
      // The sender's own box is the sender's.
      expect(reserveImageBox(styled!)).toBe(false)
      expect(styled?.style.aspectRatio).toBe('1 / 1')
    })

    it('asks again only for pictures that failed', () => {
      const doc = new DOMParser().parseFromString(
        '<img id="failed" src="/media/a/1/0.png"><img id="loaded" src="/media/a/1/1.png"><img id="loading" src="/media/a/1/2.png">',
        'text/html',
      )
      const state = { failed: [true, 0], loaded: [true, 40], loading: [false, 0] } as const
      const requested: string[] = []
      for (const image of doc.querySelectorAll('img')) {
        const [complete, naturalWidth] = state[image.id as keyof typeof state]
        Object.defineProperty(image, 'complete', { value: complete })
        Object.defineProperty(image, 'naturalWidth', { value: naturalWidth })
        const setAttribute = image.setAttribute.bind(image)
        image.setAttribute = (name: string, value: string) => {
          if (name === 'src') requested.push(image.id)
          setAttribute(name, value)
        }
      }

      reloadFailedImages(doc)

      expect(requested).toEqual(['failed'])
      expect(doc.getElementById('failed')?.getAttribute('src')).toBe('/media/a/1/0.png')
    })

    it('retries a pending picture once if its error arrives after recovery', () => {
      const doc = new DOMParser().parseFromString('<img src="/media/a/1/0.png">', 'text/html')
      const image = doc.querySelector('img')!
      Object.defineProperty(image, 'complete', { value: false })
      Object.defineProperty(image, 'naturalWidth', { value: 0 })
      let requests = 0
      const setAttribute = image.setAttribute.bind(image)
      image.setAttribute = (name, value) => {
        if (name === 'src') requests++
        setAttribute(name, value)
      }
      reloadFailedImages(doc)
      reloadFailedImages(doc)
      expect(requests).toBe(0)
      image.dispatchEvent(new Event('error'))
      expect(requests).toBe(1)
      image.dispatchEvent(new Event('error'))
      expect(requests).toBe(1)
      reloadFailedImages(doc)
      image.dispatchEvent(new Event('load'))
      image.dispatchEvent(new Event('error'))
      expect(requests).toBe(1)
    })

    // What the core hands over: its shell, its baked CSP, then the message.
    const coreDocument = (message: string) =>
      `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src 'self' data:; media-src 'self' data: blob:;"></head><body>${message}</body></html>`

    it('keeps its CSP first in the head of a document from the core', () => {
      // `<style>` text and attribute values reach the frame verbatim, so a
      // message can spell out tags the frame must not mistake for its own.
      for (const message of [
        '<style>/*<html><head>*/</style><img src="https://t.example/p.png">',
        '<img alt="</head><body><html><head>" src="https://t.example/p.png">',
      ]) {
        const out = prepareBubbleHtml(coreDocument(message), undefined, false, 'gen-1')

        expect(out.startsWith('<html data-meron-generation="gen-1" style="')).toBe(true)
        const head = out.slice(out.indexOf('<head>') + '<head>'.length)
        expect(head.startsWith('<meta http-equiv="Content-Security-Policy" content="default-src \'none\';')).toBe(true)
        expect(head.indexOf("img-src 'self' data:;")).toBeLessThan(head.indexOf('</head>'))
        expect(out).toContain(message)
        expect(out.match(new RegExp(FRAME_STYLE_MARKER, 'g'))).toHaveLength(1)
      }
    })

    it('brings the baked CSP of a document from the core in line with the reveal', () => {
      const out = prepareBubbleHtml(coreDocument('<p>Hi</p>'), undefined, true)

      expect(out).toContain("img-src 'self' data: http: https:;")
      expect(out).toContain('img-src * data: blob:')
    })

    it('strips a frame stylesheet claim from a document from the core however it is quoted', () => {
      for (const claim of ['="stolen"', "='stolen'", '=stolen', '']) {
        const out = prepareBubbleHtml(coreDocument(`<style data-meron-frame-style${claim}>p{color:red}</style>`))

        expect(out).toContain('<style>p{color:red}</style>')
      }
    })

    it('disowns sender style tags however they are written, without changing message text', () => {
      const claims = [
        `<style/data-meron-frame-style="x">p { color: red; }</style>`,
        `<style DATA-MERON-FRAME-STYLE="x">p { color: red; }</style>`,
        // An empty comment, then a real style tag.
        `<!--><style data-meron-frame-style="x">p { color: red; }</style>`,
        // A stray `<` and a quote ahead of the tag.
        `a < "b" <style data-meron-frame-style="x">p { color: red; }</style>`,
      ]
      for (const claim of claims) {
        const out = prepareBubbleHtml(coreDocument(`<p>data-meron-frame-style="x"</p>${claim}`))
        const doc = new DOMParser().parseFromString(out, 'text/html')
        // Only the frame's own stylesheet carries the marker.
        const marked = [...doc.querySelectorAll('[data-meron-frame-style]')]
        expect(marked).toHaveLength(1)
        expect(marked[0].getAttribute('data-meron-frame-style')).not.toBe('x')
        expect(doc.querySelector('p')?.textContent).toBe('data-meron-frame-style="x"')
      }
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

describe('dark message bodies in a bubble', () => {
  const darkened = (html: string, theme: BubbleTheme) => {
    const doc = new DOMParser().parseFromString(prepareBubbleHtml(html), 'text/html')
    applyBubbleTheme(doc, theme)
    return doc.documentElement.hasAttribute(DARKENED_ATTR)
  }
  const DARKENING = bubbleThemeFromTokens('dark', DARK_TOKENS, false, true)
  const styled = '<p style="color:#333">hi</p>'

  it('inverts the light card a self-styled message is given, only when asked', () => {
    expect(darkened(styled, DARKENING)).toBe(true)
    expect(darkened(styled, DARK_IN)).toBe(false)
  })

  it('leaves unstyled mail in the bubble palette and light themes untouched', () => {
    expect(darkened('<p>hi</p>', DARKENING)).toBe(false)
    expect(darkened(styled, DEFAULT_BUBBLE_THEME)).toBe(false)
  })

  it('turns back only background pictures that carry no text', () => {
    const doc = new DOMParser().parseFromString(
      prepareBubbleHtml(
        '<p style="color:#333">hi</p>' +
          '<div id="none" style="background-image:none">plain text</div>' +
          '<table><tr><td id="texted" style="background-image:url(https://example.com/hero.png)">over a photo</td></tr></table>' +
          '<div id="picture" style="background-image:url(https://example.com/banner.png)"></div>',
      ),
      'text/html',
    )
    applyBubbleTheme(doc, DARKENING)
    const picture = (id: string) => doc.getElementById(id)?.hasAttribute(PICTURE_ATTR)

    expect(picture('none')).toBe(false)
    expect(picture('texted')).toBe(false)
    expect(picture('picture')).toBe(true)

    applyBubbleTheme(doc, DARK_IN)
    expect(picture('picture')).toBe(false)
  })

  // A positioned box paints above the layer that caps the body, so its white
  // would invert to black; its background is capped on the element instead.
  it('caps a background the layer over the body cannot reach', () => {
    const doc = new DOMParser().parseFromString(
      prepareBubbleHtml(
        '<p style="color:#333">hi</p>' +
          '<div id="panel" style="position:relative;background-color:#ffffff !important">panel</div>' +
          '<table><tr><td id="cell" style="background:#ffffff">cell</td></tr></table>' +
          '<div id="dark" style="position:relative;background-color:#102030">dark</div>' +
          '<div id="clear" style="position:relative">clear</div>' +
          '<div id="picture" style="background-color:#ffffff;background-image:url(https://example.com/banner.png)"></div>',
      ),
      'text/html',
    )
    const background = (id: string) => doc.getElementById(id)!.style.getPropertyValue('background-color')
    const priority = (id: string) => doc.getElementById(id)!.style.getPropertyPriority('background-color')
    const floor = darkenedFloor(DARK_TOKENS.bubbleIn)

    applyBubbleTheme(doc, DARKENING)
    expect(toHex(background('panel'))).toBe(floor)
    expect(toHex(background('cell'))).toBe(floor)
    // Already under the floor, nothing to paint, and a picture turned back to its own colors.
    expect(toHex(background('dark'))).toBe('#102030')
    expect(background('clear')).toBe('')
    expect(toHex(background('picture'))).toBe('#ffffff')

    // A second pass caps from what the message declared, not from the last cap.
    applyBubbleTheme(doc, bubbleThemeFromTokens('dark', DARK_TOKENS, true, true))
    expect(toHex(background('panel'))).toBe(darkenedFloor(DARK_TOKENS.bubbleOut))

    applyBubbleTheme(doc, DARK_IN)
    expect(toHex(background('panel'))).toBe('#ffffff')
    expect(priority('panel')).toBe('important')
    expect(toHex(background('cell'))).toBe('#ffffff')
    expect(priority('cell')).toBe('')
    expect(doc.querySelector(`[${CAPPED_ATTR}]`)).toBeNull()
  })

  // A frame on the default light scheme under a dark app gets an opaque white
  // backdrop from the engine, which put unstyled mail's light text on white.
  it("follows the theme's color scheme so the frame stays transparent", () => {
    const scheme = (theme: BubbleTheme) => {
      const doc = new DOMParser().parseFromString(prepareBubbleHtml('<p>hi</p>'), 'text/html')
      applyBubbleTheme(doc, theme)
      return doc.documentElement.style.getPropertyValue('color-scheme')
    }
    expect(scheme(DARKENING)).toBe('dark')
    expect(scheme(DEFAULT_BUBBLE_THEME)).toBe('light')
  })

  it('carries the darkening rules in the frame stylesheet', () => {
    expect(frameStyle(prepareBubbleHtml(styled))).toContain(`html[${DARKENED_ATTR}] body`)
  })

  // Under a fully transparent root the body's background propagates to the
  // canvas, outside the body's invert filter, so a white body stayed white.
  // The root now paints the surface the darkened body comes out as.
  it('paints a darkened root with the surface the frame sits on', () => {
    const css = frameStyle(prepareBubbleHtml(styled))
    const rule = css.match(new RegExp(`html\\[${DARKENED_ATTR}\\]\\s*\\{([^}]*)\\}`))
    expect(rule?.[1]).toMatch(/background:\s*var\(--meron-[a-z0-9]+-darkened-backdrop,[^)]*\)+\s*!important/)

    const doc = new DOMParser().parseFromString(prepareBubbleHtml(styled), 'text/html')
    applyBubbleTheme(doc, DARKENING)
    const backdrop = doc.documentElement.style.getPropertyValue(frameVar(frameVarPrefix(doc), 'darkened-backdrop'))
    expect(backdrop).toBe(DARK_TOKENS.bubbleIn)
  })

  // Inverted white turns black, so a layer over the body caps it first at the
  // color that inverts to the surface. Pictures paint above the layer, uncapped.
  it('caps the darkened body at the color that inverts to the surface', () => {
    const css = frameStyle(prepareBubbleHtml(styled))
    const rule = css.match(new RegExp(`html\\[${DARKENED_ATTR}\\] body::before\\s*\\{([^}]*)\\}`))
    expect(rule?.[1]).toMatch(/background:\s*var\(--meron-[a-z0-9]+-darkened-floor, #ffffff\) !important/)
    expect(rule?.[1]).toMatch(/mix-blend-mode:\s*darken !important/)
    // Blending the body itself would cap the pictures in it as well.
    expect(css).not.toMatch(new RegExp(`html\\[${DARKENED_ATTR}\\] body\\s*\\{[^}]*mix-blend-mode`))

    const doc = new DOMParser().parseFromString(prepareBubbleHtml(styled), 'text/html')
    applyBubbleTheme(doc, DARKENING)
    const floor = doc.documentElement.style.getPropertyValue(frameVar(frameVarPrefix(doc), 'darkened-floor'))
    expect(floor).toBe(darkenedFloor(DARK_TOKENS.bubbleIn))
  })

  // The frame's own white canvas is a card inset from the bubble's edges; once
  // it has blended into the bubble, the inset would only indent the text.
  it('drops the card inset from a darkened canvas', () => {
    const inset = (theme: BubbleTheme) => {
      const doc = new DOMParser().parseFromString(prepareBubbleHtml(styled), 'text/html')
      applyBubbleTheme(doc, theme)
      return doc.documentElement.style.getPropertyValue(frameVar(frameVarPrefix(doc), 'canvas-pad'))
    }
    expect(inset(DARK_IN)).toBe('10px')
    expect(inset(DARKENING)).toBe('')
  })
})

describe('the color a darkened body is capped at', () => {
  // The darkening filter, run forwards: invert, then hue-rotate(180deg).
  const darkenedTo = (color: string) => {
    const { r, g, b } = parseColor(color)!
    const [ir, ig, ib] = [255 - r, 255 - g, 255 - b]
    return formatColor({
      r: -0.574 * ir + 1.43 * ig + 0.144 * ib,
      g: 0.426 * ir + 0.43 * ig + 0.144 * ib,
      b: 0.426 * ir + 1.43 * ig - 0.856 * ib,
      a: 1,
    })
  }
  const near = (a: string, b: string, tolerance: number) => {
    const [x, y] = [parseColor(a)!, parseColor(b)!]
    return Math.max(Math.abs(x.r - y.r), Math.abs(x.g - y.g), Math.abs(x.b - y.b)) <= tolerance
  }

  it('darkens back to the surface', () => {
    for (const surface of [DARK_TOKENS.bubbleIn, DARK_TOKENS.bubbleOut, DARK_TOKENS.bgChats]) {
      expect(near(darkenedTo(darkenedFloor(surface)), surface, 2)).toBe(true)
    }
    expect(darkenedFloor('#000000')).toBe('#ffffff')
  })

  // A surface this saturated is outside what the filter can produce.
  it('gets as near as the filter reaches for a saturated surface', () => {
    const indigo = builtinTheme('indigo-dark')!.tokens.bubbleOut
    const reached = darkenedTo(darkenedFloor(indigo))
    expect(near(reached, indigo, 2)).toBe(false)
    expect(near(reached, indigo, 32)).toBe(true)
  })

  it('caps nothing for a color it cannot read', () => {
    expect(darkenedFloor('nonsense')).toBe('#ffffff')
  })
})

describe('the surface a darkened body blends onto', () => {
  it('is the bubble colour in the chat layout, per direction', () => {
    expect(bubbleThemeFromTokens('dark', DARK_TOKENS, false, true).darkenedBackdrop).toBe(DARK_TOKENS.bubbleIn)
    expect(bubbleThemeFromTokens('dark', DARK_TOKENS, true, true).darkenedBackdrop).toBe(DARK_TOKENS.bubbleOut)
  })

  it('is the row colour in the traditional layout, whichever the direction', () => {
    expect(bubbleThemeFromTokens('dark', DARK_TOKENS, false, true, 'row').darkenedBackdrop).toBe(DARK_TOKENS.bgChats)
    expect(bubbleThemeFromTokens('dark', DARK_TOKENS, true, true, 'row').darkenedBackdrop).toBe(DARK_TOKENS.bgChats)
  })

  // A draft's bubble is the outgoing colour at part strength over the conversation.
  it('is the see-through outgoing colour over the conversation for a draft', () => {
    const backdrop = bubbleThemeFromTokens('dark', DARK_TOKENS, true, true, 'draft').darkenedBackdrop
    expect(backdrop).toBe(mix(DARK_TOKENS.bgChat, DARK_TOKENS.bubbleOut, 0.55))
    expect(backdrop).not.toBe(DARK_TOKENS.bubbleOut)
  })
})

describe('media sizing and asynchronous theming', () => {
  it('releases browser-derived dimensions and sender minimum sizes on failed images', () => {
    const doc = new DOMParser().parseFromString(
      '<img width="600" height="400" style="aspect-ratio:auto 600 / 400; min-height:400px; width:600px" src="missing.png">',
      'text/html',
    )
    const image = doc.querySelector('img')!
    releaseFailedImageBox(image)
    expect(image.hasAttribute('width')).toBe(false)
    expect(image.hasAttribute('height')).toBe(false)
    expect(image.style.aspectRatio).toBe('auto')
    expect(image.style.width).toBe('auto')
    expect(image.style.minHeight).toBe('0')
    expect(image.style.getPropertyPriority('aspect-ratio')).toBe('important')
  })

  it('reserves dimensioned videos too', () => {
    const doc = new DOMParser().parseFromString('<video width="600" height="400"></video>', 'text/html')
    const video = doc.querySelector('video')!
    expect(reserveImageBox(video)).toBe(true)
    expect(video.style.aspectRatio).toBe('auto 600 / 400')
  })

  it('finishes the same theme asynchronously and allows a cancelled walk to stop', async () => {
    const html = prepareBubbleHtml('<p style="color:black">Newsletter</p>')
    const sync = new DOMParser().parseFromString(html, 'text/html')
    const asyncDoc = new DOMParser().parseFromString(html, 'text/html')
    const theme = { ...DEFAULT_BUBBLE_THEME, appearance: 'dark' as const, darkenStyled: true }
    applyBubbleTheme(sync, theme)
    await applyBubbleThemeAsync(asyncDoc, theme, () => false)
    expect(asyncDoc.documentElement.outerHTML).toBe(sync.documentElement.outerHTML)
    const cancelled = new DOMParser().parseFromString(html, 'text/html')
    await applyBubbleThemeAsync(cancelled, theme, () => true)
    expect(cancelled.documentElement.hasAttribute(DARKENED_ATTR)).toBe(false)
    expect(cancelled.documentElement.style.colorScheme).toBe('')
  })
})

it('restores authored dimensions and priorities after a failed image recovers', () => {
  const doc = new DOMParser().parseFromString(
    '<img width="80" height="40" style="width:80px!important;min-height:40px;aspect-ratio:2 / 1"><img style="color:red">',
    'text/html',
  )
  const [sized, unsized] = [...doc.querySelectorAll('img')]
  const dimensions = (image: HTMLImageElement) => [
    image.getAttribute('width'),
    image.getAttribute('height'),
    ...['width', 'height', 'min-width', 'min-height', 'aspect-ratio'].map((name) => [
      image.style.getPropertyValue(name),
      image.style.getPropertyPriority(name),
    ]),
  ]
  for (const image of [sized!, unsized!]) {
    const before = dimensions(image)
    releaseFailedImageBox(image)
    releaseFailedImageBox(image)
    image.style.color = 'blue'
    restoreImageBox(image)
    expect(dimensions(image)).toEqual(before)
    expect(image.style.color).toBe('blue')
    restoreImageBox(image)
    expect(dimensions(image)).toEqual(before)
  }
})
