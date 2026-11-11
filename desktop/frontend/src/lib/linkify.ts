// Bare URLs in outgoing mail become links, so they are clickable in whichever
// client renders the HTML part. The same rules as the core's reader-side pass
// (meron-core/src/linkify.rs): `http(s)://` and `www.` only, never a bare domain.

const BARE_URL = /(^|[^\p{L}\p{N}./@_-])((?:https?:\/\/|www\.)[^\s<>"'`]+)/giu

export type BareUrl = { start: number; end: number; text: string; href: string }

const BRACKETS: Record<string, string> = { ')': '(', ']': '[', '}': '{' }

/** The URL without the sentence punctuation or unmatched closing bracket after
 *  it. Bracket balances are counted once and kept up as the tail is trimmed, so
 *  a long run of `)` stays linear. */
function trimUrl(url: string): string {
  const balance: Record<string, number> = { '(': 0, '[': 0, '{': 0 }
  for (const c of url) {
    if (c in balance) balance[c]++
    else if (c in BRACKETS) balance[BRACKETS[c]]--
  }
  let end = url.length
  while (end > 0) {
    const last = url[end - 1]
    if ('.,;:!?*'.includes(last)) end--
    else if (last in BRACKETS && balance[BRACKETS[last]] < 0) {
      balance[BRACKETS[last]]++
      end--
    } else break
  }
  return url.slice(0, end)
}

/** The bare URLs in a run of plain text, in order. */
export function findBareUrls(text: string): BareUrl[] {
  const found: BareUrl[] = []
  for (const match of text.matchAll(BARE_URL)) {
    const start = match.index + match[1].length
    const url = trimUrl(match[2])
    const www = /^www\./i.test(url)
    const host = url.replace(/^(https?:\/\/|www\.)/i, '')
    if (!host || (www && !/^[^./?#]+\.[^/?#]/.test(host))) continue
    found.push({ start, end: start + url.length, text: url, href: www ? `http://${url}` : url })
  }
  return found
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

/** Escape a line of plain text as HTML, wrapping its bare URLs in links. */
export function linkifyLine(line: string): string {
  let html = ''
  let copied = 0
  for (const url of findBareUrls(line)) {
    html += `${escapeHtml(line.slice(copied, url.start))}<a href="${escapeHtml(url.href)}">${escapeHtml(url.text)}</a>`
    copied = url.end
  }
  return html + escapeHtml(line.slice(copied))
}

/** Wrap the bare URLs in the text of `root` in links, leaving text already in a
 *  link, or in code, alone. */
export function linkifyTextNodes(root: Element) {
  const doc = root.ownerDocument
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  const texts: Text[] = []
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (!node.parentElement?.closest('a, code, pre, style, script')) texts.push(node as Text)
  }
  for (const node of texts) {
    const value = node.data
    const urls = findBareUrls(value)
    if (urls.length === 0) continue
    const fragment = doc.createDocumentFragment()
    let copied = 0
    for (const url of urls) {
      if (url.start > copied) fragment.append(value.slice(copied, url.start))
      const link = doc.createElement('a')
      link.setAttribute('href', url.href)
      link.textContent = url.text
      fragment.append(link)
      copied = url.end
    }
    if (copied < value.length) fragment.append(value.slice(copied))
    node.replaceWith(fragment)
  }
}
