import type { Message } from '../types'
import { t } from '../lib/i18n'
import { showToast, ui$ } from './ui'
import { accounts$, isSendableAccount } from './accounts'
import { mail$, getActiveThread, normalizeMessageId } from './mail'
import { isDraftFolder } from './mailFolders'
import { discardSavedDraftCopy } from './mailMoves'
import { getPendingSend, getUnsentRescue, unsentRescues } from './pendingSends'
import { bodyWithSignature, bodyWithSwappedSignature, signatureForms, type ComposeBody } from '../lib/signature'
import { settings$ } from './settings'
import {
  draftIdOfFailedSave,
  newestMessage,
  readComposerAttachments,
  resolveSignatureFor,
  saveComposedDraft,
} from './compose'
import { buildReplyRecipients, buildReplyThreading, pickReplyTarget, resolveQuickReplyFrom } from './composeReply'
import { compose$, newDraftMessageId } from './composeState'

// Quick-reply draft lifecycle: signature seeding, autosave, hydration from a
// thread's tail draft, and ownership hand-offs with compose tabs and sends.

function ownQuickReplyDraft(draftId: string, threadId: string, saved: boolean) {
  compose$.quickReplyDraftId.set(draftId)
  compose$.quickReplyDraftSaved.set(saved)
  compose$.quickReplyDraftThreadId.set(threadId)
}

export function clearQuickReplyDraftOwnership() {
  compose$.quickReplyDraftId.set('')
  compose$.quickReplyDraftSaved.set(false)
  compose$.quickReplyDraftThreadId.set('')
}

// While the Current conversation tab is active (activeTab ""), mirror every
// selectedThread change into conversationThread so it always remembers the live
// navigation. Thread-tab activations set activeTab first, so this guard skips
// their selectedThread retarget and leaves the Current tab's thread intact.
ui$.selectedThread.onChange(({ value }) => {
  if (compose$.activeTab.peek() === '') compose$.conversationThread.set(value)
})

/**
 * Seed the quick reply with the replying account's signature, as the box the
 * user starts typing into rather than something stapled on at send time — the
 * rule the full composer follows, and what every other mail client shows.
 *
 * Replaces whatever the box holds, so it is only ever called on a fresh quick
 * reply: a thread switch, or the clear after a send or an escalation.
 */
export function seedQuickReplySignature() {
  watchQuickReplySignatureSources()
  const thread = getActiveThread()
  const accountId = thread?.account_id || ui$.selectedAccount.peek()
  const account = accounts$.peek().find((acc) => acc.id === accountId)
  const signature = isSendableAccount(account) ? resolveSignatureFor(account) : signatureForms('')
  compose$.composer.set(signature.text ? bodyWithSignature(EMPTY_PLAIN_BODY, signature).text : '')
  compose$.quickReplySignature.set(signature.text ? { ...signature, placement: 'belowText' } : null)
}

/** An empty plaintext body, for seeding and for stripping back to. */
const EMPTY_PLAIN_BODY: ComposeBody = { rich: false, html: '', text: '' }

/**
 * The quick reply with the signature this app seeded taken back out, and
 * whether it was still there to take. A signature the user has typed into can
 * no longer be identified (see `bodyWithSwappedSignature`), which is the same
 * answer as "this text is theirs now".
 */
export function quickReplyWithoutSignature(): { text: string; found: boolean } {
  const text = compose$.composer.peek()
  const mark = compose$.quickReplySignature.peek()
  if (!mark?.text) return { text, found: false }
  const swapped = bodyWithSwappedSignature({ ...EMPTY_PLAIN_BODY, text }, mark, signatureForms(''))
  return swapped.tracking === undefined ? { text, found: false } : { text: swapped.body.text, found: true }
}

/**
 * Whether the quick reply holds nothing the user put there. A seeded signature
 * does not count as content: it is not something they wrote, and treating it as
 * such would save a draft for every thread they merely open, and let an
 * untouched box be "sent".
 */
export function isQuickReplyBlank(): boolean {
  return !quickReplyWithoutSignature().text.trim() && compose$.composerAttachments.peek().length === 0
}

/**
 * The quick reply as it should be saved and sent: what the box holds, minus
 * only the blank line the signature was seeded under.
 *
 * That padding is dropped just when nothing was typed above it — an
 * attachment-only reply, whose body is the untouched signature and would
 * otherwise go out with leading newlines. Whitespace anywhere else is the
 * user's (an indented first line, trailing blank lines they left) and is sent
 * exactly as written, so this is never a blanket trim.
 */
export function quickReplyOutgoingText(): string {
  const text = compose$.composer.peek()
  const withoutSignature = quickReplyWithoutSignature()
  if (!withoutSignature.found || withoutSignature.text.trim()) return text
  return text.replace(/^\n+/, '')
}

/**
 * How far into the box the user's own text runs — where the caret belongs when
 * the reply shortcut focuses a box whose last lines are the signature.
 */
export function quickReplyCaretOffset(): number {
  const { text, found } = quickReplyWithoutSignature()
  if (!found) return compose$.composer.peek().length
  return text.replace(/\n+$/, '').length
}

// The one in-flight (or queued) quick-reply save. Saves are chained onto it so
// autosaves can't overtake each other on the wire, and discardQuickReplyDraftIfEmpty
// awaits it before trusting quickReplyDraftSaved — the flag is only set once the
// RPC resolves, so peeking it mid-save would miss the draft about to be created.
export let quickReplyDraftSaveInFlight: Promise<void> | null = null

// Save the active thread's quick reply as a real server-side draft, reusing
// the full composer's saveDraft RPC (saveComposedDraft/mail.saveDraft) rather
// than a separate persistence path. No-op when there's nothing to save or no
// sendable account. The draft id is minted once and reused across autosaves
// so the server-side copy is replaced, not duplicated.
export function saveQuickReplyDraft(): Promise<void> {
  const previous = quickReplyDraftSaveInFlight
  const run = (async () => {
    if (previous) await previous
    await performQuickReplyDraftSave()
  })()
  quickReplyDraftSaveInFlight = run
  void run.finally(() => {
    if (quickReplyDraftSaveInFlight === run) quickReplyDraftSaveInFlight = null
  })
  return run
}

async function performQuickReplyDraftSave() {
  const activeT = getActiveThread()
  if (!activeT) return
  const text = quickReplyOutgoingText()
  const attachments = compose$.composerAttachments.peek()
  if (isQuickReplyBlank()) return

  const replyAccountId = activeT.account_id || ui$.selectedAccount.peek()
  if (!replyAccountId || replyAccountId === 'unified') return
  const accounts = accounts$.peek()
  const activeAcc = accounts.find((acc) => acc.id === replyAccountId) || accounts[0] || null
  if (activeAcc?.provider === 'rss' || activeAcc?.auth_type === 'rss') return

  const target = pickReplyTarget(activeT)
  const { to, cc } = buildReplyRecipients(target)
  const { in_reply_to, references } = buildReplyThreading(target)
  const fromEmail = resolveQuickReplyFrom(target, activeAcc)
  const subject = activeT.subject.startsWith('Re:') ? activeT.subject : `Re: ${activeT.subject}`
  // Whatever is in the box now is a different reply from the one a send took, so
  // it needs a draft of its own: reusing the claimed id would write this text
  // over the copy that send is about to discard, taking the box's safety copy
  // with it.
  const boxDraftId = compose$.quickReplyDraftId.peek()
  const draftId = boxDraftId && !draftClaimedElsewhere(boxDraftId) ? boxDraftId : newDraftMessageId()
  ownQuickReplyDraft(draftId, activeT.thread_id, boxDraftId === draftId && compose$.quickReplyDraftSaved.peek())
  const startTick = ++quickReplyOwnershipTick

  let savedDraftId = draftId
  // Whether the server is known to hold the copy. A save that failed after its
  // id was allocated may or may not have written it.
  let confirmed = true
  try {
    savedDraftId = await saveComposedDraft({
      accountId: replyAccountId,
      from: fromEmail,
      to,
      cc,
      subject,
      rich: false,
      content: text,
      inReplyTo: in_reply_to,
      references,
      draftMessageId: draftId,
      attachments,
    })
    // Only when the box still points at the placeholder this save started from:
    // a send (or a thread switch) that cleared it in the meantime has moved on,
    // and writing the server id back would leave it claiming a draft that is
    // about to be discarded.
    if (savedDraftId !== draftId && compose$.quickReplyDraftId.peek() === draftId) {
      compose$.quickReplyDraftId.set(savedDraftId)
    }
    unconfirmedDraftIds.delete(normalizeMessageId(savedDraftId))
  } catch (error) {
    console.error('Quick reply draft autosave failed:', error)
    // A save that allocated an id may have written the draft anyway, so the id
    // is kept — for the next save to overwrite and a send to clean up — but
    // never as a saved copy: nothing may rely on it holding the reply.
    const attemptedId = draftIdOfFailedSave(error)
    if (!attemptedId || attemptedId === draftId) return
    savedDraftId = attemptedId
    confirmed = false
    unconfirmedDraftIds.add(normalizeMessageId(savedDraftId))
    if (compose$.quickReplyDraftId.peek() === draftId) compose$.quickReplyDraftId.set(savedDraftId)
  }

  // A send that took the box *after* this save started consumed the very text it
  // just wrote; hand the id over and let the post-send discard drop the copy.
  // Without this the draft is left in Drafts beside the message it was sent as.
  if (handOverToOvertakingSend(activeT.thread_id, startTick, savedDraftId, confirmed)) return

  // Same story for an escalation that couldn't hand this id to the full editor:
  // the tab is editing a draft of its own, so this copy is an orphan whoever is
  // typing in the box by now.
  const abandonedAt = quickReplyAbandonedSaves.get(activeT.thread_id) ?? 0
  if (abandonedAt > startTick) {
    quickReplyAbandonedSaves.delete(activeT.thread_id)
    if (compose$.quickReplyDraftId.peek() === savedDraftId) {
      clearQuickReplyDraftOwnership()
    }
    await discardSavedDraftCopy({
      threadId: activeT.thread_id,
      messageId: '',
      folderId: '',
      accountId: replyAccountId,
      draftMessageId: savedDraftId,
    })
    return
  }

  // The composer may have gone blank while the RPC was in flight (cleared by
  // the user, a send, or escalation to the full editor) — any discard that ran
  // in that window saw quickReplyDraftSaved still false and bailed. Drop the
  // draft we just wrote instead of letting it resurrect on the next thread open.
  const sameThread = ui$.selectedThread.peek() === activeT.thread_id
  // ...unless something else took this exact draft over in the meantime.
  if (sameThread && isQuickReplyBlank() && !draftClaimedElsewhere(savedDraftId)) {
    if (compose$.quickReplyDraftId.peek() === savedDraftId) {
      clearQuickReplyDraftOwnership()
    }
    await discardSavedDraftCopy({
      threadId: activeT.thread_id,
      messageId: '',
      folderId: '',
      accountId: replyAccountId,
      draftMessageId: savedDraftId,
    })
    return
  }
  if (confirmed && sameThread && compose$.quickReplyDraftId.peek() === savedDraftId) {
    compose$.quickReplyDraftSaved.set(true)
  }
}

/** Ids a failed save may or may not have written under, normalized. The box and
 * any send that takes it keep such an id apart from a saved draft's: reused so
 * the next write overwrites the copy, discarded once its reply has gone out, but
 * never trusted as the copy that keeps an unsent reply safe. */
export const unconfirmedDraftIds = new Set<string>()

/** The box's draft id when all that is known of it is that a save failed. */
export function unconfirmedQuickReplyDraftId(): string {
  if (compose$.quickReplyDraftSaved.peek()) return ''
  const id = compose$.quickReplyDraftId.peek()
  return id && unconfirmedDraftIds.has(normalizeMessageId(id)) ? id : ''
}

// Discard the quick reply's server-side draft once the user has cleared the
// text/attachments back to blank, mirroring the full composer's discard flow.
export async function discardQuickReplyDraftIfEmpty() {
  // Wait out any in-flight autosave before peeking the flags below; the save
  // itself re-checks on completion and self-discards if the composer is blank.
  while (quickReplyDraftSaveInFlight) await quickReplyDraftSaveInFlight
  if (!isQuickReplyBlank()) return
  // A copy a failed save may have left goes too: if it did land, it holds the
  // text the user just cleared and would come back on the next thread open.
  const draftId = compose$.quickReplyDraftSaved.peek()
    ? compose$.quickReplyDraftId.peek()
    : unconfirmedQuickReplyDraftId()
  if (!draftId) return
  // The box can be left pointing at a draft a send or a full editor tab owns.
  // Clearing the box is still right; deleting their copy is not.
  if (draftClaimedElsewhere(draftId)) {
    clearQuickReplyDraftOwnership()
    return
  }
  const activeT = getActiveThread()

  clearQuickReplyDraftOwnership()
  const discarded = await discardSavedDraftCopy({
    threadId: activeT?.thread_id ?? '',
    messageId: '',
    folderId: '',
    accountId: activeT?.account_id,
    draftMessageId: draftId,
  })
  if (discarded) unconfirmedDraftIds.delete(normalizeMessageId(draftId))
}

// Hide the saved draft currently hydrated into the quick-reply editor. The
// optimistic sent bubble is appended after the draft, so matching only the
// conversation tail would briefly reveal the draft again while sending.
// Other drafts in the thread remain visible.
export function withoutHydratedQuickReplyDraft(
  messages: Message[],
  draftMessageId: string,
  draftSaved: boolean,
  sendingDraftIds: string[] = [],
): Message[] {
  const hidden = new Set(sendingDraftIds.map(normalizeMessageId))
  if (draftSaved && draftMessageId) hidden.add(normalizeMessageId(draftMessageId))
  if (hidden.size === 0) return messages
  return messages.filter(
    (message) =>
      !(
        hidden.has(normalizeMessageId(message.message_id || message.id)) &&
        isDraftFolder(message.folder_id, message.account_id)
      ),
  )
}

let quickReplyDraftSaveTimer: ReturnType<typeof setTimeout> | null = null
const QUICK_REPLY_DRAFT_SAVE_DELAY_MS = 1200
type QuickReplySendHydrationGuard = {
  threadId: string
  accountId: string
  draftId: string
  /** A copy a failed save may have left of this reply — see {@link unconfirmedDraftIds}. */
  unconfirmedDraftId: string
  /** Tick at which the send took the box — see {@link quickReplyOwnershipTick}. */
  claimTick: number
  inFlight: boolean
  suppressDraft: boolean
}
export const quickReplySendHydrationGuards = new Map<string, QuickReplySendHydrationGuard>()

// A monotonic clock for handovers of the quick reply's server-side draft. An
// autosave records the tick it started on; whoever takes the box away from it —
// a send, or an escalation that had no id to hand the full editor — records the
// tick it did so. Comparing the two is what tells a landing save whether the
// copy it just wrote is still the box's, and it has to be a clock rather than a
// "has someone taken over by now?" test: a send that was *already* under way
// when the save started (the user's next reply) must not claim it.
export let quickReplyOwnershipTick = 0

/** Advance [quickReplyOwnershipTick] and return the new tick. */
export function nextQuickReplyOwnershipTick(): number {
  return ++quickReplyOwnershipTick
}
// Bumped whenever the box changes hands: the user typing or attaching, or the
// conversation switching. A send captures it at the click and resets the box
// only if it hasn't moved since — comparing content instead can't tell "they
// typed it back" or "attachment A swapped for B" from "untouched".
export let quickReplyBoxGeneration = 0
compose$.composer.onChange(() => {
  quickReplyBoxGeneration += 1
})
compose$.composerAttachments.onChange(() => {
  quickReplyBoxGeneration += 1
})
compose$.quickReplyFrom.onChange(() => {
  quickReplyBoxGeneration += 1
})
ui$.selectedThread.onChange(() => {
  quickReplyBoxGeneration += 1
})
// Sends that have taken the box but haven't reached their guard yet: the
// identity allocation between the two is an await a landing save can slip
// through, and it needs something to hand its id to even then. The guard adopts
// the claim's ids when it is created. `draftId` is a copy known to be on the
// server — the reply's safety net should the send die; `unconfirmedDraftId` is
// one a failed save may have left, cleaned up after the send but never relied on.
export type QuickReplySendClaim = {
  threadId: string
  tick: number
  draftId: string
  unconfirmedDraftId: string
  generation: number
}

/** Every draft id a claim or guard holds. */
const claimedDraftIds = (claim: { draftId: string; unconfirmedDraftId: string }) =>
  [claim.draftId, claim.unconfirmedDraftId].filter(Boolean)
export const pendingQuickReplySendClaims: QuickReplySendClaim[] = []

/** Give up the claim of a send that never got off the ground, handing the box
 * back what the claim took from it. Nothing was sent, so the draft is still the
 * only copy of the user's reply — this deletes nothing. */
export function releaseUnsentQuickReplyClaim(claim: QuickReplySendClaim) {
  const index = pendingQuickReplySendClaims.indexOf(claim)
  if (index >= 0) pendingQuickReplySendClaims.splice(index, 1)
  publishSendingDraftIds()
  // The box has moved on — its own text, and its own autosave, are under way.
  if (quickReplyBoxGeneration !== claim.generation) return
  if (claim.draftId) {
    // Give the draft back, saved: the box is still holding the reply it belongs
    // to, and a save that handed its id over returns before flipping the flag.
    ownQuickReplyDraft(claim.draftId, claim.threadId, true)
  } else if (claim.unconfirmedDraftId) {
    // Back as it was taken: an id for the next save to overwrite, not a copy.
    ownQuickReplyDraft(claim.unconfirmedDraftId, claim.threadId, false)
  }
  // The claim cancelled this box's pending autosave on the way in. With the send
  // gone and nothing typed since, nothing else would re-arm it, and the reply
  // would sit unsaved until the next keystroke.
  if (!isQuickReplyBlank()) scheduleQuickReplyDraftSave()
}
// Threads whose in-flight autosave was orphaned by an escalation, and the tick
// it happened at. The full editor minted its own draft id in that case, so the
// copy the save lands has nothing pointing at it.
export const quickReplyAbandonedSaves = new Map<string, number>()

// Mirror the guards' draft ids into observable state so the message pane hides
// the same drafts the quick reply refuses to re-hydrate. Called after every
// guard mutation.
export function publishSendingDraftIds() {
  const ids = [
    ...new Set(
      [
        ...pendingQuickReplySendClaims,
        ...[...quickReplySendHydrationGuards.values()].filter((guard) => guard.inFlight || guard.suppressDraft),
      ]
        .flatMap(claimedDraftIds)
        .map(normalizeMessageId)
        .concat([...sentDraftDiscards.keys()]),
    ),
  ]
  const previous = compose$.sendingDraftIds.peek()
  if (previous.length === ids.length && ids.every((id, index) => previous[index] === id)) return
  compose$.sendingDraftIds.set(ids)
}

/** After the user stops typing, save (or discard, if now empty) the real server draft. */
export function scheduleQuickReplyDraftSave() {
  if (quickReplyDraftSaveTimer) clearTimeout(quickReplyDraftSaveTimer)
  quickReplyDraftSaveTimer = setTimeout(() => {
    quickReplyDraftSaveTimer = null
    if (isQuickReplyBlank()) {
      void discardQuickReplyDraftIfEmpty()
    } else {
      void saveQuickReplyDraft()
    }
  }, QUICK_REPLY_DRAFT_SAVE_DELAY_MS)
}

/** Cancel any pending debounced draft save — used when a send, thread switch,
 * or escalation to the full editor should preempt it. */
export function cancelQuickReplyDraftSave() {
  if (quickReplyDraftSaveTimer) {
    clearTimeout(quickReplyDraftSaveTimer)
    quickReplyDraftSaveTimer = null
  }
}

ui$.selectedThread.onChange(({ value }) => {
  // A failed send keeps its consumed draft suppressed while the user remains
  // in that editor. Once they leave, make the server safety copy available for
  // a later reopen; in-flight sends remain guarded across navigation.
  for (const guard of quickReplySendHydrationGuards.values()) {
    if (!guard.inFlight && guard.threadId !== value) guard.suppressDraft = false
  }
  publishSendingDraftIds()
  if (value) return
  cancelQuickReplyDraftSave()
  compose$.composer.set('')
  compose$.composerAttachments.set([])
  clearQuickReplyDraftOwnership()
  compose$.quickReplyFrom.set('')
  compose$.quickReplySignature.set(null)
})

// Pre-fills the quick reply with an already-saved draft sitting at the tail of
// the active thread, so the user can continue and send it inline instead of
// being forced into the full editor. No-op when the tail message isn't a
// draft, or is already the one loaded.
function hydrateQuickReplyFromTailDraft(messages: Message[]) {
  const activeThreadId = ui$.selectedThread.peek()
  if (!activeThreadId) return
  const inThread = messages.filter((message) => message.thread_id === activeThreadId)
  const tail = newestMessage(inThread)
  if (!tail || !isDraftFolder(tail.folder_id, tail.account_id)) return
  // Only a draft we can address on the server. A row synced from its envelope
  // has no Message-ID yet — and no body either, so hydrating it would put an
  // empty box in front of the user claiming to be their draft, and hand any send
  // a draft id nothing can delete: saving would append a second copy under that
  // id, and cleanup could only ever remove one of the two. Reading the thread
  // back-fills the header, and the next refresh hydrates it properly.
  const tailDraftId = tail.message_id
  if (!tailDraftId) return
  const normalizedTailDraftId = normalizeMessageId(tailDraftId)
  // Its message already went out; the copy is only waiting on its discard.
  if (sentDraftDiscards.has(normalizedTailDraftId)) return
  // A send that has claimed the box but is still allocating its identity is as
  // in-flight as one with a guard: hydrating here would hand the box a draft
  // that send is about to consume and discard.
  if (pendingQuickReplySendClaims.some((claim) => claim.threadId === activeThreadId)) return
  const guarded = [...quickReplySendHydrationGuards.values()].some(
    (guard) =>
      guard.threadId === activeThreadId &&
      (guard.inFlight ||
        (guard.suppressDraft && claimedDraftIds(guard).some((id) => normalizeMessageId(id) === normalizedTailDraftId))),
  )
  if (guarded) return
  // An escalated reply is the full editor's now; hydrating it here would put two
  // editors on one server draft, and hand this box a draft it may then discard.
  if (draftOpenInComposeTab(tailDraftId)) return
  if (normalizeMessageId(compose$.quickReplyDraftId.peek()) === normalizedTailDraftId) return
  // Only into a box that is free. Hydration fills an empty quick reply from a
  // saved draft; it is not entitled to replace a reply in progress — one the
  // user began while this very draft was still a header-less row, say — nor to
  // drop the id that reply has already been saved under.
  if (!isQuickReplyBlank() || compose$.quickReplyDraftId.peek()) return

  // The box is taking this draft back from a send that failed and released it
  // (see the selectedThread handler). Ownership moves with it: a retry of that
  // send must not discard what the user is now editing.
  for (const guard of quickReplySendHydrationGuards.values()) {
    if (guard.draftId && normalizeMessageId(guard.draftId) === normalizedTailDraftId) guard.draftId = ''
    if (guard.unconfirmedDraftId && normalizeMessageId(guard.unconfirmedDraftId) === normalizedTailDraftId) {
      guard.unconfirmedDraftId = ''
    }
  }
  // The row is there to hydrate from, so the copy is no longer in doubt.
  unconfirmedDraftIds.delete(normalizedTailDraftId)
  publishSendingDraftIds()

  compose$.composer.set(tail.body ?? '')
  compose$.composerAttachments.set([])
  ownQuickReplyDraft(tailDraftId, activeThreadId, true)
  compose$.quickReplyFrom.set(tail.from_addr ?? '')
  // The saved body already carries whatever signature it was written with, so
  // none of it is this app's to strip, re-seed, or discount as "not content".
  compose$.quickReplySignature.set(null)

  if (tail.has_attachments) {
    void readComposerAttachments(tail.attachments ?? [], tail.body_html ?? '').then((valid) => {
      if (ui$.selectedThread.peek() === activeThreadId && compose$.quickReplyDraftId.peek() === tailDraftId) {
        compose$.composerAttachments.set(valid)
      }
    })
  }
}

/** Whether the quick-reply draft currently belongs to `threadId`. Ownership is
 * recorded when saving starts or hydration lands, so it remains exact while
 * the thread card or Drafts row is absent from the observable mail lists. */
export function quickReplyDraftBelongsToThread(threadId: string): boolean {
  return !!threadId && !!compose$.quickReplyDraftId.peek() && compose$.quickReplyDraftThreadId.peek() === threadId
}

// Drafts the user has asked to open, counted: opening one reads the thread
// first, so the tab appears an RPC later than the click — long enough for a
// send's cleanup to decide the draft is nobody's and delete it — and two clicks
// can be in that window at once, where the first to finish must not release the
// second's hold.
const openingComposeTabDraftIds = new Map<string, number>()
// Drafts whose discard is on the wire. Opening one now would put a live editor
// on a server copy that is going away, so the tab starts its own draft instead.
export const discardingDraftIds = new Set<string>()

type SentDraft = Parameters<typeof discardSavedDraftCopy>[0]
// Drafts whose message has already gone out, by Message-ID, from the moment
// their discard starts until it succeeds. Such a copy is stale, not a safety
// net: while it lingers it stays out of the message pane and is never hydrated
// into the quick reply — which would put text that was already sent back in
// front of the user, one click from going out twice.
const sentDraftDiscards = new Map<string, SentDraft>()
// Gaps between further attempts at a discard that failed. A draft still there
// after the last one is left for the user, who is told about it.
const SENT_DRAFT_DISCARD_RETRY_DELAYS_MS = [5_000, 30_000, 120_000, 600_000]

/**
 * Discard the draft behind a message that was just sent. Resolves with whether
 * the first attempt removed it; a failed one keeps the draft suppressed and is
 * retried in the background, so callers can settle their own state either way.
 */
export async function discardSentDraft(draft: SentDraft): Promise<boolean> {
  const id = normalizeMessageId(draft.draftMessageId)
  // A row known only by its local id can be neither suppressed nor retried by
  // Message-ID: one plain attempt, failing out loud.
  if (!id) return discardSavedDraftCopy(draft)
  sentDraftDiscards.set(id, draft)
  publishSendingDraftIds()
  if (await attemptSentDraftDiscard(id, draft)) return true
  void retrySentDraftDiscard(id, draft)
  return false
}

async function attemptSentDraftDiscard(id: string, draft: SentDraft): Promise<boolean> {
  discardingDraftIds.add(id)
  try {
    await discardSavedDraftCopy(draft, { throwOnError: true })
  } catch (error) {
    console.error('Discarding a sent message’s draft failed:', error)
    return false
  } finally {
    discardingDraftIds.delete(id)
  }
  if (sentDraftDiscards.get(id) === draft) {
    sentDraftDiscards.delete(id)
    publishSendingDraftIds()
  }
  return true
}

async function retrySentDraftDiscard(id: string, draft: SentDraft) {
  for (const delay of SENT_DRAFT_DISCARD_RETRY_DELAYS_MS) {
    await new Promise((resolve) => setTimeout(resolve, delay))
    if (sentDraftDiscards.get(id) !== draft) return
    // Reopened in the full editor meanwhile: it is the user's draft again.
    if (draftOpenInComposeTab(id)) {
      sentDraftDiscards.delete(id)
      publishSendingDraftIds()
      return
    }
    if (await attemptSentDraftDiscard(id, draft)) return
  }
  // Still suppressed, so it cannot come back into the quick reply this session;
  // the Drafts folder is where the user can remove it.
  showToast(t('composer.status.couldNotDiscardDraft'), 'error')
}

/** Whether a full-editor tab is editing this server-side draft, or about to be:
 * an escalated quick reply hands its id over, and the tab is its owner from then
 * on. Counts a draft the user has clicked but whose tab hasn't opened yet. */
export function draftOpenInComposeTab(draftId: string): boolean {
  if (!draftId) return false
  const id = normalizeMessageId(draftId)
  if (openingComposeTabDraftIds.has(id)) return true
  return compose$.tabs
    .peek()
    .some((tab) => !!tab.compose?.draftMessageId && normalizeMessageId(tab.compose.draftMessageId) === id)
}

/** Hold a draft the user is opening against a send's cleanup until the read that
 * precedes its editor is done. Identify it the way hydration does — by
 * Message-ID when the row has one, else by row id — so a Drafts row whose
 * headers aren't loaded yet is covered too. Returns the release, which is
 * idempotent: an open that resolves into an owner early (a conversation, whose
 * quick reply then hydrates the draft) calls it before its own `finally`. */
export function reserveOpeningDraft(draft: Pick<Message, 'id' | 'message_id'>): () => void {
  const ids = [...new Set([draft.message_id ?? '', draft.id ?? ''].map(normalizeMessageId))].filter(Boolean)
  for (const id of ids) openingComposeTabDraftIds.set(id, (openingComposeTabDraftIds.get(id) ?? 0) + 1)
  let released = false
  return () => {
    if (released) return
    released = true
    for (const id of ids) {
      const remaining = (openingComposeTabDraftIds.get(id) ?? 1) - 1
      if (remaining > 0) openingComposeTabDraftIds.set(id, remaining)
      else openingComposeTabDraftIds.delete(id)
    }
  }
}

/** Whether anything other than the quick-reply box holds this draft: a send,
 * whose own lifecycle discards it once SMTP succeeds — dropping it earlier
 * would pull the safety copy out from under a send that then fails — or a full
 * editor tab. Every path that deletes the box's draft consults this. */
function draftClaimedElsewhere(draftId: string): boolean {
  if (!draftId) return false
  const id = normalizeMessageId(draftId)
  const claimedBySend = [...quickReplySendHydrationGuards.values(), ...pendingQuickReplySendClaims].some((claim) =>
    claimedDraftIds(claim).some((claimed) => normalizeMessageId(claimed) === id),
  )
  return claimedBySend || draftOpenInComposeTab(draftId)
}

/** Hand a just-landed draft to the send that took the box while it was on the
 * wire — a claim or guard stamped after the save started, still without an id
 * of its own (one is allocated inside the save, so the send couldn't know it).
 * A send that was already under way when the save began is a *previous* reply
 * and must never adopt the text still being written. */
function handOverToOvertakingSend(threadId: string, startTick: number, draftId: string, confirmed: boolean): boolean {
  // A send holding nothing takes the copy; one holding only this same id as
  // unconfirmed has it confirmed. A different id means the send has its own.
  const id = normalizeMessageId(draftId)
  const takes = (candidate: { draftId: string; unconfirmedDraftId: string }) =>
    !candidate.draftId && (!candidate.unconfirmedDraftId || normalizeMessageId(candidate.unconfirmedDraftId) === id)
  const adopt = (candidate: { draftId: string; unconfirmedDraftId: string }) => {
    if (confirmed) {
      candidate.draftId = draftId
      candidate.unconfirmedDraftId = ''
    } else {
      candidate.unconfirmedDraftId = draftId
    }
  }
  const claim = pendingQuickReplySendClaims.find(
    (pending) => pending.threadId === threadId && pending.tick > startTick && takes(pending),
  )
  if (claim) {
    adopt(claim)
    if (normalizeMessageId(compose$.quickReplyDraftId.peek()) === normalizeMessageId(draftId)) {
      clearQuickReplyDraftOwnership()
    }
    publishSendingDraftIds()
    return true
  }
  const guard = [...quickReplySendHydrationGuards.values()].find(
    (candidate) => candidate.threadId === threadId && candidate.claimTick > startTick && takes(candidate),
  )
  if (!guard) return false
  adopt(guard)
  if (normalizeMessageId(compose$.quickReplyDraftId.peek()) === normalizeMessageId(draftId)) {
    clearQuickReplyDraftOwnership()
  }
  publishSendingDraftIds()
  return true
}

mail$.messages.onChange(({ value }) => {
  for (const [tempId, guard] of quickReplySendHydrationGuards) {
    // A guard whose send is still in flight outlives its bubble: on success the
    // pending payload is dropped *before* the post-send draft discard resolves,
    // and a refresh landing in that window can already have swapped the bubble
    // for the canonical Sent copy. Dropping the guard there would let the
    // still-persisted server draft hydrate the just-cleared composer — the very
    // race the guard exists to close.
    if (guard.inFlight) continue
    if (!value.some((message) => message.id === tempId) && !getPendingSend(tempId)) {
      quickReplySendHydrationGuards.delete(tempId)
    }
  }
  publishSendingDraftIds()
  restoreUnsentRescueBubbles(value)
  hydrateQuickReplyFromTailDraft(value)
})

/** Put the bubbles of failed rescues back when their thread is loaded again.
 * Such a bubble is the only copy of a reply that never reached the server, and
 * navigating away replaces the message page wholesale — without this the reply,
 * and the Retry that could still save it, would be gone for good. */
function restoreUnsentRescueBubbles(messages: Message[]) {
  const missing = unsentRescues().filter(
    ([tempId, rescue]) =>
      !messages.some((message) => message.id === tempId) &&
      messages.some((message) => message.thread_id === rescue.threadId),
  )
  if (missing.length === 0) return
  // Deferred: this runs inside the change notification that just replaced the
  // page, and pushing back into it from there fights the update in flight.
  queueMicrotask(() => {
    for (const [tempId, rescue] of missing) {
      if (!getUnsentRescue(tempId)) continue
      const current = mail$.messages.peek()
      if (current.some((message) => message.id === tempId)) continue
      if (!current.some((message) => message.thread_id === rescue.threadId)) continue
      mail$.messages.push(rescue.bubble)
    }
  })
}

// The app signature is read out of the prefs table after the first render, and
// an account's override can be rewritten from the settings dialog while a
// thread is open behind it. Re-seed a quick reply the user hasn't written in
// yet, so it isn't left sitting there without the signature it should have had.
// A box with anything of theirs in it — text, an attachment, a hydrated draft —
// is never rewritten.
function reseedUntouchedQuickReply() {
  if (!ui$.selectedThread.peek()) return
  if (compose$.quickReplyDraftSaved.peek() || compose$.quickReplyDraftId.peek()) return
  if (!isQuickReplyBlank()) return
  seedQuickReplySignature()
}

// Subscribed on the first seed rather than at module scope: this module and
// ./accounts import each other, so `accounts$` is still in its temporal dead
// zone while this file's body runs. Nothing can have seeded a quick reply
// before both modules are live, which makes first-seed the earliest safe point.
let quickReplySignatureSourcesWatched = false

function watchQuickReplySignatureSources() {
  if (quickReplySignatureSourcesWatched) return
  quickReplySignatureSourcesWatched = true
  settings$.signature.onChange(reseedUntouchedQuickReply)
  accounts$.onChange(reseedUntouchedQuickReply)
}
