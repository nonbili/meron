import { combineTransactionSteps } from '@tiptap/core'
import Link from '@tiptap/extension-link'
import type { Fragment, Mark, MarkType, Node as ProseMirrorNode } from '@tiptap/pm/model'
import { Plugin, PluginKey, type Transaction } from '@tiptap/pm/state'
import { Mapping, ReplaceAroundStep, ReplaceStep, type Transform } from '@tiptap/pm/transform'
import { findBareUrls } from '../../lib/linkify'

/**
 * The editors' link, Gmail style: typing at either end of a link stays out of
 * it. (Tiptap makes links inclusive when it autolinks, so text typed after a
 * link joined it, and at the start of a line ProseMirror gave text typed
 * before one the link ahead.)
 *
 * A link whose text is a URL is the exception: it is the URL as written, so
 * after any edit touching it the link covers the URL its text now reads, with
 * the address following the text. Extending or trimming a pasted URL keeps
 * the link whole and pointing where it says, and the sentence punctuation or
 * unmatched bracket after it stays out, by the same rules as the outgoing
 * linkifier ([findBareUrls]).
 */
export const ComposerLink = Link.extend({
  inclusive() {
    return false
  },
  addProseMirrorPlugins() {
    // Replaced text gets its link back first, so URL syncing sees it linked.
    return [...(this.parent?.() ?? []), keepReplacedLinks(this.type), urlLinkSync(this.type)]
  },
})

/** Whether an edit's transactions bring their own marks: undo and redo bring
 *  back what was there, a paste or drop what was copied, and the app's own
 *  updates (a signature switch) their own links. */
function bringsOwnMarks(transactions: readonly Transaction[]): boolean {
  return transactions.some(
    (tr) =>
      tr.getMeta('history$') ||
      tr.getMeta('paste') ||
      tr.getMeta('uiEvent') === 'drop' ||
      tr.getMeta('addToHistory') === false,
  )
}

/**
 * Text typed over part of one link stays in it: fixing the end of a label
 * (`Docx` to `Docs`). A link that doesn't extend past its end would otherwise
 * drop off a replacement that reaches that end. Typing over all of it
 * replaces the link with plain text, as in Gmail.
 */
function keepReplacedLinks(linkType: MarkType): Plugin {
  return new Plugin({
    key: new PluginKey('keepReplacedLinks'),
    appendTransaction(transactions, _oldState, state) {
      if (!transactions.some((transaction) => transaction.docChanged) || bringsOwnMarks(transactions)) return
      const tr = state.tr
      transactions.forEach((transaction, t) => {
        transaction.steps.forEach((step, s) => {
          if (!(step instanceof ReplaceStep) || step.from === step.to || step.slice.size === 0) return
          const doc = transaction.docs[s]
          const link = linkAcross(doc, step.from, step.to, linkType)
          if (!link || sliceHasLink(step.slice.content, linkType)) return
          const before = link.isInSet(doc.resolve(step.from).nodeBefore?.marks ?? [])
          const after = link.isInSet(doc.resolve(step.to).nodeAfter?.marks ?? [])
          if (!before && !after) return
          // Carry the inserted range through the rest of the edit.
          const mapping = new Mapping(transaction.mapping.maps.slice(s + 1))
          for (const later of transactions.slice(t + 1)) mapping.appendMapping(later.mapping)
          const from = mapping.map(step.from, 1)
          const to = mapping.map(step.from + step.slice.size, -1)
          if (from < to) tr.addMark(from, to, link)
        })
      })
      return tr.docChanged ? tr : undefined
    },
  })
}

/** The one link all the text in `from..to` carries, if it does. */
function linkAcross(doc: ProseMirrorNode, from: number, to: number, linkType: MarkType): Mark | undefined {
  let link: Mark | undefined
  let mixed = false
  doc.nodesBetween(from, to, (node) => {
    if (mixed || !node.isText) return !mixed
    const mark = linkType.isInSet(node.marks)
    if (!mark || (link && !link.eq(mark))) mixed = true
    else link = mark
    return false
  })
  return mixed ? undefined : link
}

function sliceHasLink(content: Fragment, linkType: MarkType): boolean {
  let found = false
  content.descendants((node) => {
    found ||= !!linkType.isInSet(node.marks)
    return !found
  })
  return found
}

const URL_START = /^(?:https?:\/\/|www\.)/i

function urlLinkSync(linkType: MarkType): Plugin {
  return new Plugin({
    key: new PluginKey('urlLinkSync'),
    appendTransaction(transactions, oldState, state) {
      if (!transactions.some((transaction) => transaction.docChanged)) return
      // Undo and redo bring back what was there; the app's own updates (a
      // signature switch) bring their own links.
      if (transactions.some((tr) => tr.getMeta('history$') || tr.getMeta('addToHistory') === false)) return
      const transform = combineTransactionSteps(oldState.doc, [...transactions])
      const back = transform.mapping.invert()
      const tr = state.tr
      const changes = contentChanges(transform)
      for (const range of changes) {
        syncUrlLinks(state.doc, tr, linkType, range, (run) => {
          // A link wholly within what the edit inserted is new (pasted, or put
          // in by the Link dialog), not an old one edited; one that also
          // keeps old text (`https://` retyped as `http://`) was edited.
          if (changes.some((change) => change.from <= run.from && run.to <= change.to)) return false
          return wasSelfLink(oldState.doc, back.map(run.from, 1), run)
        })
      }
      return tr.steps.length ? tr : undefined
    },
  })
}

/** A link's run of text, and the index of its first child in its block. */
type LinkRun = { from: number; to: number; mark: Mark; index: number }

/** Where the text changed, in the final document. Formatting alone (bolding
 *  the word after a link) changes no text, so it is no edit to a URL. */
function contentChanges(transform: Transform): { from: number; to: number }[] {
  const ranges: { from: number; to: number }[] = []
  transform.steps.forEach((step, index) => {
    if (!(step instanceof ReplaceStep || step instanceof ReplaceAroundStep)) return
    const after = transform.mapping.slice(index + 1)
    step.getMap().forEach((_oldStart, _oldEnd, newStart, newEnd) => {
      ranges.push({ from: after.map(newStart, -1), to: after.map(newEnd, 1) })
    })
  })
  return ranges
}

/**
 * Fit each URL link touching `changed` to the URL its text now reads. Only a
 * link that pointed to its own text before the edit is fitted: one whose
 * destination was chosen (in the Link dialog, or by the sender of pasted
 * mail) keeps it. A link grows only into text the edit added, never into
 * text that was already there, and one whose text no longer reads as a URL
 * stops being a link rather than keep pointing where it no longer says.
 */
function syncUrlLinks(
  doc: ProseMirrorNode,
  tr: Transaction,
  linkType: MarkType,
  changed: { from: number; to: number },
  isSelfLink: (run: LinkRun) => boolean,
) {
  doc.nodesBetween(Math.max(0, changed.from - 1), Math.min(doc.content.size, changed.to + 1), (block, pos) => {
    if (!block.isTextblock) return true
    for (const run of linkRuns(block, pos + 1, linkType)) {
      if (run.from > changed.to) continue
      // The edit may land past the link but still within its word: typing on
      // after punctuation the link left out (`…/a?` then `q`).
      const word = wordFrom(block, run)
      if (word === null || Math.max(run.to, run.from + word.length) < changed.from || !isSelfLink(run)) continue
      const [url] = URL_START.test(word) ? findBareUrls(word) : []
      if (!url || url.start !== 0) {
        tr.removeMark(run.from, run.to, run.mark)
        continue
      }
      // Grow only across text this edit added, past at most the punctuation
      // the link left out for now: never over text that was already there.
      const gap = doc.textBetween(run.to, Math.max(run.to, changed.from))
      const grows = changed.to > changed.from && /^[.,;:!?*)\]}]*$/.test(gap)
      // An edit wholly after the link (a space typed) can't take the link's
      // own last characters away: a `!` the URL was written with stays.
      const fitted = Math.min(run.from + url.end, grows ? Math.max(run.to, changed.to) : run.to)
      const end = changed.from >= run.to ? Math.max(fitted, run.to) : fitted
      const href = hrefOf(doc.textBetween(run.from, end), String(run.mark.attrs.href ?? ''))
      if (end === run.to && href === run.mark.attrs.href) continue
      if (end < run.to) tr.removeMark(end, run.to, run.mark)
      tr.addMark(run.from, end, linkType.create({ ...run.mark.attrs, href }))
    }
    return false
  })
}

/** The address a URL as written opens: `www.` gets a scheme, the one the
 *  link had if any (an `http://` site may not serve HTTPS). */
function hrefOf(text: string, previous: string): string {
  if (!/^www\./i.test(text)) return text
  return `${/^https?:\/\//i.exec(previous)?.[0] ?? 'https://'}${text}`
}

/** Whether, before the edit, the link at `oldPos` pointed to its own text. */
function wasSelfLink(oldDoc: ProseMirrorNode, oldPos: number, run: LinkRun): boolean {
  const $pos = oldDoc.resolve(oldPos)
  if (!$pos.parent.isTextblock) return false
  const old = linkRuns($pos.parent, $pos.start(), run.mark.type).find(
    (candidate) => candidate.from <= oldPos && oldPos < candidate.to && candidate.mark.eq(run.mark),
  )
  if (!old) return false
  const text = oldDoc.textBetween(old.from, old.to)
  // Only the forms read here: a bare `example.com` autolinked by Tiptap is
  // left as it is.
  if (!URL_START.test(text)) return false
  const href = String(run.mark.attrs.href ?? '')
  // `www.` text autolinked or pasted with either scheme.
  return href === text || href.replace(/^https?:\/\//i, '') === text
}

/** The runs of one link (adjacent text with an equal link mark) in a block
 *  whose content starts at `start`. */
function linkRuns(block: ProseMirrorNode, start: number, linkType: MarkType): LinkRun[] {
  const runs: LinkRun[] = []
  block.forEach((child, offset, index) => {
    const mark = linkType.isInSet(child.marks)
    const last = runs[runs.length - 1]
    if (!mark) return
    if (last && last.to === start + offset && last.mark.eq(mark)) last.to += child.nodeSize
    else runs.push({ from: start + offset, to: start + offset + child.nodeSize, mark, index })
  })
  return runs
}

/** Longest URL kept in sync: it bounds the text read for each link. */
const MAX_URL_LENGTH = 4096

/** The text of `run` and what follows it up to the next whitespace, non-text
 *  node or other link; null when that runs past [MAX_URL_LENGTH], where a URL
 *  can't be read whole. */
function wordFrom(block: ProseMirrorNode, run: LinkRun): string | null {
  // From the link's own first child on, stopping at the word's end: each
  // link reads only its own word, not the paragraph.
  let word = ''
  for (let index = run.index; index < block.childCount; index += 1) {
    const child = block.child(index)
    const link = run.mark.type.isInSet(child.marks)
    // Text that can't carry this link (inline code) ends it too.
    const excludes = child.marks.some((mark) => mark.type !== run.mark.type && mark.type.excludes(run.mark.type))
    if (!child.isText || (link && !link.eq(run.mark)) || excludes) break
    const space = child.text!.search(/\s/)
    word += space < 0 ? child.text! : child.text!.slice(0, space)
    if (space >= 0) break
    if (word.length > MAX_URL_LENGTH) return null
  }
  return word.length > MAX_URL_LENGTH ? null : word
}
