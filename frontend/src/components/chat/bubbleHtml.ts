// Sanitises and styles an email's HTML body before it's rendered inside the
// bubble's sandboxed iframe. The iframe runs with `allow-scripts` (so our
// link-click handler fires), so we inject a strict CSP here to block the email's
// own JS, plus a base stylesheet that scopes typography and code-block styling.
export function prepareBubbleHtml(html: string) {
  try {
    const parser = new DOMParser()
    const doc = parser.parseFromString(html, 'text/html')

    // The iframe runs with `allow-scripts` (so our link-click handler fires),
    // so we must block the email's own JS here. `default-src 'none'` denies
    // scripts, `javascript:` URLs, and inline `on*` handlers; images/styles/
    // fonts stay permissive to match the bubble's prior no-CSP rendering.
    const csp = doc.createElement('meta')
    csp.setAttribute('http-equiv', 'Content-Security-Policy')
    // `script-src`/`object-src`/`frame-src 'none'` are explicit for robustness
    // (they inherit from `default-src`); `base-uri` and `form-action` do NOT fall
    // back to `default-src`, so they're set to block a `<base>` hijack or a form
    // posting out of the frame.
    csp.setAttribute(
      'content',
      "default-src 'none'; script-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'; img-src * data: blob:; media-src * data: blob:; style-src 'unsafe-inline'; font-src * data:;",
    )
    doc.head.insertBefore(csp, doc.head.firstChild)

    const style = doc.createElement('style')
    style.textContent = `
      html, body {
        margin: 0 !important;
        padding: 0 !important;
        background: transparent !important;
        color: #0f172a;
        font: 14px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        overflow-wrap: anywhere;
        overflow: hidden !important;
      }
      body { max-width: 100% !important; }
      *, *::before, *::after { box-sizing: border-box; }
      img, video {
        max-width: 100% !important;
        max-height: 320px !important;
        height: auto !important;
        object-fit: contain;
      }
      img { cursor: zoom-in; }
      table, pre { max-width: 100% !important; }
      pre {
        overflow-x: auto;
        white-space: pre;
        margin: 8px 0 !important;
        padding: 10px 42px 8px 12px !important;
        border: 1px solid #e2e8f0;
        border-radius: 8px;
        background: #f1f5f9;
        color: #1e293b;
        font: 12.5px/1.5 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
      }
      pre code {
        display: block;
        min-width: max-content;
        padding: 0 !important;
        background: transparent !important;
        color: inherit;
        font: inherit;
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
    `
    doc.head.appendChild(style)
    return doc.documentElement.outerHTML
  } catch {
    return html
  }
}
