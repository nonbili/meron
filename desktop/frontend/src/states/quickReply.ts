import { threadWindowSession } from '../lib/windowSession'
import { externalThreadDrafts$ } from '../lib/windowSession'
import { externalComposeDraft$ } from '../lib/composeWindow'
import type { ComposerAttachment, Message } from '../types'
import { contextualErrorMessage } from '../lib/errors'
import { t } from '../lib/i18n'
import { decryptFailed } from '../lib/crypto'
import { showToast, ui$ } from './ui'
import { accounts$, isLocalAccountId, isSendableAccount } from './accounts'
import { getActiveThread, normalizeMessageId } from './mail'
// From the leaf module: this one subscribes to it while loading, which can be
// inside an import cycle through mail.ts.
import { mail$ } from './mailState'
import { threadData$ } from './threadData'
import { isDraftFolder } from './mailFolders'
import { discardSavedDraftCopy } from './mailMoves'
import { getPendingSend, getUnsentRescue, unsentRescues } from './pendingSends'
import { quickReplyDraftSignature, signatureForms, textWithTrailingSignature, type Signature } from '../lib/signature'
import { settings$ } from './settings'
import {
  draftIdOfFailedSave,
  newestMessage,
  readComposerAttachments,
  resolveSignatureFor,
  saveComposedDraft,
} from './compose'
import {
  buildReplyRecipients,
  buildReplyThreading,
  pickReplyTarget,
  quickReplyEncrypts,
  quickReplySigns,
  resolveQuickReplyFrom,
  replySubject,
  addressQuickReply,
  quickReplyAddressing,
} from './composeReply'
import { compose$, newDraftMessageId } from './composeState'
import { draftSaveRunning } from './composeSessions'
import { parseAddressList } from '../components/chat/messageHelpers'
import { bareAddr, splitAddressList } from '../lib/address'

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
  const activeTab = compose$.tabs.peek().find((tab) => tab.id === compose$.activeTab.peek())
  // Selection also changes outside the tab strip: triage advances to a neighbour,
  // folder changes clear it, and board navigation selects another card. Show that
  // selection in Current so the displayed conversation and action target agree.
  if (activeTab?.kind === 'thread' && activeTab.threadId !== value) compose$.activeTab.set('')
  if (compose$.activeTab.peek() === '') compose$.conversationThread.set(value)
})

/**
 * The signature the quick reply goes out with: the replying account's, as it
 * is now. It is not in the box: the box holds only what the user writes, and
 * the signature is added below it when the reply is saved or sent, unless the
 * user switched it off for this reply (`compose$.quickReplySignatureOff`).
 * Accounts that can't send have none.
 */
export function quickReplySignature(thread = getActiveThread()): Signature {
  const accountId = thread?.account_id || ui$.selectedAccount.peek()
  const account = accounts$.peek().find((acc) => acc.id === accountId)
  return isSendableAccount(account) ? resolveSignatureFor(account) : signatureForms('')
}

/**
 * Start a fresh quick reply: an empty box, with the signature on. Called on a
 * thread switch, and after a send or an escalation took the box's reply.
 */
export function startFreshQuickReply() {
  quickReplyScope += 1
  loadingQuickReplyAttachments = null
  restoredQuickReply?.validation?.resolve()
  restoredQuickReply = null
  watchQuickReplySignatureSources()
  hydratedWithoutSignature = null
  compose$.composer.set('')
  compose$.composerAttachments.set([])
  compose$.quickReplySignatureOff.set(false)
  compose$.quickReplySignatureUnmanaged.set(false)
  // The OpenPGP switches are per reply too: back to the defaults.
  compose$.quickReplySign.set(null)
  compose$.quickReplyEncrypt.set(null)
}

/**
 * Whether the quick reply holds nothing the user put there. Such a box saves no
 * draft and can't be sent; the signature alone is not a reply.
 */
export function isQuickReplyBlank(): boolean {
  return !compose$.composer.peek().trim() && compose$.composerAttachments.peek().length === 0
}

/**
 * Whether the conversation shows the quick-reply box: where the
 * conversation-footer setting puts it there, and wherever it still holds a
 * reply, so switching the setting mid-reply hides nothing unsaved. The reply
 * keeps its box until it is sent, emptied, or left behind with its thread, and
 * while the box still owns a saved draft: emptying the box discards that draft,
 * which the box has to stay mounted (useQuickReply) to do. Reactive.
 */
export function quickReplyBoxShown(): boolean {
  return (
    settings$.conversationFooter.get() === 'reply' ||
    !!compose$.composer.get().trim() ||
    compose$.composerAttachments.get().length > 0 ||
    !!compose$.quickReplyDraftId.get()
  )
}

/**
 * The quick reply as it is saved and sent: what the box holds, exactly as
 * written, with the signature below it while it is on. A saved draft is thus
 * the reply as it would go out, and `hydrateQuickReplyFromTailDraft` takes the
 * signature back off when it loads it.
 */
export function quickReplyOutgoingText(thread?: Message): string {
  const text = compose$.composer.peek()
  if (compose$.quickReplySignatureOff.peek()) return text
  return textWithTrailingSignature(text, quickReplySignature(thread).text)
}

/** Switch the signature on or off for this reply; a saved draft follows. */
export function setQuickReplySignatureOff(off: boolean) {
  if (compose$.quickReplySignatureOff.peek() === off) return
  compose$.quickReplySignatureOff.set(off)
  hydratedWithoutSignature = null
  if (!isQuickReplyBlank()) scheduleQuickReplyDraftSave()
}

/** Set the OpenPGP sign switch for this thread's reply; a saved draft follows. */
export function setQuickReplySign(threadId: string, on: boolean) {
  compose$.quickReplySign.set({ threadId, on })
  if (!isQuickReplyBlank()) scheduleQuickReplyDraftSave()
}

/** Where the caret belongs when the reply shortcut focuses the box. */
export function quickReplyCaretOffset(): number {
  return compose$.composer.peek().length
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
// A queued save normally reads the latest box. Navigation supplies a snapshot
// before clearing it, so the same queue can finish a save for an outgoing reply.
type DraftSave = { threadId: string; outgoing?: DraftSaveSnapshot; superseded?: boolean }
const queuedQuickReplySaves = new Set<DraftSave>()

export function saveQuickReplyDraft(): Promise<void> {
  return queueQuickReplyDraftSave({ threadId: ui$.selectedThread.peek() })
}

function queueQuickReplyDraftSave(request: DraftSave): Promise<void> {
  const previous = quickReplyDraftSaveInFlight
  queuedQuickReplySaves.add(request)
  const run = (async () => {
    if (previous) await previous
    while (!request.outgoing && ui$.selectedThread.peek() === request.threadId) {
      const pending = pendingQuickReplyRead()
      if (!pending) break
      await pending
    }
    queuedQuickReplySaves.delete(request)
    if (request.superseded) return
    const snapshot =
      request.outgoing ??
      (ui$.selectedThread.peek() === request.threadId ? snapshotQuickReplyDraft(request.threadId) : null)
    if (!snapshot) return
    const restored = request.outgoing ? null : restoredQuickReply
    await performQuickReplyDraftSave(snapshot)
    // The active editor has now written a newer version than the copy it
    // restored on return. Future reads must stop checking that older copy.
    if (restored && restored === restoredQuickReply && snapshot.identity.saved && !snapshot.identity.failed) {
      restored.validation?.resolve()
      restoredQuickReply = null
    }
  })()
  quickReplyDraftSaveInFlight = run
  void run.finally(() => {
    if (quickReplyDraftSaveInFlight === run) quickReplyDraftSaveInFlight = null
  })
  return run
}

type DraftIdentity = { id: string; saved: boolean; failed?: boolean }
type DraftSaveSnapshot = {
  threadId: string
  identity: DraftIdentity
  args: Omit<Parameters<typeof saveComposedDraft>[0], 'draftMessageId'>
  loading: LoadingQuickReplyAttachments | null
  startTick: number
}
type LoadingQuickReplyAttachments = { files: Promise<ComposerAttachment[]> }
let loadingQuickReplyAttachments: LoadingQuickReplyAttachments | null = null

let writingQuickReplyDraft: DraftIdentity | null = null
// Keep the outgoing editor for the next visit. The tab can replay its cached
// draft before the thread read returns, even after the newer save succeeds.
type DepartingQuickReply = {
  snapshot: DraftSaveSnapshot
  row: Message | undefined
  restore: () => void
  unchanged: () => boolean
}
const departingQuickReplies = new Map<string, DepartingQuickReply>()
let restoredQuickReply: {
  reply: DepartingQuickReply
  validation?: { promise: Promise<void>; resolve: () => void }
  read?: object
} | null = null

function pendingQuickReplyRead() {
  return restoredQuickReply?.validation?.promise ?? loadingQuickReplyAttachments?.files
}

/** Wait for both the draft page and its files. A page refresh can start
 * during an attachment read, and validation can hydrate a new draft. */
export function waitForQuickReplyRead(): Promise<void> | undefined {
  const first = pendingQuickReplyRead()
  if (!first) return
  const scope = quickReplyScope
  return (async () => {
    let pending: Promise<unknown> | undefined = first
    while (pending && scope === quickReplyScope) {
      await pending
      pending = pendingQuickReplyRead()
    }
  })()
}

function draftRow(messages: Message[], draftId: string) {
  const id = normalizeMessageId(draftId)
  return messages.find(
    (message) => isDraftFolder(message.folder_id, message.account_id) && normalizeMessageId(message.message_id) === id,
  )
}

async function matchesRestoredDraft(snapshot: DraftSaveSnapshot, row: Message) {
  const { args } = snapshot
  const from = args.from || accounts$.peek().find((account) => account.id === args.accountId)?.email
  const normalizeBody = (body: string) => body.replace(/\r\n/g, '\n').replace(/\n+$/, '')
  if (
    row.body_missing ||
    decryptFailed(row.crypto) ||
    row.body_html?.trim() ||
    row.bcc?.trim() ||
    row.reply_to?.trim() ||
    normalizeBody(row.body ?? '') !== normalizeBody(args.content) ||
    row.subject.trim() !== args.subject.trim() ||
    addressesOf(row.from_addr) !== addressesOf(from) ||
    addressesOf(row.to) !== addressesOf(args.to) ||
    addressesOf(row.cc) !== addressesOf(args.cc) ||
    !!row.crypto?.encrypted !== !!args.encrypt ||
    (row.draft_sign ?? undefined) !== args.sign
  )
    return false

  const expected = snapshot.loading ? [...(await snapshot.loading.files), ...args.attachments] : args.attachments
  if (expected.length === 0) return !row.has_attachments
  const manifest = row.attachments ?? []
  if (manifest.length !== expected.length) return false
  if (
    manifest.some((attachment, index) => {
      const previous = expected[index]
      return (
        attachment.filename !== previous.filename ||
        (attachment.mime && attachment.mime !== previous.mime) ||
        (attachment.size && attachment.size !== previous.size)
      )
    })
  )
    return false
  const attachments = await readComposerAttachments(manifest, '')
  // A temporarily unreadable attachment isn't evidence of an external edit.
  if (attachments.length !== expected.length) return true
  return attachments.every((attachment, index) => {
    const previous = expected[index]
    return (
      attachment.filename === previous.filename &&
      attachment.mime === previous.mime &&
      attachment.data === previous.data
    )
  })
}

/** A cached page can arrive before the new read. Only the latter can tell us
 * that a previously visible draft was replaced or removed while we were away. */
export function beginQuickReplyDraftRead(threadId: string) {
  const restored = restoredQuickReply
  if (!restored || restored.reply.snapshot.threadId !== threadId) return
  const { snapshot, row } = restored.reply
  const { identity } = snapshot
  if (
    !identity.saved ||
    identity.failed ||
    writingQuickReplyDraft?.id === identity.id ||
    [...queuedQuickReplySaves].some((request) => request.outgoing?.identity === identity)
  )
    return

  if (!restored.validation) {
    let resolve!: () => void
    const promise = new Promise<void>((done) => {
      resolve = done
    })
    restored.validation = { promise, resolve }
  }
  const validation = restored.validation
  const read = {}
  restored.read = read
  return async (messages?: Message[]) => {
    if (restoredQuickReply !== restored || restored.read !== read) return
    const current = messages ? draftRow(messages, identity.id) : undefined
    const replaced = current && current.id !== row?.id && !(await matchesRestoredDraft(snapshot, current))
    if (restoredQuickReply !== restored || restored.read !== read) return
    restored.read = undefined
    restored.validation = undefined
    validation.resolve()
    if (!messages) return
    // A save can precede the Drafts sync that makes it visible in this
    // conversation. Only a previously observed row can be inferred removed.
    if (!current && !row) return
    restoredQuickReply = null
    if (!replaced && current) return
    if (compose$.quickReplyDraftId.peek() !== identity.id) return
    clearQuickReplyDraftOwnership()
    showToast(t('composer.status.draftChangedElsewhere'))
    if (!restored.reply.unchanged()) return
    compose$.quickReplyFrom.set('')
    compose$.quickReplyAdded.set([])
    startFreshQuickReply()
  }
}

function snapshotQuickReplyDraft(threadId: string): DraftSaveSnapshot | null {
  if (!threadId) return null
  const activeT =
    getActiveThread(threadId) ??
    (threadId !== ui$.selectedThread.peek()
      ? (mail$.messages.peek().find((message) => message.thread_id === threadId) ??
        threadData$[threadId].messages.peek()?.[0])
      : null)
  if (!activeT || (isQuickReplyBlank() && !loadingQuickReplyAttachments)) return null
  const text = quickReplyOutgoingText(activeT)
  const attachments = compose$.composerAttachments.peek().slice()

  const replyAccountId = activeT.account_id || ui$.selectedAccount.peek()
  if (!replyAccountId || replyAccountId === 'unified') return null
  const accounts = accounts$.peek()
  const activeAcc = accounts.find((acc) => acc.id === replyAccountId) || accounts[0] || null
  if (activeAcc?.provider === 'rss' || activeAcc?.auth_type === 'rss' || isLocalAccountId(replyAccountId)) return null

  const target = pickReplyTarget(activeT)
  const { to, cc } = quickReplyAddressing(target)
  const { in_reply_to, references } = buildReplyThreading(target)
  const fromEmail = resolveQuickReplyFrom(target, activeAcc)
  const subject = replySubject(activeT.subject, target)
  const boxDraftId = compose$.quickReplyDraftId.peek()
  const draftId = boxDraftId && !draftClaimedElsewhere(boxDraftId) ? boxDraftId : newDraftMessageId()
  const pendingIdentity = [...queuedQuickReplySaves].find((request) => request.outgoing?.identity.id === draftId)
    ?.outgoing?.identity
  const identity =
    writingQuickReplyDraft?.id === draftId
      ? writingQuickReplyDraft
      : pendingIdentity?.id === draftId
        ? pendingIdentity
        : { id: draftId, saved: boxDraftId === draftId && compose$.quickReplyDraftSaved.peek() }
  ownQuickReplyDraft(identity.id, threadId, identity.saved)
  return {
    threadId,
    identity,
    loading: loadingQuickReplyAttachments,
    startTick: nextQuickReplyOwnershipTick(),
    args: {
      accountId: replyAccountId,
      from: fromEmail,
      to,
      cc,
      subject,
      rich: false,
      content: text,
      inReplyTo: in_reply_to,
      references,
      attachments,
      encrypt: isSendableAccount(activeAcc) && quickReplyEncrypts(target),
      sign: isSendableAccount(activeAcc) ? quickReplySigns(target) : undefined,
    },
  }
}

async function performQuickReplyDraftSave(snapshot: DraftSaveSnapshot) {
  const { threadId, identity, args, loading, startTick } = snapshot
  const replyAccountId = args.accountId
  const draftId = identity.id
  let savedDraftId = draftId
  let confirmed = true
  writingQuickReplyDraft = identity
  try {
    const attachments = loading ? [...(await loading.files), ...args.attachments] : args.attachments
    savedDraftId = await saveComposedDraft({ ...args, draftMessageId: draftId, attachments })
    unconfirmedDraftIds.delete(normalizeMessageId(savedDraftId))
    identity.failed = false
  } catch (error) {
    console.error('Quick reply draft autosave failed:', error)
    identity.failed = true
    if (departingQuickReplies.get(threadId)?.snapshot.identity === identity) {
      showToast(contextualErrorMessage(error, t('composer.status.draftAutosaveFailed')), 'error')
    }
    const attemptedId = draftIdOfFailedSave(error)
    if (!attemptedId || attemptedId === draftId) return
    savedDraftId = attemptedId
    confirmed = false
    unconfirmedDraftIds.add(normalizeMessageId(savedDraftId))
  } finally {
    writingQuickReplyDraft = null
  }

  identity.id = savedDraftId
  identity.saved = confirmed
  if (compose$.quickReplyDraftId.peek() === draftId && savedDraftId !== draftId) {
    compose$.quickReplyDraftId.set(savedDraftId)
  }

  // A send that took the box *after* this save started consumed the very text it
  // just wrote; hand the id over and let the post-send discard drop the copy.
  // Without this the draft is left in Drafts beside the message it was sent as.
  if (handOverToOvertakingSend(threadId, startTick, savedDraftId, confirmed)) return

  // Same story for an escalation that couldn't hand this id to the full editor:
  // the tab is editing a draft of its own, so this copy is an orphan whoever is
  // typing in the box by now.
  const abandonedAt = quickReplyAbandonedSaves.get(threadId) ?? 0
  if (abandonedAt > startTick) {
    quickReplyAbandonedSaves.delete(threadId)
    if (compose$.quickReplyDraftId.peek() === savedDraftId) {
      clearQuickReplyDraftOwnership()
    }
    await discardSavedDraftCopy({
      threadId: threadId,
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
  const sameThread = ui$.selectedThread.peek() === threadId
  const leftWithReply = quickReplyLeftWithReplyTick > startTick
  // ...unless something else took this exact draft over in the meantime.
  if (sameThread && !leftWithReply && isQuickReplyBlank() && !draftClaimedElsewhere(savedDraftId)) {
    if (compose$.quickReplyDraftId.peek() === savedDraftId) {
      clearQuickReplyDraftOwnership()
    }
    await discardSavedDraftCopy({
      threadId: threadId,
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
  const threadId = ui$.selectedThread.peek()
  const scope = quickReplyScope
  const pending = waitForQuickReplyRead()
  if (pending) await pending
  if (scope !== quickReplyScope) return
  // Wait out any in-flight autosave before peeking the flags below; the save
  // itself re-checks on completion and self-discards if the composer is blank.
  while (quickReplyDraftSaveInFlight) await quickReplyDraftSaveInFlight
  if (ui$.selectedThread.peek() !== threadId || !isQuickReplyBlank()) return
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

// The tick at which the reader last left a thread with a reply still in the box
// (Esc on a board saves it just before). A save that started earlier and finds
// the box blank on landing, back on that thread, sees what leaving cleared, not
// a reply the user deleted, and must keep the copy it wrote.
let quickReplyLeftWithReplyTick = 0
// Invalidate asynchronous UI actions when the foreground reply changes.
export let quickReplyScope = 0
compose$.activeTab.onChange(() => {
  quickReplyScope += 1
})
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
compose$.quickReplyAdded.onChange(() => {
  quickReplyBoxGeneration += 1
})
ui$.selectedThread.onChange(() => {
  quickReplyScope += 1
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

/** Flush the debounce before closing the board pane. A queued save can
 * capture the outgoing reply if the pane closes while it is still waiting. */
export async function saveQuickReplyDraftNow(): Promise<void> {
  const pending = quickReplyDraftSaveTimer !== null
  cancelQuickReplyDraftSave()
  const previous = quickReplyDraftSaveInFlight
  if (pending && (!isQuickReplyBlank() || loadingQuickReplyAttachments)) void saveQuickReplyDraft()
  await previous
}

/** Copy one of the people Reply all reaches on the quick reply (picked from
 * its recipient line). A reply in progress is saved again, so its draft is
 * addressed the same way. */
export function addToQuickReply(entry: string) {
  const addr = bareAddr(entry)
  if (!addr || compose$.quickReplyAdded.peek().includes(addr)) return
  compose$.quickReplyAdded.set([...compose$.quickReplyAdded.peek(), addr])
  if (!isQuickReplyBlank()) scheduleQuickReplyDraftSave()
}

/** Cancel any pending debounced draft save — used when a send, thread switch,
 * or escalation to the full editor should preempt it. */
export function cancelQuickReplyDraftSave() {
  if (quickReplyDraftSaveTimer) {
    clearTimeout(quickReplyDraftSaveTimer)
    quickReplyDraftSaveTimer = null
  }
}

ui$.selectedThread.onChange(({ value, getPrevious }) => {
  // A failed send keeps its consumed draft suppressed while the user remains
  // in that editor. Once they leave, make the server safety copy available for
  // a later reopen; in-flight sends remain guarded across navigation.
  for (const guard of quickReplySendHydrationGuards.values()) {
    if (!guard.inFlight && guard.threadId !== value) guard.suppressDraft = false
  }
  publishSendingDraftIds()
  const pending = quickReplyDraftSaveTimer !== null
  cancelQuickReplyDraftSave()
  if (quickReplyDraftBelongsToThread(value)) return
  const outgoingThread = getPrevious()
  const queued = [...queuedQuickReplySaves].filter(
    (request) => request.threadId === outgoingThread && !request.outgoing && !request.superseded,
  )
  const outgoing = snapshotQuickReplyDraft(outgoingThread)
  if (outgoing) {
    rememberDepartingQuickReply(outgoing)
    if (pending || queued.length > 0) {
      const last = queued.pop()
      for (const request of queued) request.superseded = true
      if (last) last.outgoing = outgoing
      else void queueQuickReplyDraftSave({ threadId: outgoingThread, outgoing })
    }
  }
  // Saves started before navigation must keep their result even if the user
  // returns to a blank box on the same thread before the write lands.
  if (!isQuickReplyBlank() || loadingQuickReplyAttachments) quickReplyLeftWithReplyTick = nextQuickReplyOwnershipTick()
  clearQuickReplyDraftOwnership()
  compose$.quickReplyFrom.set('')
  compose$.quickReplyAdded.set([])
  startFreshQuickReply()
  const returning = departingQuickReplies.get(value)
  if (returning) {
    departingQuickReplies.delete(value)
    const draftId = returning.snapshot.identity.id
    if (!draftClaimedElsewhere(draftId) && !sentDraftDiscards.has(normalizeMessageId(draftId))) {
      returning.restore()
      restoredQuickReply = { reply: returning }
      if (returning.snapshot.identity.failed) scheduleQuickReplyDraftSave()
    }
  }
})

function rememberDepartingQuickReply(snapshot: DraftSaveSnapshot) {
  const text = compose$.composer.peek()
  const from = compose$.quickReplyFrom.peek()
  const added = compose$.quickReplyAdded.peek()
  const signatureOff = compose$.quickReplySignatureOff.peek()
  const signatureUnmanaged = compose$.quickReplySignatureUnmanaged.peek()
  const encrypt = compose$.quickReplyEncrypt.peek()
  const sign = compose$.quickReplySign.peek()
  const row =
    draftRow(
      mail$.messages.peek().filter((message) => message.thread_id === snapshot.threadId),
      snapshot.identity.id,
    ) ?? draftRow(threadData$[snapshot.threadId].messages.peek() ?? [], snapshot.identity.id)
  departingQuickReplies.set(snapshot.threadId, {
    snapshot,
    row,
    unchanged: () => {
      const attachments = compose$.composerAttachments.peek()
      return (
        compose$.composer.peek() === text &&
        compose$.quickReplyFrom.peek() === from &&
        compose$.quickReplyAdded.peek().join('\u001f') === added.join('\u001f') &&
        compose$.quickReplySignatureOff.peek() === signatureOff &&
        compose$.quickReplySignatureUnmanaged.peek() === signatureUnmanaged &&
        compose$.quickReplyEncrypt.peek() === encrypt &&
        compose$.quickReplySign.peek() === sign &&
        attachments.length === snapshot.args.attachments.length &&
        attachments.every((attachment, index) => attachment.id === snapshot.args.attachments[index].id)
      )
    },
    restore: () => {
      const { id, saved } = snapshot.identity
      ownQuickReplyDraft(id, snapshot.threadId, saved)
      compose$.composer.set(text)
      compose$.composerAttachments.set(snapshot.args.attachments.slice())
      compose$.quickReplyFrom.set(from)
      compose$.quickReplyAdded.set(added)
      compose$.quickReplySignatureOff.set(signatureOff)
      compose$.quickReplySignatureUnmanaged.set(signatureUnmanaged)
      compose$.quickReplyEncrypt.set(encrypt)
      compose$.quickReplySign.set(sign)
      if (snapshot.loading) loadQuickReplyAttachments(snapshot.loading)
    },
  })
}

const addressesOf = (raw: string | undefined) =>
  parseAddressList(raw)
    .map((item) => item.email.toLowerCase())
    .sort()
    .join(' ')

// The quick reply saves plain text under recipients and a subject worked out
// from the thread, so a draft it can't reproduce must stay out of the box: its
// next autosave would flatten the body and redirect the reply. That is a draft
// the full composer kept with Esc, or another client wrote. It stays a bubble,
// which opens in the full composer. Returns the people the box would have
// added from its recipient line (none for a plain reply).
function quickReplyCanHoldDraft(draft: Message): { added: string[] } | null {
  if (draft.body_html?.trim() || draft.bcc?.trim() || draft.reply_to?.trim()) return null
  const activeT = getActiveThread()
  if (!activeT) return { added: [] }
  const subject = activeT.subject.startsWith('Re:') ? activeT.subject : `Re: ${activeT.subject}`
  if ((draft.subject ?? '').trim() !== subject.trim()) return null
  const target = pickReplyTarget(activeT)
  // The extras the draft copies beyond the plain reply, if all are people
  // Reply all would reach; then the box can address it the same way.
  const plainCc = new Set(splitAddressList(buildReplyRecipients(target).cc).map(bareAddr))
  const added = splitAddressList(draft.cc)
    .map(bareAddr)
    .filter((addr) => !plainCc.has(addr))
  const { to, cc } = addressQuickReply(target, added)
  if (addressesOf(draft.to) !== addressesOf(to) || addressesOf(draft.cc) !== addressesOf(cc)) return null
  return { added }
}

// Pre-fills the quick reply with an already-saved draft sitting at the tail of
// the active thread, so the user can continue and send it inline instead of
// being forced into the full editor. No-op when the tail message isn't a
// draft, or is already the one loaded.
// Back to the box: the draft its hydration passed over while the setting had
// no box takes its place in it now, as it would have on load.
settings$.conversationFooter.onChange(({ value }) => {
  if (value === 'reply') hydrateQuickReplyFromTailDraft(mail$.messages.peek())
})

function hydrateQuickReplyFromTailDraft(messages: Message[]) {
  if (threadWindowSession) return
  // No box to fill: the conversation-footer setting put buttons, or nothing,
  // there. The draft stays a bubble in the conversation, to open from there.
  if (settings$.conversationFooter.peek() !== 'reply') return
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
  // Nor one whose body couldn't be fetched or decrypted: the box would show
  // empty while holding the draft's id, and the first keystroke's autosave
  // would replace a draft the user never saw.
  if (tail.body_missing || decryptFailed(tail.crypto)) return
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
  const holds = quickReplyCanHoldDraft(tail)
  if (!holds) return
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

  compose$.composerAttachments.set([])
  // A draft saved encrypted comes back encrypted, whatever the conversation's
  // default: its text must not go out (or be saved again) in the clear.
  if (tail.crypto?.encrypted) compose$.quickReplyEncrypt.set({ threadId: activeThreadId, on: true, draft: true })
  if (tail.draft_sign != null) {
    compose$.quickReplySign.set({ threadId: activeThreadId, on: tail.draft_sign })
    // The sign switch is only set with encryption off: a plain draft that
    // carries it was switched off on purpose (signed only, or plain), and
    // stays off rather than being encrypted again by default.
    if (!tail.crypto?.encrypted) compose$.quickReplyEncrypt.set({ threadId: activeThreadId, on: false, byUser: true })
  }
  ownQuickReplyDraft(tailDraftId, activeThreadId, true)
  compose$.quickReplyFrom.set(tail.from_addr ?? '')
  compose$.quickReplyAdded.set(holds.added)
  if (tail.has_attachments) {
    loadQuickReplyAttachments({ files: readComposerAttachments(tail.attachments ?? [], tail.body_html ?? '') })
  }
  loadDraftBodyIntoQuickReply(activeThreadId, tail.body ?? '')
}

function loadQuickReplyAttachments(loading: LoadingQuickReplyAttachments) {
  loadingQuickReplyAttachments = loading
  void loading.files.then((attachments) => {
    if (loadingQuickReplyAttachments !== loading) return
    loadingQuickReplyAttachments = null
    if (attachments.length > 0) {
      compose$.composerAttachments.set((added) => [...attachments, ...added])
    }
  })
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
// A closed tab gets one quick retry, for a connection that was only slow: the
// user just asked for this, so a failure has to reach them while they still
// remember doing it.
const CLOSED_DRAFT_DISCARD_RETRY_DELAYS_MS = [5_000]

/**
 * Discard the draft behind a message that was just sent. Resolves with whether
 * the first attempt removed it; a failed one keeps the draft suppressed and is
 * retried in the background, so callers can settle their own state either way.
 */
export async function discardSentDraft(
  draft: SentDraft,
  retryDelays: number[] = SENT_DRAFT_DISCARD_RETRY_DELAYS_MS,
): Promise<boolean> {
  const id = normalizeMessageId(draft.draftMessageId)
  if (id) {
    for (const [threadId, reply] of departingQuickReplies) {
      if (normalizeMessageId(reply.snapshot.identity.id) === id) departingQuickReplies.delete(threadId)
    }
  }
  // A row known only by its local id can be neither suppressed nor retried by
  // Message-ID: one plain attempt, failing out loud.
  if (!id) return discardSavedDraftCopy(draft)
  sentDraftDiscards.set(id, draft)
  publishSendingDraftIds()
  const failure = await attemptSentDraftDiscard(id, draft)
  if (!failure) return true
  void retrySentDraftDiscard(id, draft, retryDelays, failure.error)
  return false
}

/** Discard the draft of a compose tab the user closed, as a send does but with
 * a failure reported within about a minute. */
export function discardClosedDraft(draft: SentDraft): Promise<boolean> {
  return discardSentDraft(draft, CLOSED_DRAFT_DISCARD_RETRY_DELAYS_MS)
}

/** Null when the draft is gone. */
async function attemptSentDraftDiscard(id: string, draft: SentDraft): Promise<{ error: unknown } | null> {
  discardingDraftIds.add(id)
  try {
    await discardSavedDraftCopy(draft, { throwOnError: true })
  } catch (error) {
    console.error('Discarding a draft failed:', error)
    return { error }
  } finally {
    discardingDraftIds.delete(id)
  }
  if (sentDraftDiscards.get(id) === draft) {
    sentDraftDiscards.delete(id)
    publishSendingDraftIds()
  }
  return null
}

async function retrySentDraftDiscard(id: string, draft: SentDraft, delays: number[], firstError: unknown) {
  let lastError = firstError
  for (const delay of delays) {
    await new Promise((resolve) => setTimeout(resolve, delay))
    if (sentDraftDiscards.get(id) !== draft) return
    // Reopened in the full editor meanwhile: it is the user's draft again.
    if (draftOpenInComposeTab(id)) {
      sentDraftDiscards.delete(id)
      publishSendingDraftIds()
      return
    }
    const failure = await attemptSentDraftDiscard(id, draft)
    if (!failure) return
    lastError = failure.error
  }
  // Still suppressed, so it cannot come back into the quick reply this session;
  // the Drafts folder is where the user can remove it.
  showToast(contextualErrorMessage(lastError, t('composer.status.couldNotDiscardDraft')), 'error')
}

/** Whether a full-editor tab is editing this server-side draft, or about to be:
 * an escalated quick reply hands its id over, and the tab is its owner from then
 * on. Counts a draft the user has clicked but whose tab hasn't opened yet. */
export function draftOpenInComposeTab(draftId: string): boolean {
  if (!draftId) return false
  const id = normalizeMessageId(draftId)
  if (normalizeMessageId(externalComposeDraft$.peek()) === id || externalThreadDrafts$.peek().includes(id)) return true
  if (openingComposeTabDraftIds.has(id)) return true
  // Its tab has closed, but the Esc save is still writing what this copy is
  // about to become.
  if (draftSaveRunning((draft) => normalizeMessageId(draft.draftMessageId) === id)) return true
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

/**
 * Put a saved draft's body in the box. The body is the reply as it would go
 * out, so it may end in a signature: when that is the replying account's
 * signature, exactly and as the last block, it is taken off and the switch is
 * on, so sending adds the same signature back where it was. Anything else is
 * left as written with the switch off: switchable when the draft holds no
 * trace of the signature (it was saved with it off), not when it holds an
 * edited or older copy, which switching on would put a second one under
 * (#149; quickReplyDraftSignature).
 */
function loadDraftBodyIntoQuickReply(threadId: string, body: string) {
  watchQuickReplySignatureSources()
  const signature = quickReplySignature()
  const { text, state } = quickReplyDraftSignature(body, signature.text, signature.html)
  compose$.composer.set(text)
  setHydratedSignatureState(state)
  hydratedWithoutSignature = state === 'on' ? null : { threadId, body }
}

function setHydratedSignatureState(state: 'on' | 'off' | 'unmanaged') {
  compose$.quickReplySignatureOff.set(state !== 'on')
  compose$.quickReplySignatureUnmanaged.set(state === 'unmanaged')
}

// A draft loaded with no signature recognised, while the box still holds it as
// loaded. The app signature is read out of the prefs table after the first
// render, and an account's override can change in Settings, so a draft opened
// before either lands is looked at again when they do.
let hydratedWithoutSignature: { threadId: string; body: string } | null = null

function recheckHydratedDraftSignature() {
  const hydrated = hydratedWithoutSignature
  if (!hydrated) return
  if (ui$.selectedThread.peek() !== hydrated.threadId || compose$.composer.peek() !== hydrated.body) {
    hydratedWithoutSignature = null
    return
  }
  const signature = quickReplySignature()
  const { text, state } = quickReplyDraftSignature(hydrated.body, signature.text, signature.html)
  setHydratedSignatureState(state)
  if (state !== 'on') return
  hydratedWithoutSignature = null
  compose$.composer.set(text)
}

// Subscribed on the first fresh box rather than at module scope: this module
// and ./accounts import each other, so `accounts$` is still in its temporal
// dead zone while this file's body runs.
let quickReplySignatureSourcesWatched = false

function watchQuickReplySignatureSources() {
  if (quickReplySignatureSourcesWatched) return
  quickReplySignatureSourcesWatched = true
  settings$.signature.onChange(recheckHydratedDraftSignature)
  accounts$.onChange(recheckHydratedDraftSignature)
}
