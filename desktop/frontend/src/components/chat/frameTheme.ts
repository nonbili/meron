import { formatColor, luminance, mix, parseColor, type Rgba } from '../../lib/color'
import type { Appearance, ThemeTokens } from '../../lib/themes'

// A message body renders in its own document, so it inherits none of the app's
// CSS vars: the active theme has to be handed to each frame as plain values.
// This module holds those palettes and the one question both frames ask before
// using the dark half of them — whether the message brings colors of its own.

/**
 * The colors a body frame paints itself with. The frame can't reach the app's
 * CSS vars (it's a separate document), so the active theme arrives here as plain
 * values and is written onto the frame's `<html>` as `--meron-*` properties that
 * READER_CSS reads, with today's light values as fallbacks.
 */
export interface ReaderTheme {
  appearance: Appearance
  /** Draw a light self-styled message inverted rather than as a light card. */
  darkenStyled: boolean
  /** Behind the reader column — always themed, whatever the message declares. */
  pageBg: string
  text: string
  /** Code blocks and inline code. */
  surface: string
  raised: string
  raisedHover: string
  border: string
  muted: string
  mutedStrong: string
}

export const DEFAULT_READER_THEME: ReaderTheme = {
  appearance: 'light',
  darkenStyled: false,
  pageBg: '#f8fafc',
  text: '#0f172a',
  surface: '#f1f5f9',
  raised: 'rgba(255, 255, 255, 0.92)',
  raisedHover: '#ffffff',
  border: '#e2e8f0',
  muted: '#cbd5e1',
  mutedStrong: '#94a3b8',
}

/** Map the active theme's tokens onto the frame's palette. */
export function readerThemeFromTokens(appearance: Appearance, tokens: ThemeTokens, darkenStyled = false): ReaderTheme {
  if (appearance === 'light') {
    // Light frames keep the hand-picked slate palette; only the gutter follows
    // the theme, so a tinted light theme doesn't sit next to a grey letterbox.
    return { ...DEFAULT_READER_THEME, pageBg: tokens.bgChat }
  }
  return {
    appearance,
    darkenStyled,
    pageBg: tokens.bgChat,
    text: tokens.textPrimary,
    surface: tokens.bgRaised,
    raised: tokens.bgRaised,
    raisedHover: tokens.bgActive,
    border: tokens.border,
    muted: tokens.border,
    mutedStrong: tokens.textSecondary,
  }
}

/**
 * The colors a bubble body frame paints itself with. Unlike the reader the
 * bubble has no page of its own: its document is transparent and the bubble
 * behind it shows through, so the palette follows the bubble it sits in —
 * incoming or outgoing, which carry different backgrounds in every theme.
 */
export interface BubbleTheme {
  appearance: Appearance
  /** Draw a light self-styled message inverted rather than as a light card. */
  darkenStyled: boolean
  text: string
  link: string
  /** Code blocks and inline code. */
  surface: string
  surfaceText: string
  border: string
  /** The copy-code button's outline, a shade stronger than the code border. */
  strongBorder: string
  muted: string
  raised: string
  raisedHover: string
  /** Text on a search hit's yellow highlight, which never follows the theme. */
  highlightText: string
  /** What a darkened body turns its white into: the surface the frame sits on. */
  darkenedBackdrop: string
}

/** The foreground for a canvas that turned out to be dark and declared no text color. */
export const LIGHT_ON_DARK_TEXT = '#f8fafc'

export const DEFAULT_BUBBLE_THEME: BubbleTheme = {
  appearance: 'light',
  darkenStyled: false,
  text: '#0f172a',
  link: '#4f46e5',
  surface: '#f1f5f9',
  surfaceText: '#1e293b',
  border: '#e2e8f0',
  strongBorder: '#cbd5e1',
  muted: '#64748b',
  raised: 'rgba(255, 255, 255, 0.92)',
  raisedHover: '#ffffff',
  highlightText: 'inherit',
  darkenedBackdrop: '#000000',
}

/** What a message body frame sits on: a chat bubble, a draft's bubble, or a traditional layout's row. */
export type FrameSurface = 'bubble' | 'draft' | 'row'

/** How much of the outgoing colour a draft's bubble is painted with (`bg-bubble-out/55`). */
const DRAFT_BUBBLE_OPACITY = 0.55

/** The colour of the surface a frame sits on, for a darkened body to blend onto. */
function surfaceColor(tokens: ThemeTokens, outgoing: boolean, surface: FrameSurface): string {
  if (surface === 'row') return tokens.bgChats
  // A draft's bubble is see-through; over the conversation's own background it
  // comes out as this. A wallpaper behind it can't be matched from in here.
  if (surface === 'draft') return mix(tokens.bgChat, tokens.bubbleOut, DRAFT_BUBBLE_OPACITY)
  return outgoing ? tokens.bubbleOut : tokens.bubbleIn
}

/** Map the active theme's tokens onto a bubble frame's palette. */
export function bubbleThemeFromTokens(
  appearance: Appearance,
  tokens: ThemeTokens,
  outgoing: boolean,
  darkenStyled = false,
  surface: FrameSurface = 'bubble',
): BubbleTheme {
  // Light bubbles are already the palette these frames were drawn for.
  if (appearance === 'light') return DEFAULT_BUBBLE_THEME
  return {
    appearance,
    darkenStyled,
    text: outgoing ? tokens.bubbleOutText : tokens.bubbleInText,
    link: tokens.accent,
    surface: tokens.bgRaised,
    surfaceText: outgoing ? tokens.bubbleOutText : tokens.bubbleInText,
    border: tokens.border,
    strongBorder: tokens.border,
    muted: tokens.textSecondary,
    raised: tokens.bgRaised,
    raisedHover: tokens.bgActive,
    // The highlight keeps its yellow in both appearances, so its text can't.
    highlightText: '#0f172a',
    darkenedBackdrop: surfaceColor(tokens, outgoing, surface),
  }
}

const TRANSPARENT_BG = new Set(['', 'transparent', 'none', 'inherit', 'initial', 'unset', 'revert'])
const INHERITED_COLOR = new Set(['', 'inherit', 'initial', 'unset', 'revert', 'currentcolor'])
// The stylesheets the frames inject themselves. Ownership is tracked by node
// where the node survives — the reader injects into the live document — and by
// a marker attribute where it can't: the bubble's stylesheet is written into
// HTML that is parsed again inside the frame.
const ownedStyles = new WeakSet<Element>()

/**
 * The marker on a stylesheet a frame wrote. Sender markup can't carry it: it is
 * stripped from any inherited document by `disownStyleElements` before a frame's
 * own is added, and the sanitiser drops every `data-*` attribute besides.
 */
export const FRAME_STYLE_MARKER = 'data-meron-frame-style'

/**
 * Claim a style element the frame injected, so it isn't read back as sender
 * styling. The marker carries the document's variable prefix, which is how it
 * survives a frame whose HTML is serialised and parsed again.
 */
export function ownStyleElement(el: Element) {
  ownedStyles.add(el)
  el.setAttribute(FRAME_STYLE_MARKER, frameVarPrefix(el.ownerDocument))
}

/** Drop the marker from anything that arrived carrying it. */
export function disownStyleElements(doc: Document) {
  for (const el of doc.querySelectorAll(`[${FRAME_STYLE_MARKER}]`)) {
    if (!ownedStyles.has(el)) el.removeAttribute(FRAME_STYLE_MARKER)
  }
}

// Custom properties inherit and are addressable by name, so a static
// `--meron-text` is a sender-writable hole in the frame's own styling: one
// `body { --meron-text: #111 }` and the palette is theirs. The names are
// per-document and unguessable instead.
const varPrefixes = new WeakMap<Document, string>()

/**
 * The prefix this document's frame variables are named with — recovered from
 * the frame's own stylesheet when its HTML was parsed again inside the frame,
 * and minted fresh otherwise.
 */
export function frameVarPrefix(doc: Document): string {
  const known = varPrefixes.get(doc)
  if (known) return known

  const carried = doc.querySelector(`style[${FRAME_STYLE_MARKER}]`)?.getAttribute(FRAME_STYLE_MARKER) ?? ''
  const prefix = /^[a-z0-9]{8,}$/.test(carried) ? carried : mintPrefix()
  varPrefixes.set(doc, prefix)
  return prefix
}

function mintPrefix(): string {
  const random = globalThis.crypto?.randomUUID?.().replace(/-/g, '')
  return random ?? Math.random().toString(36).slice(2).padEnd(12, '0')
}

/** Name one of the frame's variables for this document. */
export function frameVar(prefix: string, name: string): string {
  return `--meron-${prefix}-${name}`
}

/** Whether this element is one the frame injected itself. */
export function isOwnStyleElement(el: Element | null): boolean {
  return !!el && (ownedStyles.has(el) || el.hasAttribute(FRAME_STYLE_MARKER))
}
// Enough of the CSS named colors to judge the ones mail actually writes out.
// A keyword outside this table classifies as unknown, which is a safe answer:
// `frameCanvasBackground` refuses to restore a canvas it can't judge.
const NAMED_COLORS: Record<string, string> = {
  white: '#ffffff',
  whitesmoke: '#f5f5f5',
  ivory: '#fffff0',
  snow: '#fffafa',
  beige: '#f5f5dc',
  linen: '#faf0e6',
  azure: '#f0ffff',
  lavender: '#e6e6fa',
  wheat: '#f5deb3',
  gold: '#ffd700',
  yellow: '#ffff00',
  lightyellow: '#ffffe0',
  aqua: '#00ffff',
  cyan: '#00ffff',
  lime: '#00ff00',
  lightblue: '#add8e6',
  lightgreen: '#90ee90',
  lightgray: '#d3d3d3',
  lightgrey: '#d3d3d3',
  pink: '#ffc0cb',
  orange: '#ffa500',
  silver: '#c0c0c0',
  tan: '#d2b48c',
  gray: '#808080',
  grey: '#808080',
  darkgray: '#a9a9a9',
  darkgrey: '#a9a9a9',
  black: '#000000',
  navy: '#000080',
  midnightblue: '#191970',
  darkblue: '#00008b',
  darkred: '#8b0000',
  darkgreen: '#006400',
  maroon: '#800000',
  purple: '#800080',
  teal: '#008080',
  olive: '#808000',
  red: '#ff0000',
  blue: '#0000ff',
  green: '#008000',
  magenta: '#ff00ff',
  fuchsia: '#ff00ff',
}

// Below this the color is too see-through to judge on its own: what shows
// through it decides, and that is whatever the frame is painting.
const OPAQUE_ENOUGH = 0.6

/**
 * Whether a color reads as light or dark, or null when it can't be judged — an
 * unknown keyword, a value the parser doesn't cover, a color transparent enough
 * that the surface under it decides. Callers must treat null as "don't assume",
 * never as a tone.
 */
export function colorTone(color: string): 'light' | 'dark' | null {
  const value = color.trim().toLowerCase()
  const resolved = NAMED_COLORS[value] ?? value
  const parsed = parseColor(resolved)
  if (!parsed || parsed.a < OPAQUE_ENOUGH) return null
  return luminance(resolved) > 0.55 ? 'light' : 'dark'
}

/** The canvas the message declares for its whole page: `background` and its `text`. */
export interface DeclaredCanvas {
  background: string | null
  /** The text color declared alongside it, which has to travel with it. */
  text: string | null
  /** Which of the two the message declared `!important`: "background", "color". */
  important: string[]
}

/**
 * The canvas colors the message declares on its own body.
 *
 * `prepare_html` hoists them off the `<body>` tag into `<meta>`s (the tag itself
 * doesn't survive sanitising); stored feed HTML can still carry a real `<body>`,
 * so both are checked.
 */
export function declaredCanvas(doc: Document): DeclaredCanvas {
  const meta = (name: string) => doc.querySelector(`meta[name="${name}"]`)?.getAttribute('content')?.trim() ?? ''
  const body = doc.body
  const background = meta('meron-body-bg') || body?.style.backgroundColor || body?.getAttribute('bgcolor') || ''
  const text = meta('meron-body-fg') || body?.style.color || body?.getAttribute('text') || ''

  return {
    background: TRANSPARENT_BG.has(background.trim().toLowerCase()) ? null : background.trim(),
    text: INHERITED_COLOR.has(text.trim().toLowerCase()) ? null : text.trim(),
    important: meta('meron-body-important').split(/\s+/).filter(Boolean),
  }
}

/** Kept for the frames that only need the page color. */
export function declaredBodyBackground(doc: Document): string | null {
  return declaredCanvas(doc).background
}

/**
 * Whether the message brings colors of its own — a background anywhere, or text
 * colors it expects to sit on one.
 *
 * This is the gate on dark rendering. Painting the body dark is only safe when
 * the message declares nothing: an email that sets `color: #333` on a background
 * it never declares (assuming the client's is white) would otherwise turn into
 * dark-on-dark, and one that paints its own light card would get light text on
 * it. Those keep their design and are given a canvas to sit on instead.
 */
export function messageIsSelfStyled(doc: Document): boolean {
  // A background only counts as styling if it actually paints: a fully
  // see-through one leaves the frame's own canvas showing, which is the
  // unstyled case. A text color counts either way — an unrecognised one is
  // still a color the sender chose against some canvas, and the conservative
  // answer is to give it the one it expects.
  const canvas = declaredCanvas(doc)
  if (canvas.text) return true
  if (canvas.background && colorTone(canvas.background)) return true

  for (const el of doc.querySelectorAll<HTMLElement>('[style], [bgcolor], [color], font')) {
    if (el.hasAttribute('bgcolor') || el.hasAttribute('color')) return true
    const bg = el.style.backgroundColor.trim()
    if (bg && colorTone(bg)) return true
    const color = el.style.color.trim().toLowerCase()
    if (color && !INHERITED_COLOR.has(color)) return true
    const image = el.style.backgroundImage.trim().toLowerCase()
    if (image && !TRANSPARENT_BG.has(image)) return true
  }

  // Stylesheet rules can't be read declaration by declaration (the CSS may not
  // even have parsed under the frame's CSP), so a `color`/`background` rule in
  // any <style> block counts as styled — except in the stylesheets the reader
  // injects itself (identified by node, never by their id), which are full of both.
  for (const style of doc.querySelectorAll('style')) {
    if (isOwnStyleElement(style)) continue
    if (/(^|[;{\s])(background[\w-]*|color)\s*:/i.test(style.textContent ?? '')) return true
  }

  return false
}

const SKIPPED_TAGS = new Set(['STYLE', 'SCRIPT', 'HEAD', 'TITLE', 'TEMPLATE'])

/**
 * Put the declarations the message wrote on its own `<body>` back where they
 * were, or take them away again.
 *
 * Sanitising drops the `<body>` tag, so `prepare_html` hoists its colors — and
 * which of them were `!important` — into `<meta>`s. Restoring them as the inline
 * declarations they were keeps the document faithful and lets the engine resolve
 * them against the sender's own stylesheets the way it would have: inline beats
 * a `body { color: … }` rule, and an important one beats a rule that outranks a
 * normal declaration.
 *
 * Only what the frame itself restored is ever removed again.
 */
const restoredBodyDeclarations = new WeakMap<Document, string[]>()

function writeBodyDeclarations(doc: Document, properties: Array<[string, string | null, boolean?]>) {
  const body = doc.body
  if (!body) return
  const written = restoredBodyDeclarations.get(doc) ?? []

  for (const [property, value, important] of properties) {
    if (value) {
      // The message's own surviving declaration is left alone; nothing was lost.
      if (body.style.getPropertyValue(property) && !written.includes(property)) continue
      body.style.setProperty(property, value, important ? 'important' : '')
      if (!written.includes(property)) written.push(property)
    } else if (written.includes(property)) {
      body.style.removeProperty(property)
      written.splice(written.indexOf(property), 1)
    }
  }
  restoredBodyDeclarations.set(doc, written)
}

/** What a frame should paint the message on, and the text color that goes with it. */
export interface FrameCanvas {
  /** The canvas, or null to leave the frame's own background showing. */
  background: string | null
  /** The color to paint text in, or null to leave the frame's palette in charge. */
  text: string | null
  /**
   * Whether the frame paints the canvas itself.
   *
   * False when the canvas is the message's own, restored as the inline
   * declaration it was: painting it again from the frame's stylesheet would
   * enter the cascade a second time, at the frame's priority rather than the
   * one the sender wrote — enough to beat a sender rule that originally won.
   */
  framePaints: boolean
}

/**
 * Decide the canvas, and put the message's own body declarations back only if
 * they are part of it.
 *
 * The decision needs the declared text color in place — it is what the engine
 * inherits into the message — but the pair only holds together: with no canvas
 * accepted, a restored `color: white` would render on the frame's own light
 * background, so it is taken away again.
 */
export function frameCanvas(
  doc: Document,
  appearance: Appearance,
  fallbackText: string,
  /** False where the frame's own background already suits the message — a light
   *  bubble under a light design — which still has to undo an earlier restore. */
  wantsCanvas = true,
  preparedBackground?: { background: string | null },
): FrameCanvas {
  const declared = declaredCanvas(doc)
  const isImportant = (property: string) => declared.important.includes(property)
  writeBodyDeclarations(doc, [['color', declared.text, isImportant('color')]])

  const background = preparedBackground
    ? preparedBackground.background
    : wantsCanvas
      ? frameCanvasBackground(doc, appearance)
      : null
  const restoresDeclared = !!background && background === declared.background
  // The canvas is painted on the body itself rather than from the frame's
  // stylesheet. A stylesheet declaration is in the cascade whether or not the
  // frame wants one — `background: var(--unset, transparent)` still resolves to
  // transparent, and an unresolved `var()` computes to `initial`, which is the
  // same thing — so it would quietly wipe a background the message paints for
  // itself. The message's own comes back with the priority it was written with;
  // one the frame chose is a normal declaration, and a sender's `!important`
  // still beats it.
  writeBodyDeclarations(doc, [
    ['color', background ? declared.text : null, isImportant('color')],
    ['background-color', background, restoresDeclared && isImportant('background-color')],
  ])

  return {
    background,
    text: canvasTextColor(doc, background, fallbackText),
    framePaints: !!background && !restoresDeclared,
  }
}

// Text the sender hides — the preheader every newsletter opens with, a
// screen-reader-only line — is not what the message looks like, so it can't be
// what decides the canvas.
function isHidden(style: CSSStyleDeclaration): boolean {
  const display = style.display.trim().toLowerCase()
  if (display === 'none') return true
  const visibility = style.visibility.trim().toLowerCase()
  if (visibility === 'hidden' || visibility === 'collapse') return true
  if (style.opacity.trim() === '0') return true
  const fontSize = style.fontSize.trim()
  if (fontSize === '0' || fontSize === '0px') return true
  const flat = ['0', '0px'].includes(style.maxHeight.trim()) || ['0', '0px'].includes(style.height.trim())
  return flat && style.overflow.trim().toLowerCase() === 'hidden'
}

/**
 * The tone the message's own text colors expect from the canvas under them, or
 * null when it declares none. Dark text wants a light canvas, and vice versa.
 *
 * The cascade is not re-implemented here: the document is a real one in a real
 * engine, so each run of text is asked what color it is actually painted in —
 * specificity, importance, inheritance, selector syntax and media queries all
 * resolved by the engine that will render it. Every visible run then counts for
 * its own length, so the colors that cover the message decide it, and the ones
 * on a hidden preheader or an empty wrapper decide nothing.
 */
function* textToneWalk(doc: Document): Generator<void, 'light' | 'dark' | null> {
  const win = doc.defaultView
  if (!win || !doc.body) return null
  let light = 0
  let dark = 0
  const pending: Element[] = [doc.body]
  while (pending.length) {
    const el = pending.pop()!
    yield
    if (SKIPPED_TAGS.has(el.tagName)) continue
    const style = win.getComputedStyle(el)
    if (isHidden(style)) continue
    const tone = colorTone(style.color)
    for (const node of el.childNodes) {
      if (node.nodeType === Node.ELEMENT_NODE) pending.push(node as Element)
      else if (node.nodeType === Node.TEXT_NODE && tone) {
        const length = node.textContent?.trim().length ?? 0
        if (tone === 'light') light += length
        else dark += length
      }
    }
  }
  return !light && !dark ? null : light > dark ? 'dark' : 'light'
}

// Give input and paint a turn between small batches of computed-style reads.
async function runThemeWalk<T>(walk: Generator<void, T>, cancelled: () => boolean): Promise<T | undefined> {
  await new Promise<void>((resolve) => window.setTimeout(resolve, 0))
  let slice = performance.now()
  while (!cancelled()) {
    const step = walk.next()
    if (step.done) return step.value
    if (performance.now() - slice >= 4) {
      await new Promise<void>((resolve) => window.setTimeout(resolve, 0))
      slice = performance.now()
    }
  }
  return undefined
}

export async function frameCanvasAsync(
  doc: Document,
  appearance: Appearance,
  fallbackText: string,
  wantsCanvas: boolean,
  cancelled: () => boolean,
): Promise<FrameCanvas | undefined> {
  const declared = declaredCanvas(doc)
  writeBodyDeclarations(doc, [['color', declared.text, declared.important.includes('color')]])
  const background = wantsCanvas ? await runThemeWalk(canvasBackgroundWalk(doc, appearance), cancelled) : null
  if (cancelled()) return undefined
  return frameCanvas(doc, appearance, fallbackText, wantsCanvas, { background: background ?? null })
}

/**
 * The canvas a self-styled message needs behind it, or null to leave the frame's
 * own background showing.
 *
 * A message that declares colors authored for another client's canvas has to be
 * given one, or it renders as unreadable same-on-same text: its own declared
 * page color when it has one, otherwise white for the usual dark-text mail. Mail
 * that declares only light text was written for a dark canvas and already has
 * one — the frame's — so it keeps it.
 */
function* canvasBackgroundWalk(doc: Document, appearance: Appearance): Generator<void, string | null> {
  const { background } = declaredCanvas(doc)
  // A canvas whose color can't be judged can't be paired with a readable
  // foreground either, so it is treated as no canvas at all and falls through to
  // the fallback below rather than risking same-on-same text.
  if (background && colorTone(background)) return background
  if (appearance === 'light') return null
  if (!messageIsSelfStyled(doc)) return null
  return (yield* textToneWalk(doc)) === 'dark' ? null : '#ffffff'
}

export function frameCanvasBackground(doc: Document, appearance: Appearance): string | null {
  const walk = canvasBackgroundWalk(doc, appearance)
  let step = walk.next()
  while (!step.done) step = walk.next()
  return step.value
}

/**
 * The text color to pair with a canvas, when the message didn't declare one:
 * a dark canvas restored on its own would otherwise keep the light-mode default
 * and render dark-on-dark.
 */
export function canvasTextColor(doc: Document, canvas: string | null, fallback: string): string | null {
  // The declared pair only holds together: with no canvas in force, restoring
  // the foreground alone would put the message's white text on a light frame.
  if (!canvas) return null
  const { text } = declaredCanvas(doc)
  if (text) return text
  return colorTone(canvas) === 'dark' ? fallback : null
}

/**
 * Set on a frame's `<html>` while its body is drawn inverted. The sanitiser
 * drops every `data-*` attribute, so sender markup can't carry it.
 */
export const DARKENED_ATTR = 'data-meron-darkened'

/** Set by `setDarkened` on an element that is a background picture and nothing else. */
export const PICTURE_ATTR = 'data-meron-picture'

/** `hue-rotate(180deg)` as the matrix the engine applies, row by row. */
const HUE_TURN = [
  [-0.574, 1.43, 0.144],
  [0.426, 0.43, 0.144],
  [0.426, 1.43, -0.856],
]

/**
 * The color a darkened body has to be painted with to come out as `surface`:
 * the darkening filter run backwards. The filter can't reach every color — a
 * very saturated surface falls outside what it produces — and then this is the
 * nearest it gets to, a shade duller than the surface itself.
 */
export function darkenedFloor(surface: string): string {
  const color = parseColor(surface)
  if (!color) return '#ffffff'
  const channels = [color.r, color.g, color.b]
  const [r, g, b] = HUE_TURN.map((row) => 255 - row.reduce((sum, weight, i) => sum + weight * channels[i], 0))
  return formatColor({ r, g, b, a: 1 })
}

/**
 * The rules that draw a darkened body: inverted, with the hue turned back so
 * links and brand colors stay recognisable, and pictures inverted a second time
 * so they come out as sent — once only, so an image inside a re-inverted
 * background picture is left alone. The search highlight keeps its yellow too.
 *
 * Inverted white is black, not the surface the frame sits on, so a layer over
 * the body caps how light it gets before the filter runs: at the color that
 * inverts to that surface (see `darkenedFloor`). The layer is the first thing
 * positioned in the body, above everything in the flow and below whatever
 * comes after it with a filter of its own — the pictures, which a cap applied
 * to the whole body would tint with the surface instead. Whatever else paints
 * above it — a box the sender positioned, and all that is in one — has its
 * background capped one element at a time (see `cappedBackground`).
 */
export const darkenedCss = (v: (name: string) => string) => `
  html[${DARKENED_ATTR}] body,
  html[${DARKENED_ATTR}] body :is(img, video, [${PICTURE_ATTR}], mark.meron-search-hit) {
    filter: invert(1) hue-rotate(180deg);
  }
  html[${DARKENED_ATTR}] body [${PICTURE_ATTR}] img {
    filter: none;
  }
  html[${DARKENED_ATTR}] body::before {
    content: '' !important;
    display: block !important;
    position: absolute !important;
    inset: 0 !important;
    background: var(${v('darkened-floor')}, #ffffff) !important;
    mix-blend-mode: darken !important;
    pointer-events: none !important;
  }
`

/**
 * Whether a frame should draw its body inverted: the reader asked for dark
 * bodies, the theme is dark, and the message was given a light canvas of its
 * own — the one case a dark theme otherwise leaves bright. Mail with no colors
 * of its own is already painted in the theme's palette, and a dark canvas is
 * dark already.
 */
export function darkensCanvas(appearance: Appearance, darkenStyled: boolean, canvas: string | null): boolean {
  return darkenStyled && appearance === 'dark' && !!canvas && colorTone(canvas) === 'light'
}

/**
 * Whether an element is a picture painted as a background, with no text of its
 * own. Only those are turned back to their own colors: turning a box back also
 * turns back the text in it, which would then read as sent — dark — on a canvas
 * that is now dark too. `background-image: none` is no picture at all.
 */
function isBackgroundPicture(el: HTMLElement): boolean {
  if (el.textContent?.trim()) return false
  const win = el.ownerDocument.defaultView
  // A document that was only parsed has no engine to resolve stylesheets; its
  // inline declaration and the legacy attribute are all there is to go on.
  const image = win ? win.getComputedStyle(el).backgroundImage : el.style.backgroundImage
  return /url\(/i.test(image) || (!win && !!el.getAttribute('background')?.trim())
}

/**
 * Set by `setDarkened` on an element whose background color it capped, holding
 * the inline declaration that was there before: its priority, a bar, its value.
 */
export const CAPPED_ATTR = 'data-meron-capped'

/** Elements that are turned back to their own colors, which a cap would then show through. */
const UNCAPPED_SELECTOR = `img, video, mark.meron-search-hit, [${PICTURE_ATTR}], [${PICTURE_ATTR}] *`

/**
 * An element's background color held to the floor, or null if it is under it
 * already. The layer over the body (see `darkenedCss`) caps what is in the
 * flow, but a positioned box paints above that layer, and so does everything
 * inside one: its white would invert to black, not to the surface.
 */
function cappedBackground(el: HTMLElement, floor: Rgba): string | null {
  if (el.matches(UNCAPPED_SELECTOR)) return null
  const win = el.ownerDocument.defaultView
  const declared = win
    ? win.getComputedStyle(el).backgroundColor
    : el.style.backgroundColor || el.getAttribute('bgcolor') || ''
  const color = parseColor(declared)
  if (!color || color.a === 0) return null
  if (color.r <= floor.r && color.g <= floor.g && color.b <= floor.b) return null
  return formatColor({
    r: Math.min(color.r, floor.r),
    g: Math.min(color.g, floor.g),
    b: Math.min(color.b, floor.b),
    a: color.a,
  })
}

/** Put back the inline background an element had before it was capped. */
function uncapBackground(el: HTMLElement) {
  const saved = el.getAttribute(CAPPED_ATTR) ?? ''
  const bar = saved.indexOf('|')
  const value = saved.slice(bar + 1)
  if (value) el.style.setProperty('background-color', value, saved.slice(0, bar))
  else el.style.removeProperty('background-color')
  el.removeAttribute(CAPPED_ATTR)
}

/** Mark or unmark a frame's document as darkened (see `darkenedCss`). */
function* darkenedWalk(doc: Document, darkened: boolean, floor: string): Generator<void, void> {
  doc.documentElement.toggleAttribute(DARKENED_ATTR, darkened)
  for (const el of doc.querySelectorAll(`[${PICTURE_ATTR}]`)) {
    el.removeAttribute(PICTURE_ATTR)
    yield
  }
  for (const el of doc.querySelectorAll<HTMLElement>(`[${CAPPED_ATTR}]`)) {
    uncapBackground(el)
    yield
  }
  if (!darkened || !doc.body) return
  const cap = parseColor(floor)
  // Written once the walk has read every element: a write in between would
  // have the engine resolve the document's styles again for the next read.
  const capped: Array<[HTMLElement, string]> = []
  for (const el of doc.body.querySelectorAll<HTMLElement>('*')) {
    if (isBackgroundPicture(el)) el.setAttribute(PICTURE_ATTR, '')
    const background = cap && cappedBackground(el, cap)
    if (background) capped.push([el, background])
    yield
  }
  for (const [el, background] of capped) {
    const { style } = el
    el.setAttribute(
      CAPPED_ATTR,
      `${style.getPropertyPriority('background-color')}|${style.getPropertyValue('background-color')}`,
    )
    style.setProperty('background-color', background, 'important')
  }
}

/** `floor` is the color a darkened body is capped at (see `darkenedFloor`). */
export function setDarkened(doc: Document, darkened: boolean, floor: string) {
  for (const _ of darkenedWalk(doc, darkened, floor)) {
    /* finish synchronously for reader callers */
  }
}

export async function setDarkenedAsync(doc: Document, darkened: boolean, floor: string, cancelled: () => boolean) {
  await runThemeWalk(darkenedWalk(doc, darkened, floor), cancelled)
}
