/** Rewrite `cid:<inlineId>` image refs to `data:` URLs from the matching
 * composer attachments. Used for locally-rendered copies of an outgoing
 * message (the optimistic sent bubble/reader): they never went through the
 * backend's cid-to-`/media/<key>` rewrite, so without this the reader has no
 * way to resolve the refs and shows broken images. */
export function resolveInlineCids(
  html: string,
  attachments: Array<{ inlineId?: string; mime: string; data: string }>,
): string {
  let out = html
  for (const attachment of attachments) {
    if (!attachment.inlineId) continue
    out = out.split(`cid:${attachment.inlineId}`).join(`data:${attachment.mime};base64,${attachment.data}`)
  }
  return out
}

/** Derive a plaintext fallback from rich-text HTML while preserving visible line breaks. */
export function htmlToText(html: string): string {
  const div = document.createElement('div')
  div.innerHTML = html

  let out = ''
  const paragraphTags = new Set([
    'ADDRESS',
    'ARTICLE',
    'ASIDE',
    'BLOCKQUOTE',
    'DIV',
    'FIGCAPTION',
    'FIGURE',
    'FOOTER',
    'H1',
    'H2',
    'H3',
    'H4',
    'H5',
    'H6',
    'HEADER',
    'HR',
    'LI',
    'MAIN',
    'NAV',
    'P',
    'PRE',
    'SECTION',
    'TABLE',
  ])
  const lineBreakTags = new Set(['LI', 'OL', 'TR', 'UL'])

  const appendNewlines = (count: number) => {
    if (!out) return
    const trailing = out.match(/\n*$/)?.[0].length ?? 0
    if (trailing < count) out += '\n'.repeat(count - trailing)
  }

  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      out += node.textContent ?? ''
      return
    }
    if (!(node instanceof HTMLElement)) return

    if (node.tagName === 'BR') {
      out += '\n'
      return
    }
    if (node.tagName === 'IMG') {
      // Composer inline images are block-level, so put the alt (the image's
      // filename) on its own paragraph rather than gluing it to the text
      // before/after the image.
      const alt = node.getAttribute('alt')?.trim()
      if (alt) {
        appendNewlines(2)
        out += alt
        appendNewlines(2)
      }
      return
    }

    for (const child of Array.from(node.childNodes)) walk(child)
    if (paragraphTags.has(node.tagName)) appendNewlines(2)
    if (lineBreakTags.has(node.tagName)) appendNewlines(1)
  }

  for (const child of Array.from(div.childNodes)) walk(child)
  return out.replace(/\n+$/g, '').trimStart()
}
