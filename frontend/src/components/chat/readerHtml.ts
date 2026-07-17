const READER_STYLE_ID = 'meron-reader-style'

const READER_CSS = `
  html {
    background: #f8fafc;
  }
  body {
    box-sizing: border-box;
    max-width: 760px;
    margin: 0 auto !important;
    padding: 24px 20px 40px !important;
    color: #0f172a;
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
  pre {
    display: block;
    overflow-x: auto;
    margin: 16px 0 !important;
    padding: 14px 48px 10px 16px !important;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    background: #f1f5f9;
    color: #1e293b;
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
    border: 2px solid transparent;
    background-clip: padding-box;
    border-radius: 999px;
    background: #cbd5e1;
  }
  pre::-webkit-scrollbar-thumb:hover {
    background: #94a3b8;
    border: 0px solid transparent;
  }
  pre code {
    display: block;
    min-width: max-content;
    padding: 0 !important;
    background: transparent !important;
    color: inherit;
    font: inherit;
  }
  code {
    border-radius: 4px;
    background: #eef2f7;
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
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.92);
    color: #64748b;
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
    background: #ffffff;
    color: #0f172a;
  }
  .meron-copy-code svg {
    width: 15px;
    height: 15px;
  }
`

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

// Apply the reader-width layout to a rendered frame document: inject the reader
// stylesheet, wrap each <pre> with a copy-code button, and force media to fit.
// Runs in the frontend so already-stored feed HTML gets the same treatment.
export function applyReaderLayout(doc: Document) {
  if (!doc.getElementById(READER_STYLE_ID)) {
    const style = doc.createElement('style')
    style.id = READER_STYLE_ID
    style.textContent = READER_CSS
    ;(doc.head ?? doc.documentElement).appendChild(style)
  }

  for (const pre of doc.querySelectorAll<HTMLPreElement>('pre')) {
    if (pre.closest('.meron-code-block')) continue

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
