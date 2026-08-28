import type { Account, AccountSignature } from '../types'

// Signatures are stored as HTML: one app-wide signature in settings, plus an
// optional per-account override (see states/settings.ts and types.ts). They are
// inserted into the draft body when a composer opens — the user can edit or
// delete the text like any other content — rather than being stapled on at send
// time, so what the composer shows is what goes out.
//
// The thread's quick reply is seeded the same way. It renders as a chat bubble,
// but what it sends is an ordinary mail that its recipient reads in an ordinary
// client, so leaving it unsigned reads as inconsistency rather than brevity.
// Its tracking is simpler — see compose$.quickReplySignature.

/** Whether signature HTML carries any visible content (text or an image). */
export function isBlankSignature(html: string): boolean {
  if (!html.trim()) return true
  const stripped = html
    .replace(/<(img|hr|br)\b[^>]*>/gi, 'x')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/gi, ' ')
  return !stripped.trim()
}

/**
 * The signature HTML an account actually sends: its own override, nothing when
 * it opts out, or the app-wide signature. Accounts that can't send (RSS) and
 * blank signatures resolve to ''.
 */
export function resolveSignature(account: Account | undefined | null, globalHtml: string): string {
  const override: AccountSignature | null | undefined = account?.signature
  const html = override?.mode === 'none' ? '' : override?.mode === 'custom' ? override.html : globalHtml
  return isBlankSignature(html) ? '' : html
}

/**
 * What to persist for an account's signature choice. A `global` mode with
 * nothing written is stored as "no override" (null) so the account simply
 * follows the app-wide signature; every other combination keeps the html, so
 * flipping to None and back does not lose what the user wrote.
 */
export function accountSignaturePayload(mode: AccountSignature['mode'], html: string): AccountSignature | null {
  return mode === 'global' && !html ? null : { mode, html }
}

export type ComposeBody = { rich: boolean; html: string; text: string }

/** A signature resolved for a draft, in both of the forms a body can need. */
export type Signature = { html: string; text: string }

/**
 * A signature's plaintext form: block elements end a line, `<br>` breaks one,
 * and the remaining markup and entities are unwrapped.
 *
 * Deliberately not `lib/html`'s `htmlToText`, which parses arbitrary message
 * HTML through the DOM. A signature is short and comes out of our own editor,
 * and doing it with string work keeps this identical to the mobile
 * implementation — the two platforms share the stored signature, so they had
 * better render it the same way.
 */
export function signatureToText(html: string): string {
  if (!html.trim()) return ''
  return html
    .replace(/<\/(p|div|li|tr|h[1-6]|blockquote|pre)>/gi, '\n')
    .replace(/<(br|hr)\s*\/?>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/gi, ' ')
    .replace(/&lt;/gi, '<')
    .replace(/&gt;/gi, '>')
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/gi, '&')
    .split('\n')
    .map((line) => line.trim())
    .join('\n')
    .replace(/^\n+|\n+$/g, '')
    .trim()
}

/** An account's signature in both body forms, ready to insert or track. */
export function signatureForms(html: string): Signature {
  return { html, text: signatureToText(html) }
}

/**
 * Where the signature lands relative to whatever the draft was seeded with.
 * 'aboveQuote' for a forward, whose seeded body is the quoted message: the
 * signature belongs between what the user is about to type and the quote, as
 * in Gmail and Apple Mail. 'belowText' for everything else, where the seed is
 * the user's own text (a quick reply carried into the full editor) or nothing.
 */
export type SignaturePlacement = 'aboveQuote' | 'belowText'

/** A signature this app put in a draft body, and where it put it. */
export type SignatureMark = Signature & { placement: SignaturePlacement }

/**
 * What a draft knows about the signature in its body — three states, not two:
 *
 *   a mark with html   the app inserted exactly this, there; it can be swapped
 *   a mark without     the app inserted nothing, because the account sends no
 *                      signature. The placement is still remembered, so a later
 *                      account with one puts it where this draft's would have
 *                      gone — above the quote of a forward, not after it.
 *   absent             unmanaged. The body came from elsewhere (a saved draft
 *                      reopened, "Edit as New Message") and may well already
 *                      end in a signature. Nothing here is ours to rewrite, and
 *                      appending would give the message two.
 *
 * `null` means the same as a mark without html, and is what compose tabs
 * persisted by a build that had no placement to remember come back as.
 */
export type SignatureTracking = SignatureMark | null | undefined

/** The tracking for a draft the app deliberately gave no signature. */
export function noSignatureMark(placement: SignaturePlacement): SignatureMark {
  return { html: '', text: '', placement }
}

/**
 * Move a draft body to another account's signature, and report what the draft
 * now knows about it (see [[SignatureTracking]]).
 *
 * A signature that is no longer in the body verbatim has been edited — the text
 * is the user's now, so it is neither replaced nor tracked any further.
 */
export function bodyWithSwappedSignature(
  body: ComposeBody,
  tracking: SignatureTracking,
  next: Signature,
): { body: ComposeBody; tracking: SignatureTracking } {
  // Unmanaged: leave the body alone, and keep it unmanaged.
  if (tracking === undefined) return { body, tracking }

  const markFor = (placement: SignaturePlacement): SignatureTracking =>
    next.html ? { ...next, placement } : noSignatureMark(placement)

  // Nothing inserted yet: this is the first signature the draft gets, and it
  // goes where this draft's signature belongs — which a forward opened under an
  // account with no signature of its own still remembers.
  if (!tracking?.html) {
    const placement = tracking?.placement ?? 'belowText'
    return { body: bodyWithSignature(body, next, placement), tracking: markFor(placement) }
  }

  const at = trackedIndex(body, tracking)
  if (at < 0) return { body, tracking: undefined }

  if (body.rich) {
    // Removing the signature takes the empty paragraph inserted with it, or
    // A -> none -> B would leave a blank line behind on every round trip.
    const cut = !next.html && body.html.slice(0, at).endsWith(RICH_SEPARATOR) ? at - RICH_SEPARATOR.length : at
    const html = body.html.slice(0, cut) + next.html + body.html.slice(at + tracking.html.length)
    return { body: { ...body, html }, tracking: markFor(tracking.placement) }
  }
  const prefix = body.text.slice(0, at)
  const suffix = body.text.slice(at + tracking.text.length)
  const text = next.text ? prefix + next.text + suffix : joinAcrossRemoval(prefix, suffix)
  return { body: { ...body, text }, tracking: markFor(tracking.placement) }
}

/**
 * Where the tracked signature sits in the body, or -1 when it cannot be
 * identified — because the user has edited it, or because the body holds a
 * second block just like it and ours is no longer where it was put.
 *
 * The placement says which copy is ours when there are several: above a quote
 * it is the first, below the text the last. That alone is a guess, though, and
 * guessing wrong would rewrite the user's own words — so in an ambiguous body
 * ours must still be exactly at the edge it was inserted against (nothing but
 * blank lines beyond it) to be touched at all. Failing that the draft simply
 * stops being managed: nothing rewritten, and no second signature added.
 */
function trackedIndex(body: ComposeBody, mark: SignatureMark): number {
  const [haystack, needle] = body.rich ? [body.html, mark.html] : [body.text, mark.text]
  if (!needle) return -1
  const matches = blockMatches(haystack, needle, body.rich)
  if (matches.length === 0) return -1

  const index = mark.placement === 'aboveQuote' ? matches[0] : matches[matches.length - 1]
  if (matches.length === 1) return index

  const edge = mark.placement === 'aboveQuote' ? haystack.slice(0, index) : haystack.slice(index + needle.length)
  return isBlankFragment(edge, body.rich) ? index : -1
}

/**
 * Every position where `needle` sits in `haystack` as a block of its own.
 *
 * In plaintext that means whole lines, which is what tells an untouched
 * signature apart from one the user has written into: "Ping" must not match
 * inside "Ping, but edited". In HTML the same markup can appear nested inside
 * the user's own content — quoting the signature inside a blockquote, say, puts
 * a byte-identical `<p>Ping</p>` in the body — so a match only counts when it
 * begins at a top-level block and covers whole blocks of the document.
 */
function blockMatches(haystack: string, needle: string, rich: boolean): number[] {
  if (rich) return richBlockMatches(haystack, needle)
  const out: number[] = []
  let index = haystack.indexOf(needle)
  while (index >= 0) {
    const end = index + needle.length
    if ((index === 0 || haystack[index - 1] === '\n') && (end === haystack.length || haystack[end] === '\n')) {
      out.push(index)
    }
    index = haystack.indexOf(needle, index + 1)
  }
  return out
}

/** Offsets where `needle` starts a run of whole top-level blocks of `html`. */
function richBlockMatches(html: string, needle: string): number[] {
  const blocks = topLevelBlocks(html)
  // A body the DOM cannot round-trip (an older tab, hand-edited content) is not
  // one to guess at: no match means the signature is simply left alone.
  if (blocks.join('') !== html) return []
  const out: number[] = []
  let offset = 0
  const needleBlocks = topLevelBlocks(needle)
  if (needleBlocks.join('') !== needle || needleBlocks.some(isContainerBlock)) return []
  for (const block of blocks) {
    if (
      !isContainerBlock(block) &&
      html.startsWith(needle, offset) &&
      endsOnBlockBoundary(blocks, offset, needle.length)
    ) {
      out.push(offset)
    }
    offset += block.length
  }
  return out
}

function isContainerBlock(html: string): boolean {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const element = doc.body.firstElementChild
  return !!element?.querySelector(
    ':scope > p, :scope > div, :scope > blockquote, :scope > ul, :scope > ol, :scope > table',
  )
}

/** Whether a match starting at a block boundary also ends on one. */
function endsOnBlockBoundary(blocks: string[], start: number, length: number): boolean {
  let offset = 0
  for (const block of blocks) {
    offset += block.length
    if (offset === start + length) return true
    if (offset > start + length) return false
  }
  return false
}

/**
 * The document's top-level blocks, as the markup that produced them. The
 * composer's HTML is a flat sequence of blocks (tiptap's schema has no
 * top-level text), so joining these reproduces the original string.
 */
function topLevelBlocks(html: string): string[] {
  if (!html) return []
  const doc = new DOMParser().parseFromString(html, 'text/html')
  return Array.from(doc.body.childNodes).map((node) =>
    node.nodeType === Node.ELEMENT_NODE ? (node as Element).outerHTML : (node.textContent ?? ''),
  )
}

/** Whether a fragment holds nothing but blank lines (or empty paragraphs). */
function isBlankFragment(fragment: string, rich: boolean): boolean {
  return (rich ? fragment.split(RICH_SEPARATOR).join('') : fragment).trim() === ''
}

/**
 * Rejoin a plaintext body after cutting the signature out of it, closing the
 * blank line that separated it. Only the seam is touched: blank lines elsewhere
 * are the user's text (or a quote's), and rewriting those is not our business.
 */
function joinAcrossRemoval(prefix: string, suffix: string): string {
  const head = prefix.replace(/\n+$/, '')
  const tail = suffix.replace(/^\n+/, '')
  // A signature above a quote sat under a blank line meant for typing; that
  // line is not part of the signature, so it stays.
  if (!head) return tail ? `\n\n${tail}` : ''
  return tail ? `${head}\n\n${tail}` : head
}

/** The empty paragraph that separates the signature from the body above it. */
const RICH_SEPARATOR = '<p></p>'

/** Place a signature in a draft body, leaving a blank line for the cursor. */
export function bodyWithSignature(
  body: ComposeBody,
  signature: Signature,
  placement: SignaturePlacement = 'belowText',
): ComposeBody {
  if (!signature.html) return body
  if (body.rich) {
    const html =
      placement === 'aboveQuote'
        ? `${RICH_SEPARATOR}${signature.html}${body.html}`
        : `${body.html}${RICH_SEPARATOR}${signature.html}`
    return { ...body, html }
  }
  const signatureText = signature.text
  if (!signatureText) return body
  if (!body.text.trim()) return { ...body, text: `\n\n${signatureText}` }
  // Only the blank line at the join is this function's business: whitespace the
  // user (or a quote) put elsewhere in the body stays exactly as it is.
  const text =
    placement === 'aboveQuote'
      ? `\n\n${signatureText}\n\n${body.text.replace(/^\n+/, '')}`
      : `${body.text.replace(/\n+$/, '')}\n\n${signatureText}`
  return { ...body, text }
}
