// Folding my own signature away in the conversation, with the quote fold (see
// quoteFold): a bubble sits under every other message I sent, each ending in the
// same lines. Only my own messages, and only the sending account's current
// signature, matched exactly as the message's last block: other people's
// signatures have no reliable edge, and a guess would hide what they wrote.
// Nothing is removed; the fold's toggle shows it again.

import { useValue } from '@legendapp/state/react'
import type { Message } from '../../types'
import { accounts$ } from '../../states/accounts'
import { settings$ } from '../../states/settings'
import { resolveSignature, signatureToText, textWithoutTrailingSignature } from '../../lib/signature'
import { HTML_QUOTE_SELECTOR } from './quoteFold'

/**
 * My signature as plain text when `message` is one I sent, else ''. That
 * includes the bubble shown while it is being sent: only its Sent copy is
 * marked outgoing, but a local send status is set on my own sends alone, and
 * without it the signature would show until the Sent copy replaced the bubble.
 */
export function ownSignatureText(message: Message): string {
  if (!message.outgoing && !message.send_status) return ''
  const account = accounts$.get().find((acc) => acc.id === message.account_id)
  if (!account) return ''
  return signatureToText(resolveSignature(account, settings$.signature.get()))
}

/**
 * `ownSignatureText` for a component. Every bubble in a thread reads it, so it
 * renders again only when this text changes, not on every change to an account.
 */
export function useOwnSignatureText(message: Message): string {
  return useValue(() => ownSignatureText(message))
}

/**
 * Where a plain body's fold starts once my signature folds with it: at the
 * signature when it is the last block above the quote (or of the message),
 * else where the quote starts. A message that is nothing but the signature
 * keeps it in view.
 */
export function foldStartWithOwnSignature(
  body: string,
  quoteStart: number | null | undefined,
  signatureText: string,
): number | null | undefined {
  if (!signatureText) return quoteStart
  const hasQuote = !!quoteStart && quoteStart > 0 && quoteStart < body.length
  const reply = hasQuote ? body.slice(0, quoteStart!) : body
  const text = textWithoutTrailingSignature(reply, signatureText)
  if (text === null || !text.trim()) return quoteStart
  return text.length
}

const BLOCK_TAGS = new Set(['P', 'DIV', 'LI', 'TR', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'BLOCKQUOTE', 'PRE'])

/** A node's text the way `signatureToText` reads signature HTML: blocks and
 *  `<br>` end lines. */
function rawText(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent ?? ''
  if (node.nodeType !== Node.ELEMENT_NODE) return ''
  const element = node as Element
  if (element.tagName === 'BR' || element.tagName === 'HR') return '\n'
  const inner = Array.from(element.childNodes).map(rawText).join('')
  return BLOCK_TAGS.has(element.tagName) ? `${inner}\n` : inner
}

function linesOf(nodes: Node[]): string {
  return nodes
    .map(rawText)
    .join('')
    .replace(/ /g, ' ')
    .split('\n')
    .map((line) => line.trim())
    .join('\n')
    .replace(/^\n+|\n+$/g, '')
}

const isBlank = (node: Node) =>
  !linesOf([node]) && !(node.nodeType === Node.ELEMENT_NODE && (node as Element).querySelector('img'))

/**
 * Mark the blocks of my signature in a bubble's HTML document so they fold
 * with the quote (`data-meron-quote`, which the frame folds and the search
 * unfolds). Only when they end the message, or sit right above its marked
 * quote, and something else is left above them. Returns whether it marked.
 */
export function markOwnSignature(doc: Document, signatureText: string): boolean {
  if (!signatureText || !doc.body) return false
  // A client may wrap the whole message in one element: look inside it.
  let container: Element = doc.body
  for (;;) {
    const children = Array.from(container.childNodes).filter((node) => !isBlank(node))
    const only = children.length === 1 ? children[0] : null
    if (!only || only.nodeType !== Node.ELEMENT_NODE || (only as Element).matches(HTML_QUOTE_SELECTOR)) break
    if (!(only as Element).children.length) break
    container = only as Element
  }

  const nodes = Array.from(container.childNodes)
  const quoteAt = nodes.findIndex(
    (node) =>
      node.nodeType === Node.ELEMENT_NODE &&
      ((node as Element).matches(HTML_QUOTE_SELECTOR) || !!(node as Element).querySelector(HTML_QUOTE_SELECTOR)),
  )
  const above = quoteAt >= 0 ? nodes.slice(0, quoteAt) : nodes

  // Walk up from the end, past blank lines, until the collected blocks read as
  // the signature (a match) or more than it (none).
  let end = above.length
  while (end > 0 && isBlank(above[end - 1])) end -= 1
  for (let start = end - 1; start >= 0; start -= 1) {
    const text = linesOf(above.slice(start, end))
    if (text.length > signatureText.length) return false
    if (text !== signatureText) continue
    // The signature alone is the message: nothing to fold it away from.
    if (!above.slice(0, start).some((node) => !isBlank(node))) return false
    // The blank lines around it go too, or they stay behind as space.
    let from = start
    while (from > 0 && isBlank(above[from - 1])) from -= 1
    const folded = above.slice(from)
    for (const node of folded) {
      if (node.nodeType === Node.ELEMENT_NODE) {
        ;(node as Element).setAttribute('data-meron-quote', '')
      } else if (node.textContent?.trim()) {
        // Loose text can't carry the attribute: wrap it so it folds too.
        const span = doc.createElement('span')
        span.setAttribute('data-meron-quote', '')
        node.parentNode?.replaceChild(span, node)
        span.appendChild(node)
      }
    }
    return true
  }
  return false
}
