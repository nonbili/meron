import { BUBBLE_CODE_BASE_PX, BUBBLE_HTML_BASE_PX, type MessageFrameFont } from '../../lib/fonts'
import { QUOTE_FOLDED_CLASS, QUOTE_TOGGLE_CLASS } from './quoteFold'
import { allowRemoteContent, allowRemoteInCsp, blockRemoteContent, blockRemoteInCsp } from './remoteContentCsp'
import {
  DARKENED_ATTR,
  FRAME_STYLE_MARKER,
  DEFAULT_BUBBLE_THEME,
  LIGHT_ON_DARK_TEXT,
  colorTone,
  darkenedCss,
  darkenedFloor,
  darkensCanvas,
  disownStyleElements,
  frameCanvas,
  frameCanvasAsync,
  type FrameCanvas,
  frameVar,
  frameVarPrefix,
  ownStyleElement,
  declaredCanvas,
  frameCanvasBackground,
  setDarkened,
  setDarkenedAsync,
  type BubbleTheme,
} from './frameTheme'

/** The attribute `prepareBubbleHtml` stamps its generation on. */
export const FRAME_GENERATION_MARKER = 'data-meron-generation'

/**
 * Hold the place of a picture that has not loaded, from its width and height
 * attributes. Returns whether it did, so the caller can give the place back.
 *
 * `auto`: once the picture loads its own proportions win, so mislabelled
 * attributes only last until then. A sender's inline ratio is preserved;
 * browser-derived ratios still get the same explicit reservation.
 */
export function reserveImageBox(image: HTMLImageElement | HTMLVideoElement): boolean {
  const width = plainLength(image.getAttribute('width'))
  const height = plainLength(image.getAttribute('height'))
  if (width === null || height === null) return false
  const current = image.style.aspectRatio || image.ownerDocument.defaultView?.getComputedStyle(image).aspectRatio
  // Browsers derive `auto w / h` from attributes; a fixed authored ratio
  // keeps its own proportions instead of being overwritten.
  if (current && current !== 'auto' && !/^auto\s/.test(current)) return false
  image.style.aspectRatio = `auto ${width} / ${height}`
  return true
}

const IMAGE_BOX_PROPERTIES = ['aspect-ratio', 'width', 'height', 'min-width', 'min-height'] as const
const failedImageBoxes = new WeakMap<
  HTMLImageElement,
  {
    attributes: Array<[string, string | null]>
    declarations: Array<[string, string, string]>
  }
>()

/** Release both browser-derived and explicit dimensions after a failed image. */
export function releaseFailedImageBox(image: HTMLImageElement) {
  if (!failedImageBoxes.has(image)) {
    failedImageBoxes.set(image, {
      attributes: ['width', 'height'].map((name) => [name, image.getAttribute(name)]),
      declarations: IMAGE_BOX_PROPERTIES.map((name) => [
        name,
        image.style.getPropertyValue(name),
        image.style.getPropertyPriority(name),
      ]),
    })
  }
  image.style.setProperty('aspect-ratio', 'auto', 'important')
  image.style.setProperty('width', 'auto', 'important')
  image.style.setProperty('height', 'auto', 'important')
  image.style.setProperty('min-width', '0', 'important')
  image.style.setProperty('min-height', '0', 'important')
  image.removeAttribute('width')
  image.removeAttribute('height')
}

/** Restore the sender's box only after an image actually arrives. */
export function restoreImageBox(image: HTMLImageElement) {
  const saved = failedImageBoxes.get(image)
  if (!saved) return
  for (const [name, value] of saved.attributes) {
    if (value === null) image.removeAttribute(name)
    else image.setAttribute(name, value)
  }
  for (const [name, value, priority] of saved.declarations) {
    if (value) image.style.setProperty(name, value, priority)
    else image.style.removeProperty(name)
  }
  failedImageBoxes.delete(image)
}

// A bare number of pixels: a percentage is not a length a box can be built from.
function plainLength(value: string | null): number | null {
  if (!value || !/^\d+(\.\d+)?$/.test(value.trim())) return null
  const parsed = Number(value)
  return parsed > 0 ? parsed : null
}

const pendingImageRetries = new WeakMap<HTMLImageElement, () => void>()

/** Retry failed pictures, including pending requests that fail after recovery. */
export function reloadFailedImages(doc: Document) {
  for (const image of doc.querySelectorAll<HTMLImageElement>('img[src]')) {
    if (image.complete && image.naturalWidth > 0) continue
    // A second recovery while this request is pending still needs only one retry.
    pendingImageRetries.get(image)?.()
    const retry = () => {
      cleanup()
      if (image.naturalWidth > 0) return
      const src = image.getAttribute('src')
      if (!src) return
      // An unchanged src is not a reason for the browser to load again.
      image.removeAttribute('src')
      image.setAttribute('src', src)
    }
    const cleanup = () => {
      image.removeEventListener('error', retry)
      image.removeEventListener('load', cleanup)
      pendingImageRetries.delete(image)
    }
    if (image.complete) retry()
    else {
      pendingImageRetries.set(image, cleanup)
      image.addEventListener('error', retry, { once: true })
      image.addEventListener('load', cleanup, { once: true })
    }
  }
}

const DEFAULT_MESSAGE_FRAME_FONT: MessageFrameFont = {
  family: null,
  zoom: 1,
}

const SIZING_STYLE = 'height: auto !important; min-height: 0 !important'

// An empty document, only so the frame's own tags can be built. A message from
// the core is never parsed here: WebKit parses it once, as the iframe's srcdoc.
function emptyDocument(): Document {
  return document.implementation?.createHTMLDocument?.('') ?? new DOMParser().parseFromString('', 'text/html')
}

// The shell the core wraps every sanitized message in (`inject_head` in
// meron-core's parse.rs). A document that opens with exactly this has its real
// head right here, whatever the message goes on to contain — so this is where
// the frame's CSP goes. Nothing is ever looked for inside the message: `<html>`
// or `<head>` can be spelled in its `<style>` text or an attribute value, and a
// CSP spliced in there is one the browser never reads.
const CORE_SHELL_OPEN = '<!doctype html><html><head>'
const CORE_BODY_OPEN = '</head><body>'

function escapeAttr(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;')
}

function endOfTag(html: string, start: number): number {
  let quote: string | null = null
  for (let i = start; i < html.length; i++) {
    const char = html[i]
    if (quote) {
      if (char === quote) quote = null
      continue
    }
    if (char === '"' || char === "'") {
      quote = char
      continue
    }
    if (char === '>') return i + 1
  }
  return -1
}

function rewriteCspMetas(html: string, rewrite: (csp: string) => string): string {
  const lower = html.toLowerCase()
  let out = ''
  let from = 0
  while (from < html.length) {
    const at = lower.indexOf('<meta', from)
    if (at < 0) return out + html.slice(from)
    const end = endOfTag(html, at)
    if (end < 0) return out + html.slice(from)
    out += html.slice(from, at) + rewriteCspMeta(html.slice(at, end), rewrite)
    from = end
  }
  return out
}

function rewriteCspMeta(tag: string, rewrite: (csp: string) => string): string {
  if (!/http-equiv\s*=\s*["']content-security-policy["']/i.test(tag)) return tag
  return tag.replace(/content\s*=\s*(["'])([\s\S]*?)\1/i, (_full, quote: string, content: string) => {
    return `content=${quote}${rewrite(content)}${quote}`
  })
}

// The frame's tags spliced into a document from the core, so the iframe's parse
// is the only one. Null for anything else, which is parsed and rebuilt instead:
// that includes a body that mentions the frame stylesheet marker at all, since
// only the parser can say whether the mention is an attribute or just text.
//
// No doctype is written back: the parsed path serialises the root element
// alone, and a message must lay out the same whichever path prepared it.
function assembleCoreDocument(
  html: string,
  csp: string,
  style: string,
  allowRemote: boolean,
  generation: string,
): string | null {
  if (!html.startsWith(CORE_SHELL_OPEN)) return null
  const rest = html.slice(CORE_SHELL_OPEN.length)
  const bodyAt = rest.indexOf(CORE_BODY_OPEN)
  if (bodyAt < 0) return null
  // The head is the core's own: its baked CSP is the one to bring in line with
  // the caller's decision. The frame's CSP goes ahead of it and blocks on its
  // own, so a rewrite that misses can only keep pictures hidden.
  const head = rewriteCspMetas(rest.slice(0, bodyAt), allowRemote ? allowRemoteInCsp : blockRemoteInCsp)
  const body = rest.slice(bodyAt + CORE_BODY_OPEN.length)
  if (body.toLowerCase().includes(FRAME_STYLE_MARKER)) return null
  return (
    `<html ${FRAME_GENERATION_MARKER}="${escapeAttr(generation)}" style="${SIZING_STYLE}">` +
    `<head>${csp}${head}${style}</head><body style="${SIZING_STYLE}">${body}`
  )
}

// Sanitises and styles an email's HTML body before it's rendered inside the
// bubble's sandboxed iframe. The iframe runs with `allow-scripts` (so our
// link-click handler fires), so we inject a strict CSP here to block the email's
// own JS, plus a base stylesheet that scopes typography and code-block styling.
//
// `font` carries the user's message typography: the frame can't reach the app's
// CSS vars or root font size, so the family is baked into the stylesheet and the
// text size arrives as a body `zoom` — a baked font-size would only move the
// bodies that don't set their own, which most HTML mail does (see lib/fonts).
export function prepareBubbleHtml(
  html: string,
  font: MessageFrameFont = DEFAULT_MESSAGE_FRAME_FONT,
  allowRemote = false,
  /** Stamped on the document so the frame can tell it from the one it replaced. */
  generation = '',
) {
  try {
    // A document from the core gets the frame's tags built on an empty document
    // and spliced in. Parsing it here and serialising it back made WebKit parse
    // every newsletter twice before the iframe could paint.
    const spliced = html.startsWith(CORE_SHELL_OPEN)
    const doc = spliced ? emptyDocument() : new DOMParser().parseFromString(html, 'text/html')

    // The iframe runs with `allow-scripts` (so our link-click handler fires),
    // so we must block the email's own JS here. `default-src 'none'` denies
    // scripts, `javascript:` URLs, and inline `on*` handlers; styles and fonts
    // stay permissive to match the bubble's prior no-CSP rendering.
    const csp = doc.createElement('meta')
    csp.setAttribute('http-equiv', 'Content-Security-Policy')
    // Media follows the caller: `*` while remote content is allowed, otherwise
    // the same same-origin/inline sources the sidecar bakes into a blocked body
    // (the frame runs with `allow-same-origin`, so `'self'` still resolves the
    // `/media` attachments). This meta is enforced alongside the baked one, so
    // it must block on its own — a body baked while its sender was allowed
    // carries a permissive policy until the thread is read again.
    const media = allowRemote
      ? 'img-src * data: blob:; media-src * data: blob:'
      : "img-src 'self' data:; media-src 'self' data: blob:"
    // `script-src`/`object-src`/`frame-src 'none'` are explicit for robustness
    // (they inherit from `default-src`); `base-uri` and `form-action` do NOT fall
    // back to `default-src`, so they're set to block a `<base>` hijack or a form
    // posting out of the frame.
    csp.setAttribute(
      'content',
      `default-src 'none'; script-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'; ${media}; style-src 'unsafe-inline'; font-src * data:;`,
    )

    const style = doc.createElement('style')
    ownStyleElement(style)
    const v = (name: string) => frameVar(frameVarPrefix(doc), name)
    style.textContent = `
      html {
        margin: 0 !important;
        padding: 0 !important;
        background: transparent !important;
      }
      html, body {
        margin: 0 !important;
        padding: 0 !important;
        color: var(${v('text')}, ${DEFAULT_BUBBLE_THEME.text});
        font: ${BUBBLE_HTML_BASE_PX}px/1.45 ${font.family ?? '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'};
        overflow-wrap: anywhere;
        overflow: hidden !important;
      }
      /* Zoom sits on the body, not the root: the frame self-sizes off the
         document's height, which only tracks the scaled content that way. */
      body {
        max-width: 100% !important;
        padding: var(${v('canvas-pad')}, 0) !important;
        border-radius: var(${v('canvas-radius')}, 0);${font.zoom === 1 ? '' : ` zoom: ${font.zoom};`}
      }
      *, *::before, *::after { box-sizing: border-box; }
      img, video {
        max-width: 100% !important;
        max-height: 320px !important;
        height: auto !important;
        object-fit: contain;
      }
      img { cursor: zoom-in; }
      table, pre { max-width: 100% !important; }
      /* \`anywhere\` lets a word break mid-character *and* drops the cell's
         min-content floor to one glyph, so a column with a small declared
         width (GitHub's 24px "Status" header) shreds its label vertically.
         Cells keep whole words; long URLs still compress because the anchor
         opts back in, and a table that can't fit gets a horizontal scroller. */
      td, th { overflow-wrap: break-word; }
      :is(td, th) a { overflow-wrap: anywhere; }
      pre {
        overflow-x: auto;
        white-space: pre;
        margin: 8px 0 !important;
        padding: 10px 42px 8px 12px !important;
        border: 1px solid var(${v('border')}, ${DEFAULT_BUBBLE_THEME.border});
        border-radius: 8px;
        background: var(${v('surface')}, ${DEFAULT_BUBBLE_THEME.surface});
        color: var(${v('surface-text')}, ${DEFAULT_BUBBLE_THEME.surfaceText});
        font: ${BUBBLE_CODE_BASE_PX}px/1.5 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
      }
      pre code {
        display: block;
        min-width: max-content;
        padding: 0 !important;
        background: transparent !important;
        color: inherit;
        font: inherit;
      }
      /* GitLab email diffs render each line-content cell as a <pre>. Forcing
         \`white-space: pre\` there gives the cell a max-content floor the table
         can't shrink below, so let those diff rows wrap instead. */
      table.code .diff-line-num {
        width: 35px !important;
        min-width: 35px;
        white-space: nowrap;
      }
      :is(td, th).line_content pre {
        margin: 0 !important;
        padding: 0 !important;
        border: 0 !important;
        border-radius: 0;
        overflow-wrap: anywhere;
        white-space: pre-wrap;
      }
      :is(td, th).line_content pre code { min-width: 0; }
      /* Escape hatch for content that still can't shrink (fixed-width layout
         tables): scroll it rather than clip it. */
      .meron-table-scroll {
        max-width: 100%;
        overflow-x: auto;
      }
      a { color: var(${v('link')}, ${DEFAULT_BUBBLE_THEME.link}); }
      .meron-code-block {
        position: relative;
        max-width: 100%;
      }
      .meron-copy-code {
        position: absolute;
        top: 6px;
        right: 6px;
        z-index: 2;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 28px;
        border: 1px solid var(${v('strong-border')}, ${DEFAULT_BUBBLE_THEME.strongBorder});
        border-radius: 6px;
        background: var(${v('raised')}, ${DEFAULT_BUBBLE_THEME.raised});
        color: var(${v('muted')}, ${DEFAULT_BUBBLE_THEME.muted});
        opacity: 0;
        cursor: pointer;
        box-shadow: 0 1px 2px rgba(15, 23, 42, 0.08);
        transition: opacity 0.12s ease, color 0.12s ease, background 0.12s ease;
      }
      .meron-code-block:hover .meron-copy-code,
      .meron-copy-code:focus-visible {
        opacity: 1;
      }
      .meron-copy-code:hover {
        background: var(${v('raised-hover')}, ${DEFAULT_BUBBLE_THEME.raisedHover});
        color: var(${v('text')}, ${DEFAULT_BUBBLE_THEME.text});
      }
      .meron-copy-code svg {
        width: 15px;
        height: 15px;
      }
      /* The folded quoted tail and its toggle (see quoteFold). */
      html.${QUOTE_FOLDED_CLASS} [data-meron-quote] {
        display: none !important;
      }
      .${QUOTE_TOGGLE_CLASS} {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 14px;
        margin: 8px 0;
        padding: 0;
        border: 1px solid var(${v('border')}, ${DEFAULT_BUBBLE_THEME.border});
        border-radius: 7px;
        background: var(${v('surface')}, ${DEFAULT_BUBBLE_THEME.surface});
        color: var(${v('muted')}, ${DEFAULT_BUBBLE_THEME.muted});
        font: 700 10px/1 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        letter-spacing: 1px;
        cursor: pointer;
      }
      .${QUOTE_TOGGLE_CLASS}::before { content: '•••'; }
      .${QUOTE_TOGGLE_CLASS}:hover,
      .${QUOTE_TOGGLE_CLASS}[aria-expanded="true"] {
        background: var(${v('raised-hover')}, ${DEFAULT_BUBBLE_THEME.raisedHover});
        color: var(${v('text')}, ${DEFAULT_BUBBLE_THEME.text});
      }
      /* Open: the chip stays lit, so it reads as the state, not a bare hover. */
      .${QUOTE_TOGGLE_CLASS}[aria-expanded="true"] {
        border-color: var(${v('muted')}, ${DEFAULT_BUBBLE_THEME.muted});
      }
      /* In-thread search hits, applied to the live document by BubbleHtmlFrame. */
      mark.meron-search-hit {
        border-radius: 3px;
        padding: 0 1px;
        background: rgba(253, 224, 71, 0.55);
        color: var(${v('highlight-text')}, ${DEFAULT_BUBBLE_THEME.highlightText});
      }
      mark.meron-search-hit.meron-search-hit-active {
        background: #fcd34d;
        color: #000000;
      }
      ${darkenedCss(v)}
      /* The root above is transparent, so a darkened body's background would
         propagate to the canvas, outside the body's filter, and stay light
         while the text inverted to light on it. A background of the root's
         own keeps it on the body: the surface the frame sits on, which is
         what the body's inverted white comes out as too. */
      html[${DARKENED_ATTR}] {
        background: var(${v('darkened-backdrop')}, rgba(0, 0, 0, 0.004)) !important;
      }
    `
    // A self-sizing frame needs its document boxes to follow the message.
    // Newsletter resets commonly force both boxes to height:100%, which pins
    // them to the placeholder viewport; with overflow hidden that also hides
    // the real scroll extent and leaves only the preheader. The override can't
    // live in our head stylesheet: a sender `<style>` inside `<body>` stays in
    // the body when parsed, and between two equally specific `!important`
    // rules the later one wins. Inline declarations outrank every stylesheet
    // rule of the same importance, so they win wherever the reset sits — and
    // unlike an appended `<style>` they leave `:last-child` and friends alone.
    if (spliced) {
      const assembled = assembleCoreDocument(html, csp.outerHTML, style.outerHTML, allowRemote, generation)
      if (assembled !== null) return assembled
    }

    const parsed = spliced ? new DOMParser().parseFromString(html, 'text/html') : doc
    parsed.head.insertBefore(parsed.importNode(csp, true), parsed.head.firstChild)
    // The body arrives with the sidecar's own CSP meta, baked from the policy in
    // force when it was read. Rewrite it in place so a decision made since then
    // takes effect without a refetch: loosen it once the user reveals this
    // message (or allows its sender), tighten it once that trust is withdrawn.
    if (allowRemote) allowRemoteContent(parsed)
    else blockRemoteContent(parsed)
    // Anything that arrived claiming to be a frame stylesheet isn't one — the
    // marker is also where this document's variable prefix travels.
    disownStyleElements(parsed)
    // Which document this is. A frame is wired as soon as its srcDoc changes,
    // while the one it replaces is still loaded, so the host checks this before
    // it treats what it finds as the document it just asked for. Sanitising
    // drops every `data-*` attribute, so sender markup can't carry one.
    parsed.documentElement.setAttribute(FRAME_GENERATION_MARKER, generation)
    parsed.head.appendChild(spliced ? parsed.importNode(style, true) : style)
    for (const el of [parsed.documentElement, parsed.body]) {
      el.style.setProperty('height', 'auto', 'important')
      el.style.setProperty('min-height', '0', 'important')
    }
    return parsed.documentElement.outerHTML
  } catch {
    return html
  }
}

/**
 * Paint a live bubble frame with the active theme.
 *
 * The decision needs the rendered document — which colors the message's own CSS
 * actually resolves to, and how much text they cover — so it runs here rather
 * than in `prepareBubbleHtml`, and writes the `--meron-*` properties its
 * stylesheet reads. A message that brings its own colors gets the canvas they
 * were authored for and the palette that goes with it; one that declares nothing
 * is painted in the bubble's colors, so it reads as part of the conversation.
 */
export function applyBubbleTheme(doc: Document, theme: BubbleTheme, preparedCanvas?: FrameCanvas) {
  const style = doc.documentElement.style
  const v = (name: string) => frameVar(frameVarPrefix(doc), name)
  // A light bubble only needs a canvas when the message declared a dark one:
  // otherwise its light design already sits on a light bubble.
  const needsCanvas = theme.appearance === 'dark' || colorTone(declaredCanvas(doc).background ?? '') === 'dark'
  const {
    background: canvas,
    text: declaredText,
    framePaints,
  } = preparedCanvas ?? frameCanvas(doc, theme.appearance, LIGHT_ON_DARK_TEXT, needsCanvas)
  const palette = canvas && colorTone(canvas) !== 'dark' ? DEFAULT_BUBBLE_THEME : theme
  const text = declaredText ?? palette.text
  const darkened = darkensCanvas(theme.appearance, theme.darkenStyled, canvas)
  // A darkened canvas blends into the bubble, so there is no card left to inset.
  const inset = framePaints && !darkened

  const vars: Array<[string, string | null]> = [
    // The canvas itself is painted on the body by `frameCanvas`; a canvas of
    // the frame's own also reads as a card, inset from the bubble's edges,
    // while the message's own design keeps its own box.
    [v('canvas-pad'), inset ? '10px' : null],
    [v('canvas-radius'), inset ? '8px' : null],
    [v('text'), text],
    [v('link'), palette.link],
    [v('surface'), palette.surface],
    [v('surface-text'), palette.surfaceText],
    [v('border'), palette.border],
    [v('strong-border'), palette.strongBorder],
    [v('raised'), palette.raised],
    [v('raised-hover'), palette.raisedHover],
    [v('muted'), palette.muted],
    [v('highlight-text'), palette.highlightText],
    [v('darkened-backdrop'), theme.darkenedBackdrop],
    [v('darkened-floor'), darkenedFloor(theme.darkenedBackdrop)],
  ]
  for (const [name, value] of vars) {
    if (value === null) style.removeProperty(name)
    else style.setProperty(name, value)
  }
  // The frame's root is transparent, but only while its color scheme matches
  // the app's: under a dark app a frame left on the default light scheme gets
  // an opaque white backdrop from the engine, and the dark palette above ends
  // up as light text on white. Important, so a sender's `:root` can't undo it.
  style.setProperty('color-scheme', theme.appearance, 'important')
  if (!preparedCanvas) setDarkened(doc, darkened, darkenedFloor(theme.darkenedBackdrop))
}

/** Theme large documents in small batches before the first visible measurement. */
export async function applyBubbleThemeAsync(doc: Document, theme: BubbleTheme, cancelled: () => boolean) {
  await new Promise<void>((resolve) => window.setTimeout(resolve, 0))
  if (cancelled()) return
  const needsCanvas = theme.appearance === 'dark' || colorTone(declaredCanvas(doc).background ?? '') === 'dark'
  const canvas = await frameCanvasAsync(doc, theme.appearance, LIGHT_ON_DARK_TEXT, needsCanvas, cancelled)
  if (!canvas || cancelled()) return
  applyBubbleTheme(doc, theme, canvas)
  await setDarkenedAsync(
    doc,
    darkensCanvas(theme.appearance, theme.darkenStyled, canvas.background),
    darkenedFloor(theme.darkenedBackdrop),
    cancelled,
  )
}
