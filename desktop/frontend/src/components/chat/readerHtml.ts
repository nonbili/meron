import { type MessageFrameFont } from '../../lib/fonts'
import {
  DEFAULT_READER_THEME,
  LIGHT_ON_DARK_TEXT,
  disownStyleElements,
  frameCanvas,
  frameVar,
  frameVarPrefix,
  isOwnStyleElement,
  ownStyleElement,
  type ReaderTheme,
} from './frameTheme'

const READER_STYLE_ID = 'meron-reader-style'
const READER_FONT_STYLE_ID = 'meron-reader-font'

export const DEFAULT_READER_FONT: MessageFrameFont = {
  family: null,
  zoom: 1,
}

const readerCss = (v: (name: string) => string) => `
  html {
    background: var(${v('page-bg')}, #f8fafc);
  }
  body {
    box-sizing: border-box;
    max-width: 760px;
    margin: 0 auto !important;
    padding: 24px 20px 40px !important;
    color: var(${v('text')}, #0f172a);
    overflow-wrap: anywhere;
  }
  *, *::before, *::after {
    box-sizing: border-box;
  }
  img, video {
    display: inline-block;
    width: auto !important;
    max-width: 100% !important;
    height: auto !important;
    max-height: 48vh;
    object-fit: contain;
  }
  img {
    cursor: zoom-in;
  }
  table, pre {
    max-width: 100% !important;
  }
  /* \`anywhere\` on the body also drops a cell's min-content floor to a single
     glyph, so a column with a small declared width breaks its label one letter
     per line. Cells keep whole words; anchors opt back in so long URLs still
     let the table shrink. */
  td, th {
    overflow-wrap: break-word;
  }
  :is(td, th) a {
    overflow-wrap: anywhere;
  }
  pre {
    display: block;
    overflow-x: auto;
    margin: 16px 0 !important;
    padding: 14px 48px 10px 16px !important;
    border: 1px solid var(${v('border')}, #e2e8f0);
    border-radius: 8px;
    background: var(${v('surface')}, #f1f5f9);
    color: var(${v('text')}, #1e293b);
    font: 13px/1.55 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    white-space: pre;
    overflow-y: hidden;
    scrollbar-gutter: stable;
  }
  pre::-webkit-scrollbar {
    height: 10px;
  }
  pre::-webkit-scrollbar-track {
    background: transparent;
  }
  pre::-webkit-scrollbar-thumb {
    border: 3px solid transparent;
    background-clip: padding-box;
    border-radius: 999px;
    background-color: var(${v('muted')}, #cbd5e1);
  }
  pre::-webkit-scrollbar-thumb:hover {
    background-color: var(${v('muted-strong')}, #94a3b8);
    border: 2px solid transparent;
  }
  pre code {
    display: block;
    min-width: max-content;
    padding: 0 !important;
    background: transparent !important;
    color: inherit;
    font: inherit;
  }
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
  :is(td, th).line_content pre code {
    min-width: 0;
  }
  code {
    border-radius: 4px;
    background: var(${v('surface')}, #eef2f7);
    padding: 0.12em 0.32em;
    font: 0.92em ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
  }
  .meron-code-block {
    position: relative;
    max-width: 100%;
  }
  .meron-copy-code {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 2;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(${v('border')}, #cbd5e1);
    border-radius: 6px;
    background: var(${v('raised')}, rgba(255, 255, 255, 0.92));
    color: var(${v('muted-strong')}, #64748b);
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
    background: var(${v('raised-hover')}, #ffffff);
    color: var(${v('text')}, #0f172a);
  }
  .meron-copy-code svg {
    width: 15px;
    height: 15px;
  }
`

/**
 * Paint the frame with the active theme. The gutter always follows it; the
 * message body only goes dark when it brings no colors of its own, and one that
 * does is given the canvas those colors were authored for (see
 * `frameCanvasBackground`) — light, or its own declared page color.
 */
export function applyReaderTheme(doc: Document, theme: ReaderTheme) {
  // Anything that arrived claiming to be a frame stylesheet isn't one: the
  // reader's own are the ones it injected itself.
  disownStyleElements(doc)
  const style = doc.documentElement.style
  const v = (name: string) => frameVar(frameVarPrefix(doc), name)
  // The canvas itself is painted on the body by `frameCanvas`; what is left for
  // the frame here is the gutter around it and the palette on top of it.
  const { background: canvas, text: canvasText } = frameCanvas(doc, theme.appearance, LIGHT_ON_DARK_TEXT)

  // In a light appearance the message's own page color simply takes over the
  // whole frame, the way the sender meant it — a themed gutter would only put a
  // seam around the body. In a dark one the gutter stays dark and the canvas is
  // confined to the reader column, so the message reads as a card on it.
  style.setProperty(v('page-bg'), canvas && theme.appearance === 'light' ? canvas : theme.pageBg)

  const vars: Array<[string, string]> = [
    [v('text'), theme.text],
    [v('surface'), theme.surface],
    [v('raised'), theme.raised],
    [v('raised-hover'), theme.raisedHover],
    [v('border'), theme.border],
    [v('muted'), theme.muted],
    [v('muted-strong'), theme.mutedStrong],
  ]
  // A message on a canvas of its own keeps the light defaults baked into
  // READER_CSS: its own colors were authored against them.
  if (theme.appearance === 'dark' && !canvas) {
    for (const [name, value] of vars) style.setProperty(name, value)
  } else {
    for (const [name] of vars) style.removeProperty(name)
  }

  // A canvas the message declared but never wrote text colors for still needs a
  // readable foreground — a restored black page with the light-mode default on
  // it is the dark-on-dark case this whole path exists to avoid.
  if (canvasText) style.setProperty(v('text'), canvasText)
}

/**
 * The reader's own element carrying `id`, if it is really ours.
 *
 * A sender can ship `<style id="meron-reader-style">`: the core strips `meron-*`
 * hooks, but a body that predates that (or arrives from a feed) can still carry
 * one, and looking the id up blindly would let it stand in for the reader's own
 * stylesheet. A squatter loses the id and the reader injects its own.
 */
function ownStyle(doc: Document, id: string): HTMLElement | null {
  const found = doc.getElementById(id)
  if (!found) return null
  if (isOwnStyleElement(found)) return found
  found.removeAttribute('id')
  return null
}

// Neutralise likely tracking pixels in the stored email HTML: tiny/hidden images
// and known tracker URL patterns are swapped for a transparent 1x1 GIF.
export function stripTrackingPixels(html: string): string {
  try {
    const parser = new DOMParser()
    const doc = parser.parseFromString(html, 'text/html')
    const images = doc.querySelectorAll('img')

    images.forEach((img) => {
      const src = img.getAttribute('src') || ''
      const width = img.getAttribute('width') || ''
      const height = img.getAttribute('height') || ''
      const style = img.getAttribute('style') || ''

      // 1. Size attributes (0, 1, 2)
      const isTinyAttr =
        (width === '1' || width === '0' || width === '2') && (height === '1' || height === '0' || height === '2')

      // 2. Hidden CSS
      const lowerStyle = style.toLowerCase()
      const isHiddenStyle =
        lowerStyle.includes('display:none') ||
        lowerStyle.includes('display: none') ||
        lowerStyle.includes('visibility:hidden') ||
        lowerStyle.includes('visibility: hidden')

      // 3. Micro-sized CSS
      const hasTinyW =
        lowerStyle.includes('width:0px') ||
        lowerStyle.includes('width: 0px') ||
        lowerStyle.includes('width:1px') ||
        lowerStyle.includes('width: 1px') ||
        lowerStyle.includes('width:2px') ||
        lowerStyle.includes('width: 2px')
      const hasTinyH =
        lowerStyle.includes('height:0px') ||
        lowerStyle.includes('height: 0px') ||
        lowerStyle.includes('height:1px') ||
        lowerStyle.includes('height: 1px') ||
        lowerStyle.includes('height:2px') ||
        lowerStyle.includes('height: 2px')
      const isTinyStyle = hasTinyW && hasTinyH

      // 4. URL pattern matches
      const lowerSrc = src.toLowerCase()
      const isTrackingUrl =
        lowerSrc.includes('/open/') ||
        lowerSrc.includes('/track') ||
        lowerSrc.includes('/pixel') ||
        lowerSrc.includes('pixel.gif') ||
        lowerSrc.includes('cleardot.gif') ||
        lowerSrc.includes('spacer.gif') ||
        lowerSrc.includes('/wf/open') ||
        lowerSrc.includes('/open.php') ||
        lowerSrc.includes('utm_') ||
        lowerSrc.includes('bounce')

      if (isTinyAttr || isHiddenStyle || isTinyStyle || isTrackingUrl) {
        // Replace tracker src with safe transparent 1x1 base64 GIF
        img.setAttribute('src', 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7')
        img.removeAttribute('srcset')
        img.removeAttribute('width')
        img.removeAttribute('height')
      }
    })

    return doc.documentElement.outerHTML
  } catch (e) {
    console.error('Error sanitizing trackers in HtmlMessageView', e)
    return html
  }
}

/**
 * Paint a reader frame with the user's message typography. Declarations are
 * only emitted when a preference is actually set, so by default the email's own
 * fonts (and the browser default) render exactly as before. No `!important`
 * either: an email that names its own fonts keeps them. The text size arrives as
 * a body `zoom` so it also moves the emails that size their own text.
 */
export function applyReaderFont(doc: Document, font: MessageFrameFont) {
  const rules: string[] = []
  if (font.family) rules.push(`font-family: ${font.family};`)
  if (font.zoom !== 1) rules.push(`zoom: ${font.zoom};`)

  let style = ownStyle(doc, READER_FONT_STYLE_ID)
  if (rules.length === 0) {
    style?.remove()
    return
  }
  if (!style) {
    style = doc.createElement('style')
    style.id = READER_FONT_STYLE_ID
    ownStyleElement(style)
    ;(doc.head ?? doc.documentElement).appendChild(style)
  }
  style.textContent = `body { ${rules.join(' ')} }`
}

// Apply the reader-width layout to a rendered frame document: inject the reader
// stylesheet, wrap standalone <pre> elements with copy-code buttons, and force
// media to fit. Runs in the frontend so already-stored feed HTML gets the same
// treatment.
export function applyReaderLayout(
  doc: Document,
  font: MessageFrameFont = DEFAULT_READER_FONT,
  theme: ReaderTheme = DEFAULT_READER_THEME,
) {
  applyReaderFont(doc, font)
  applyReaderTheme(doc, theme)

  if (!ownStyle(doc, READER_STYLE_ID)) {
    const prefix = frameVarPrefix(doc)
    const style = doc.createElement('style')
    style.id = READER_STYLE_ID
    style.textContent = readerCss((name) => frameVar(prefix, name))
    ownStyleElement(style)
    ;(doc.head ?? doc.documentElement).appendChild(style)
  }

  for (const pre of doc.querySelectorAll<HTMLPreElement>('pre')) {
    if (pre.closest('.meron-code-block')) continue
    if (pre.closest('td.line_content, th.line_content')) continue

    const wrapper = doc.createElement('div')
    wrapper.className = 'meron-code-block'
    pre.parentNode?.insertBefore(wrapper, pre)
    wrapper.appendChild(pre)

    const button = doc.createElement('button')
    button.type = 'button'
    button.className = 'meron-copy-code'
    button.title = 'Copy code'
    button.setAttribute('aria-label', 'Copy code')
    button.innerHTML = `
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
        stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect>
        <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
      </svg>
    `
    button.addEventListener('click', (event) => {
      event.preventDefault()
      event.stopPropagation()
      navigator.clipboard?.writeText(pre.innerText).catch(() => undefined)
    })
    wrapper.appendChild(button)
  }

  for (const media of doc.querySelectorAll<HTMLImageElement | HTMLVideoElement>('img,video')) {
    media.removeAttribute('width')
    media.removeAttribute('height')
    media.style.setProperty('width', 'auto', 'important')
    media.style.setProperty('max-width', '100%', 'important')
    media.style.setProperty('height', 'auto', 'important')
    media.style.setProperty('max-height', '48vh', 'important')
  }
}
