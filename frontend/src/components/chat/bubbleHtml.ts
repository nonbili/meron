import { BUBBLE_CODE_BASE_PX, BUBBLE_HTML_BASE_PX, type MessageFrameFont } from '../../lib/fonts'
import { allowRemoteContent, blockRemoteContent } from './remoteContentCsp'

const DEFAULT_MESSAGE_FRAME_FONT: MessageFrameFont = {
  family: null,
  zoom: 1,
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
) {
  try {
    const parser = new DOMParser()
    const doc = parser.parseFromString(html, 'text/html')

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
    doc.head.insertBefore(csp, doc.head.firstChild)

    // The body arrives with the sidecar's own CSP meta, baked from the policy in
    // force when it was read. Rewrite it in place so a decision made since then
    // takes effect without a refetch: loosen it once the user reveals this
    // message (or allows its sender), tighten it once that trust is withdrawn.
    if (allowRemote) allowRemoteContent(doc)
    else blockRemoteContent(doc)

    const style = doc.createElement('style')
    style.textContent = `
      html, body {
        margin: 0 !important;
        padding: 0 !important;
        background: transparent !important;
        color: #0f172a;
        font: ${BUBBLE_HTML_BASE_PX}px/1.45 ${font.family ?? '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'};
        overflow-wrap: anywhere;
        overflow: hidden !important;
      }
      /* Zoom sits on the body, not the root: the frame self-sizes off the
         document's height, which only tracks the scaled content that way. */
      body { max-width: 100% !important;${font.zoom === 1 ? '' : ` zoom: ${font.zoom};`} }
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
        border: 1px solid #e2e8f0;
        border-radius: 8px;
        background: #f1f5f9;
        color: #1e293b;
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
      a { color: #4f46e5; }
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
      /* In-thread search hits, applied to the live document by BubbleHtmlFrame. */
      mark.meron-search-hit {
        border-radius: 3px;
        padding: 0 1px;
        background: rgba(253, 224, 71, 0.55);
        color: inherit;
      }
      mark.meron-search-hit.meron-search-hit-active {
        background: #fcd34d;
        color: #000000;
      }
    `
    doc.head.appendChild(style)

    // A self-sizing frame needs its document boxes to follow the message.
    // Newsletter resets commonly force both boxes to height:100%, which pins
    // them to the placeholder viewport; with overflow hidden that also hides
    // the real scroll extent and leaves only the preheader. The override can't
    // live in our head stylesheet: a sender `<style>` inside `<body>` stays in
    // the body when parsed, and between two equally specific `!important`
    // rules the later one wins. Inline declarations outrank every stylesheet
    // rule of the same importance, so they win wherever the reset sits — and
    // unlike an appended `<style>` they leave `:last-child` and friends alone.
    for (const el of [doc.documentElement, doc.body]) {
      el.style.setProperty('height', 'auto', 'important')
      el.style.setProperty('min-height', '0', 'important')
    }
    return doc.documentElement.outerHTML
  } catch {
    return html
  }
}
