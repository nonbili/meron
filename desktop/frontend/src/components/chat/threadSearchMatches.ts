// Occurrence counting for the in-thread search. The search bar counts matches,
// not messages, so it has to know how many <mark>s each message will produce —
// including HTML bodies, whose marks are placed inside a frame that may not even
// be mounted yet. Both counts are computed the way their renderer highlights:
// plain bodies chunk by chunk (MessageBubbleBody), HTML bodies over the
// concatenated text nodes (frameSearchHighlight). Both go through matchRanges,
// the matcher the two renderers place their marks with, so bar and body agree
// on what counts as a match — case folding included.

import type { Message } from '../../types'
import { collectTextNodes, matchRanges } from './frameSearchHighlight'
import { bodyContentKey, plainHighlightTexts } from './messageHelpers'

// Parsing every HTML body on every keystroke is the expensive part; the text
// nodes don't change while the thread is open, so keep them keyed by message.
const HTML_TEXT_CACHE_LIMIT = 200
const htmlTextCache = new Map<string, string[]>()

function htmlBodyTexts(messageId: string, html: string): string[] {
  // Hashed, not just measured: a body refreshed in the background can come back
  // the same length and different, and a stale cache would then miscount it.
  const key = `${messageId}:${bodyContentKey(html)}`
  const cached = htmlTextCache.get(key)
  if (cached) return cached

  const doc = new DOMParser().parseFromString(html, 'text/html')
  const texts = collectTextNodes(doc).map((node) => node.nodeValue ?? '')
  if (htmlTextCache.size >= HTML_TEXT_CACHE_LIMIT) {
    const oldest = htmlTextCache.keys().next().value
    if (oldest !== undefined) htmlTextCache.delete(oldest)
  }
  htmlTextCache.set(key, texts)
  return texts
}

/** How many matches this message highlights for `query` — 0 when none of its
 *  body does, even if the query matches its subject or sender. */
export function messageMatchCount(message: Message, query: string, useHtmlBody: boolean): number {
  const needle = query.trim()
  if (!needle) return 0

  if (useHtmlBody && message.body_html && typeof DOMParser !== 'undefined') {
    return matchRanges(htmlBodyTexts(message.id, message.body_html), needle).hits
  }

  // One text at a time, because the plain renderer marks each chunk on its own:
  // a match may not run from one chunk into the next there either.
  let count = 0
  for (const text of plainHighlightTexts(message.body ?? '')) count += matchRanges([text], needle).hits
  return count
}
