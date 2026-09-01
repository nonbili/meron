import { describe, expect, it } from 'bun:test'
import { applyReaderLayout, applyReaderTheme } from './readerHtml'
import {
  DEFAULT_READER_THEME,
  frameVar,
  frameVarPrefix,
  colorTone,
  declaredBodyBackground,
  messageIsSelfStyled,
  readerThemeFromTokens,
  type ReaderTheme,
} from './frameTheme'
import { builtinTheme } from '../../lib/themes'

const DARK: ReaderTheme = readerThemeFromTokens('dark', builtinTheme('dark')!.tokens)

const parse = (html: string) => new DOMParser().parseFromString(html, 'text/html')
// The frame's variables are named per document and unguessable, so sender CSS
// can't address them; tests ask the document what its own names are.
// What the message actually sits on: a canvas the message declared is restored
// as its own inline declaration, one the frame chose is painted from the frame's
// stylesheet.
const canvasOf = (doc: Document) =>
  doc.body.style.getPropertyValue('background-color') ||
  doc.documentElement.style.getPropertyValue(frameVar(frameVarPrefix(doc), 'body-bg'))

const vars = (doc: Document) => ({
  getPropertyValue: (name: string) => doc.documentElement.style.getPropertyValue(frameVar(frameVarPrefix(doc), name)),
})

describe('declaredBodyBackground', () => {
  it('reads the color prepare_html hoisted off the stripped <body>', () => {
    const doc = parse('<meta name="meron-body-bg" content="#f5f4f2"><p>hi</p>')
    expect(declaredBodyBackground(doc)).toBe('#f5f4f2')
  })

  it('falls back to a surviving <body> tag, and ignores transparent', () => {
    expect(declaredBodyBackground(parse('<body bgcolor="#fff"><p>hi</p></body>'))).toBe('#fff')
    expect(declaredBodyBackground(parse('<body style="background-color: transparent"><p>hi</p></body>'))).toBeNull()
    expect(declaredBodyBackground(parse('<p>hi</p>'))).toBeNull()
  })
})

describe('messageIsSelfStyled', () => {
  it('is false for a message that brings no colors', () => {
    expect(messageIsSelfStyled(parse('<div><p style="margin:0">hi</p><pre>code</pre></div>'))).toBe(false)
  })

  it('is true once anything declares a background or a text color', () => {
    expect(messageIsSelfStyled(parse('<meta name="meron-body-bg" content="#fff"><p>hi</p>'))).toBe(true)
    expect(messageIsSelfStyled(parse('<table bgcolor="#eee"><tr><td>hi</td></tr></table>'))).toBe(true)
    expect(messageIsSelfStyled(parse('<p style="color: #333">hi</p>'))).toBe(true)
    expect(messageIsSelfStyled(parse('<div style="background-image: url(x.png)">hi</div>'))).toBe(true)
    expect(messageIsSelfStyled(parse('<font color="red">hi</font>'))).toBe(true)
    expect(messageIsSelfStyled(parse('<style>.a { background: #fff }</style><p>hi</p>'))).toBe(true)
  })

  it('ignores properties that only look like colors', () => {
    expect(messageIsSelfStyled(parse('<style>td { border-color: #eee; -webkit-text-size-adjust: none }</style>'))).toBe(
      false,
    )
  })
})

describe('applyReaderTheme', () => {
  it('themes the gutter but leaves a self-styled message light', () => {
    const doc = parse('<meta name="meron-body-bg" content="#f5f4f2"><p style="color:#333">hi</p>')
    applyReaderTheme(doc, DARK)

    expect(vars(doc).getPropertyValue('page-bg')).toBe(DARK.pageBg)
    // The email's own page color is handed back to the body ammonia stripped it from.
    expect(canvasOf(doc)).toBe('#f5f4f2')
    // Its text colors were authored against a light background: no dark repaint.
    expect(vars(doc).getPropertyValue('text')).toBe('')
  })

  it('darkens a message that declares nothing', () => {
    const doc = parse('<p>hi</p>')
    applyReaderTheme(doc, DARK)

    expect(vars(doc).getPropertyValue('page-bg')).toBe(DARK.pageBg)
    expect(canvasOf(doc)).toBe('')
    expect(vars(doc).getPropertyValue('text')).toBe(DARK.text)
    expect(vars(doc).getPropertyValue('surface')).toBe(DARK.surface)
  })

  it('clears the dark vars when the theme switches back to light', () => {
    const doc = parse('<p>hi</p>')
    applyReaderTheme(doc, DARK)
    applyReaderTheme(doc, DEFAULT_READER_THEME)

    expect(vars(doc).getPropertyValue('text')).toBe('')
    expect(vars(doc).getPropertyValue('page-bg')).toBe(DEFAULT_READER_THEME.pageBg)
  })

  it('lets a light appearance hand the whole frame to the declared color', () => {
    const doc = parse('<meta name="meron-body-bg" content="#f5f4f2"><p>hi</p>')
    applyReaderTheme(doc, DEFAULT_READER_THEME)

    // No seam between the reader column and the gutter around it.
    expect(vars(doc).getPropertyValue('page-bg')).toBe('#f5f4f2')
    expect(canvasOf(doc)).toBe('#f5f4f2')
  })

  it('only follows the theme for the gutter in a light appearance', () => {
    const light = readerThemeFromTokens('light', builtinTheme('light')!.tokens)
    expect(light.text).toBe(DEFAULT_READER_THEME.text)
    expect(light.pageBg).toBe(builtinTheme('light')!.tokens.bgChat)
  })

  it('keeps darkening after the reader has injected its own stylesheet', () => {
    const doc = parse('<p>hi</p>')
    // READER_CSS is full of background/color declarations; reading them back as
    // sender styling would classify every message as self-styled from then on.
    applyReaderLayout(doc)
    applyReaderTheme(doc, DARK)

    expect(vars(doc).getPropertyValue('text')).toBe(DARK.text)
  })

  it('pairs a declared dark canvas with a readable foreground', () => {
    const declared = parse(
      '<meta name="meron-body-bg" content="#000000"><meta name="meron-body-fg" content="white"><p>hi</p>',
    )
    applyReaderTheme(declared, DARK)
    expect(canvasOf(declared)).toBe('#000000')
    expect(vars(declared).getPropertyValue('text')).toBe('white')

    // A black canvas with no text color of its own would otherwise keep the
    // light-mode default and render black on black.
    const bare = parse('<meta name="meron-body-bg" content="#000000"><p>hi</p>')
    applyReaderTheme(bare, DEFAULT_READER_THEME)
    expect(colorTone(vars(bare).getPropertyValue('text'))).toBe('light')
  })

  it('gives a message that only declares dark text a light canvas to sit on', () => {
    const doc = parse('<p style="color: #333">hi</p>')
    applyReaderTheme(doc, DARK)

    // Withholding the dark palette without a canvas is the dark-on-dark case.
    expect(canvasOf(doc)).toBe('#ffffff')
    expect(vars(doc).getPropertyValue('text')).toBe('')
  })

  it('leaves a message written for a dark canvas on the dark frame', () => {
    const doc = parse('<p style="color: white">hi</p>')
    applyReaderTheme(doc, DARK)

    // A white card under white text would be the light-on-light mirror image.
    expect(canvasOf(doc)).toBe('')
    expect(vars(doc).getPropertyValue('text')).toBe(DARK.text)
  })

  it('takes the tone of a body color hoisted off the stripped body', () => {
    // `<body style="color:white">`: a white canvas under it would be white-on-white.
    const doc = parse('<meta name="meron-body-fg" content="white"><p>hi</p>')
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('')
    // With no canvas in force the declared pair doesn't hold, so the frame's own
    // light-on-dark text is what the message renders in.
    expect(vars(doc).getPropertyValue('text')).toBe(DARK.text)
  })

  it('weighs a declared color by how much text it covers', () => {
    // A colored link inside a light-text wrapper must not outvote the wrapper.
    const doc = parse('<div style="color: white">a long stretch of light text <a style="color: black">link</a></div>')
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('')
  })

  it('refuses to restore a canvas whose color it cannot judge', () => {
    // Unknown keyword: pairing it with a foreground would be guesswork. With no
    // text colors of its own to protect, the message just takes the dark frame.
    const bare = parse('<meta name="meron-body-bg" content="rebeccapurple"><p>hi</p>')
    applyReaderTheme(bare, DARK)
    expect(canvasOf(bare)).toBe('')
    expect(vars(bare).getPropertyValue('text')).toBe(DARK.text)

    // With dark text of its own, it falls back to the readable card instead.
    const styled = parse('<meta name="meron-body-bg" content="rebeccapurple"><p style="color: #333">hi</p>')
    applyReaderTheme(styled, DARK)
    expect(canvasOf(styled)).toBe('#ffffff')
  })

  it('judges the color syntaxes the sanitiser lets through', () => {
    for (const dark of ['#000f', 'hsl(0, 0%, 0%)', 'rgb(0 0 0)', 'darkblue']) {
      const doc = parse(`<meta name="meron-body-bg" content="${dark}"><p>hi</p>`)
      applyReaderTheme(doc, DARK)
      expect(canvasOf(doc)).toBe(dark)
      expect(colorTone(vars(doc).getPropertyValue('text'))).toBe('light')
    }
  })

  it('does not mistake a sender element squatting on a reader style id or marker', () => {
    const doc = parse('<style id="meron-reader-style" data-meron-frame-style>body{color:#333}</style><p>hi</p>')
    applyReaderLayout(doc)

    // The squatter is disowned and the reader's real stylesheet still lands.
    const styles = [...doc.querySelectorAll('style')]
    expect(styles.some((style) => style.textContent?.includes(frameVar(frameVarPrefix(doc), 'page-bg')))).toBe(true)
    // Its colors are still the sender's, so the message counts as self-styled.
    applyReaderTheme(doc, DARK)
    expect(canvasOf(doc)).toBe('#ffffff')
  })

  it('resolves stylesheet rules against the elements they actually match', () => {
    // The unused rule matches nothing, so it can't tie with the one that paints.
    const doc = parse('<style>p { color: white } .unused { color: black }</style><p>Hi</p>')
    applyReaderTheme(doc, DARK)
    expect(canvasOf(doc)).toBe('')

    // Neither can a declaration that only exists inside a comment.
    const commented = parse('<style>/* p { color: black } */ p { color: white }</style><p>Hi</p>')
    applyReaderTheme(commented, DARK)
    expect(canvasOf(commented)).toBe('')

    // A rule inside a media query that applies here is found on its own.
    const media = parse('<style>@media screen { p { color: #222 } }</style><p>Hi</p>')
    applyReaderTheme(media, DARK)
    expect(canvasOf(media)).toBe('#ffffff')
  })

  it('ignores at-rules that describe a different rendering', () => {
    // The print override must not be read as the color on screen.
    const doc = parse('<style>p { color: #222 } @media print { p { color: white } }</style><p>Hi</p>')
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('#ffffff')
  })

  it('ranks stylesheet rules by importance and specificity', () => {
    // `#x` beats the later element rule, so this text really is white.
    const specific = parse('<style>#x { color: white } p { color: #222 }</style><p id="x">Hi</p>')
    applyReaderTheme(specific, DARK)
    expect(canvasOf(specific)).toBe('')

    // An important rule beats an inline declaration, as it does in the cascade.
    const important = parse('<style>p { color: white !important }</style><p style="color: #222">Hi</p>')
    applyReaderTheme(important, DARK)
    expect(canvasOf(important)).toBe('')
  })

  it('does not let hidden text decide the canvas', () => {
    // The preheader every newsletter opens with is longer than the message.
    const doc = parse(
      '<span style="display: none">' +
        'a hidden preheader that runs on and on and on'.repeat(3) +
        '</span><p style="color: #222">Hi</p>',
    )
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('#ffffff')
  })

  it('lets a descendant override the body default it sits under', () => {
    // The body default paints nothing: every visible character is recolored.
    const doc = parse('<meta name="meron-body-fg" content="white"><div style="color: #222">all of it</div>')
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('#ffffff')
  })

  it('counts stylesheet text colors when judging the canvas', () => {
    // The white-card fallback is only safe when the unknown styling is dark.
    const light = parse('<style>p { color: white }</style><p>hi</p>')
    applyReaderTheme(light, DARK)
    expect(canvasOf(light)).toBe('')

    const dark = parse('<style>p { color: #222 }</style><p>hi</p>')
    applyReaderTheme(dark, DARK)
    expect(canvasOf(dark)).toBe('#ffffff')
  })

  it('credits recolored text to the descendant that paints it', () => {
    // One white character around a long dark link: almost all the visible text
    // is dark, so the message still wants a light canvas.
    const doc = parse('<div style="color: white"> <a style="color: black">a long dark link</a></div>')
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('#ffffff')
  })

  it('does not restore a declared foreground without its canvas', () => {
    // The unknown canvas is refused, so its white text can't be restored alone.
    const doc = parse(
      '<meta name="meron-body-bg" content="rebeccapurple"><meta name="meron-body-fg" content="white"><p>hi</p>',
    )
    applyReaderTheme(doc, DEFAULT_READER_THEME)

    expect(canvasOf(doc)).toBe('')
    expect(vars(doc).getPropertyValue('text')).toBe('')
  })

  it('treats a see-through canvas as no canvas at all', () => {
    for (const transparent of ['rgba(0, 0, 0, 0)', '#0000', '#fff0']) {
      const doc = parse(`<meta name="meron-body-bg" content="${transparent}"><p>hi</p>`)
      applyReaderTheme(doc, DARK)
      // Nothing was really painted, so the frame's own dark palette applies.
      expect(canvasOf(doc)).toBe('')
      expect(vars(doc).getPropertyValue('text')).toBe(DARK.text)
    }
  })

  it('asks the engine what each run of text is painted, not the markup', () => {
    // Whether a media query applies is the engine's call: this one does, so its
    // color is the one on screen. A `max-width: 1px` block would not have been.
    const media = parse('<style>p { color: white } @media (min-width: 1px) { p { color: #222 } }</style><p>Hi</p>')
    applyReaderTheme(media, DARK)
    expect(canvasOf(media)).toBe('#ffffff')

    const inactive = parse('<style>p { color: #222 } @media (max-width: 1px) { p { color: white } }</style><p>Hi</p>')
    applyReaderTheme(inactive, DARK)
    expect(canvasOf(inactive)).toBe('#ffffff')

    // `:where()` adds no specificity, so the element rule wins and this is dark.
    const where = parse('<style>:where(#x) { color: white } p { color: #222 }</style><p id="x">Hi</p>')
    applyReaderTheme(where, DARK)
    expect(canvasOf(where)).toBe('#ffffff')

    // A selector group with a comma inside `:is()` is one selector, not two.
    const group = parse('<style>:is(p, span) { color: white }</style><p>Hi</p>')
    applyReaderTheme(group, DARK)
    expect(canvasOf(group)).toBe('')
  })

  it('counts text inside a hidden ancestor as hidden too', () => {
    const doc = parse(
      '<div style="display: none"><p style="color: white">' +
        'a long hidden preheader that runs on and on'.repeat(3) +
        '</p></div><p style="color: #222">Hi</p>',
    )
    applyReaderTheme(doc, DARK)

    expect(canvasOf(doc)).toBe('#ffffff')
  })

  it('lets the body color the sanitiser stripped win over a rule, as inline does', () => {
    const doc = parse('<meta name="meron-body-fg" content="white"><style>body { color: #222 }</style><p>Hi</p>')
    applyReaderTheme(doc, DARK)

    // The hoisted declaration was inline on <body>, so it outranks the rule and
    // the message really is light text.
    expect(canvasOf(doc)).toBe('')
  })

  it('keeps its own variables beyond the reach of sender CSS', () => {
    // Custom properties inherit, so a fixed name would be a hole in the frame's
    // own styling: one `body { --meron-text: #111 }` and the palette is theirs.
    const doc = parse('<style>body { --meron-text: #111; --meron-body-bg: transparent }</style><p>hi</p>')
    applyReaderLayout(doc, undefined, DARK)

    const name = frameVar(frameVarPrefix(doc), 'text')
    expect(name).not.toBe('--meron-text')
    expect(doc.documentElement.style.getPropertyValue(name)).toBe(DARK.text)
  })

  it('restores the declared body color even when the canvas came from the meta', () => {
    // The restore can't hang off the tone walk: a recognised background returns
    // before it, and the foreground would never be put back.
    const doc = parse(
      '<meta name="meron-body-bg" content="#000000"><meta name="meron-body-fg" content="white">' +
        '<meta name="meron-body-important" content="color">' +
        '<style>html body { color: #222 }</style><p>hi</p>',
    )
    applyReaderTheme(doc, DARK)

    // Inline and important, as it was written, so the sender's own rule loses.
    expect(doc.body.style.getPropertyValue('color')).toBe('white')
    expect(doc.body.style.getPropertyPriority('color')).toBe('important')
  })

  it('takes the declared body color away again when no canvas is accepted', () => {
    // White text with no canvas of its own would otherwise be restored onto the
    // frame's own light background.
    const doc = parse('<meta name="meron-body-fg" content="white"><p>hi</p>')
    applyReaderTheme(doc, DEFAULT_READER_THEME)
    expect(doc.body.style.getPropertyValue('color')).toBe('')

    // And it comes back once a canvas it belongs on is in force.
    const paired = parse(
      '<meta name="meron-body-bg" content="#000000"><meta name="meron-body-fg" content="white"><p>hi</p>',
    )
    applyReaderTheme(paired, DEFAULT_READER_THEME)
    expect(paired.body.style.getPropertyValue('color')).toBe('white')
  })

  it('restores a declared canvas inline, with the priority it was written with', () => {
    const doc = parse(
      '<meta name="meron-body-bg" content="#000000"><meta name="meron-body-fg" content="white">' +
        '<meta name="meron-body-important" content="background-color color">' +
        '<style>html body { background: #ffffff !important; color: #222 !important }</style><p>hi</p>',
    )
    applyReaderTheme(doc, DARK)

    // A sender rule the original outranked must still lose to it: a stylesheet
    // variable would not have survived this.
    expect(doc.body.style.getPropertyValue('background-color')).toBe('#000000')
    expect(doc.body.style.getPropertyPriority('background-color')).toBe('important')
  })

  it('leaves a canvas the frame chose itself to the frame', () => {
    // The white card is ours, not the message's, so it stays in the stylesheet.
    const doc = parse('<p style="color: #333">hi</p>')
    applyReaderTheme(doc, DARK)

    // Painted on the body, not declared in the frame's stylesheet: a stylesheet
    // declaration would be in the cascade even when the frame wants none.
    expect(doc.body.style.getPropertyValue('background-color')).toBe('#ffffff')
    expect(doc.body.style.getPropertyPriority('background-color')).toBe('')
  })

  it('leaves a background the message paints for itself alone', () => {
    // The reader declared no canvas of its own, so nothing of the frame's is in
    // the cascade to wipe the sender's rule.
    const doc = parse('<style>body { background: #000 } p { color: #fff }</style><p>Hi</p>')
    applyReaderLayout(doc, undefined, DEFAULT_READER_THEME)

    expect(doc.body.style.getPropertyValue('background-color')).toBe('')
    const sheet = [...doc.querySelectorAll('style')]
      .map((style) => style.textContent ?? '')
      .find((css) => css.includes('max-width: 760px'))
    expect(sheet?.slice(sheet.indexOf('body {'), sheet.indexOf('*, *::before'))).not.toContain('background')
  })
})
