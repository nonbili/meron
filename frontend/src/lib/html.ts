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
      const alt = node.getAttribute('alt')?.trim()
      if (alt) out += alt
      return
    }

    for (const child of Array.from(node.childNodes)) walk(child)
    if (paragraphTags.has(node.tagName)) appendNewlines(2)
    if (lineBreakTags.has(node.tagName)) appendNewlines(1)
  }

  for (const child of Array.from(div.childNodes)) walk(child)
  return out.replace(/\n+$/g, '').trimStart()
}
