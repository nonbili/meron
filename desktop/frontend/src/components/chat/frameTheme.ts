import { luminance, parseColor } from '../../lib/color'
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
export function readerThemeFromTokens(appearance: Appearance, tokens: ThemeTokens): ReaderTheme {
  if (appearance === 'light') {
    // Light frames keep the hand-picked slate palette; only the gutter follows
    // the theme, so a tinted light theme doesn't sit next to a grey letterbox.
    return { ...DEFAULT_READER_THEME, pageBg: tokens.bgChat }
  }
  return {
    appearance,
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
}

/** The foreground for a canvas that turned out to be dark and declared no text color. */
export const LIGHT_ON_DARK_TEXT = '#f8fafc'

export const DEFAULT_BUBBLE_THEME: BubbleTheme = {
  appearance: 'light',
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
}

/** Map the active theme's tokens onto a bubble frame's palette. */
export function bubbleThemeFromTokens(appearance: Appearance, tokens: ThemeTokens, outgoing: boolean): BubbleTheme {
  // Light bubbles are already the palette these frames were drawn for.
  if (appearance === 'light') return DEFAULT_BUBBLE_THEME
  return {
    appearance,
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
): FrameCanvas {
  const declared = declaredCanvas(doc)
  const isImportant = (property: string) => declared.important.includes(property)
  writeBodyDeclarations(doc, [['color', declared.text, isImportant('color')]])

  const background = wantsCanvas ? frameCanvasBackground(doc, appearance) : null
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

// Text the sender hides// Text the sender hides — the preheader every newsletter opens with, a
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
function declaredTextTone(doc: Document): 'light' | 'dark' | null {
  const win = doc.defaultView
  // A document that was only parsed has no engine to ask, and no rendering to
  // describe. Callers treat null as "nothing declared", which is the safe read.
  if (!win) return null

  let light = 0
  let dark = 0
  const walk = (el: Element) => {
    if (SKIPPED_TAGS.has(el.tagName)) return
    const style = win.getComputedStyle(el)
    if (isHidden(style)) return

    const tone = colorTone(style.color)
    for (const node of el.childNodes) {
      if (node.nodeType === Node.ELEMENT_NODE) {
        walk(node as Element)
        continue
      }
      if (node.nodeType !== Node.TEXT_NODE || !tone) continue
      // Only the text this element paints itself: a child's runs are counted
      // with the child, in whatever color it resolves to.
      const length = node.textContent?.trim().length ?? 0
      if (tone === 'light') light += length
      else dark += length
    }
  }
  if (doc.body) walk(doc.body)

  if (!light && !dark) return null
  // Text lighter than the canvas it never declared means the sender assumed a
  // dark one, so the tone the *canvas* wants is the opposite of the text's.
  return light > dark ? 'dark' : 'light'
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
export function frameCanvasBackground(doc: Document, appearance: Appearance): string | null {
  const { background } = declaredCanvas(doc)
  // A canvas whose color can't be judged can't be paired with a readable
  // foreground either, so it is treated as no canvas at all and falls through to
  // the fallback below rather than risking same-on-same text.
  if (background && colorTone(background)) return background
  if (appearance === 'light') return null
  if (!messageIsSelfStyled(doc)) return null
  return declaredTextTone(doc) === 'dark' ? null : '#ffffff'
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
