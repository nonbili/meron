import { t } from '../lib/i18n'
import type { Attachment, ComposerAttachment, Message } from '../types'
import { invoke } from '../lib/bridge'
import { ui$, showToast } from './ui'
import { accounts$ } from './accounts'
import { mail$, getActiveThread, loadThread, normalizeMessageId } from './mail'
import { isDraftFolder } from './mailFolders'
import { discardSavedDraftCopy } from './mailMoves'
import {
  LOCAL_SEND_PREFIX,
  type PendingSend,
  setPendingSend,
  getPendingSend,
  discardPendingSend,
  setUnsentRescue,
  getUnsentRescue,
  discardUnsentRescue,
} from './pendingSends'
import { offerCertificateTrust } from './certificateTrust'
import { draftIdOfFailedSave, escapeHtml, saveComposedDraft, textToHtml } from './compose'
import { buildReplyRecipients, buildReplyThreading, pickReplyTarget, resolveQuickReplyFrom } from './composeReply'
import { allocateMessageIdentity, compose$, newDraftMessageId } from './composeState'
import {
  cancelQuickReplyDraftSave,
  clearQuickReplyDraftOwnership,
  discardSentDraft,
  draftOpenInComposeTab,
  isQuickReplyBlank,
  pendingQuickReplySendClaims,
  publishSendingDraftIds,
  quickReplyBoxGeneration,
  quickReplyDraftSaveInFlight,
  quickReplyOutgoingText,
  nextQuickReplyOwnershipTick,
  type QuickReplySendClaim,
  quickReplySendHydrationGuards,
  releaseUnsentQuickReplyClaim,
  seedQuickReplySignature,
  unconfirmedDraftIds,
  unconfirmedQuickReplyDraftId,
} from './quickReply'

// Sending from the quick-reply composer: optimistic bubbles, dispatch, retry,
// and draft cleanup.

const newInlineImageId = () => `meron-image-${Date.now()}-${Math.random().toString(36).substring(2, 9)}@meron`

function prepareConversationAttachments(attachments: ComposerAttachment[]) {
  let hasInlineImages = false
  const prepared = attachments.map((attachment) => {
    if (!attachment.mime.toLowerCase().startsWith('image/')) return attachment
    hasInlineImages = true
    return {
      ...attachment,
      inlineId: attachment.inlineId || newInlineImageId(),
    }
  })
  return { attachments: prepared, hasInlineImages }
}

function conversationHtmlBody(text: string, attachments: ComposerAttachment[]): string {
  const images = attachments.filter((a) => a.inlineId && a.mime.toLowerCase().startsWith('image/'))
  if (images.length === 0) return ''
  const body = textToHtml(text)
  const imageHtml = images
    .map((image) => `<p><img src="cid:${escapeHtml(image.inlineId ?? '')}" alt="${escapeHtml(image.filename)}"></p>`)
    .join('')
  return `${body}${imageHtml}`
}

export async function sendReply() {
  const composerText = quickReplyOutgoingText()
  const attachments = compose$.composerAttachments.get()
  const activeT = getActiveThread()
  const selectedAcc = ui$.selectedAccount.get()

  // A box holding only the seeded signature is not a reply worth sending.
  if (isQuickReplyBlank() || !activeT) return

  const replyAccountId = activeT.account_id || selectedAcc
  const accounts = accounts$.get()
  const activeAcc = accounts.find((acc) => acc.id === replyAccountId) || accounts[0] || null
  if (!replyAccountId || replyAccountId === 'unified') return
  if (activeAcc?.provider === 'rss' || activeAcc?.auth_type === 'rss') return

  // Claim the box now, synchronously: the thread load and identity allocation
  // below are awaits the user can keep typing through, and an autosave that
  // starts in that window is writing text this send never took. Stamping the
  // claim at the click keeps such a save out of the handover below — and takes
  // the draft id as it stood at the click for the same reason. Only a draft that
  // reached the server: a save still on the wire has nothing to discard yet, so
  // it hands its id over when it lands (see performQuickReplyDraftSave).
  const claimTick = nextQuickReplyOwnershipTick()
  const boxGeneration = quickReplyBoxGeneration
  const matchingLoadedDraft = mail$.messages
    .peek()
    .filter(
      (message) =>
        message.thread_id === activeT.thread_id &&
        isDraftFolder(message.folder_id, message.account_id) &&
        !!message.message_id &&
        (message.body ?? '') === composerText,
    )
    .sort((left, right) => right.date - left.date)[0]
  const claim: QuickReplySendClaim = {
    threadId: activeT.thread_id,
    tick: claimTick,
    // Hydration normally owns the draft explicitly. Fall back to the loaded row
    // only when it is unambiguously this reply: ownership can be lost across a
    // component remount, but leaving the matching copy behind renders the sent
    // message twice and keeps the thread's Draft badge set.
    draftId: compose$.quickReplyDraftSaved.peek()
      ? compose$.quickReplyDraftId.peek()
      : (matchingLoadedDraft?.message_id ?? ''),
    // A copy a failed autosave may have left: cleaned up once the reply is out,
    // but never taken for the safety net above.
    unconfirmedDraftId: '',
    generation: boxGeneration,
  }
  const unconfirmedBoxDraftId = unconfirmedQuickReplyDraftId()
  if (unconfirmedBoxDraftId && normalizeMessageId(unconfirmedBoxDraftId) !== normalizeMessageId(claim.draftId)) {
    claim.unconfirmedDraftId = unconfirmedBoxDraftId
  }
  pendingQuickReplySendClaims.push(claim)
  // The draft leaves the box with the send, right now rather than at the guard:
  // anything the user does during the awaits below — typing on, escalating to
  // the full editor — must start from a draft of its own, not from the copy this
  // send is about to discard. Releasing the claim hands it back.
  if (claim.draftId) {
    clearQuickReplyDraftOwnership()
    // The box stops hiding the draft the moment it lets go of it, so the claim
    // has to take that over now rather than when the guard appears — otherwise a
    // slow allocation shows the draft beside the reply being sent.
    publishSendingDraftIds()
  }
  // The debounced save of the text being sent is redundant now, and if it fired
  // during the awaits below it would land a draft with nothing left pointing at
  // it — the original stranded-draft symptom. Typing on re-arms it.
  cancelQuickReplyDraftSave()

  const text = composerText
  const prepared = prepareConversationAttachments(attachments)
  const sendAttachments = prepared.attachments
  const html = prepared.hasInlineImages ? conversationHtmlBody(text, sendAttachments) : ''
  const subject = activeT.subject.startsWith('Re:') ? activeT.subject : `Re: ${activeT.subject}`
  const tempId = `${LOCAL_SEND_PREFIX}${Date.now()}`
  // Recipients and threading follow the reply target, which the thread load
  // below can change; the From indicator's pick, else the alias the original
  // was delivered to. Ignore this send's own bubble after it is inserted: in a
  // sent-only thread, pickReplyTarget otherwise falls back to that newest local
  // message while it is still waiting for its Message-ID.
  const addressReply = () => {
    const target = pickReplyTarget(activeT, tempId)
    const { to, cc } = buildReplyRecipients(target)
    const { in_reply_to, references } = buildReplyThreading(target)
    return { to, cc, in_reply_to, references, from: resolveQuickReplyFrom(target, activeAcc) }
  }
  let addressed = addressReply()

  // Render the sent bubble optimistically — right at the click, before the
  // thread load, the identity allocation and the SMTP round-trip below — so
  // sending feels instant. The first two can take seconds (the load fetches
  // Message-IDs over IMAP), and a reply that vanishes from the box only to
  // reappear later reads as lost. The bubble starts without a Message-ID and
  // with the recipients as they stand now; both are patched in once known. Its
  // status starts as "sending" and flips to "sent" or "failed" once the backend
  // responds.
  const sent: Message = {
    id: tempId,
    account_id: replyAccountId,
    folder_id: activeT.folder_id,
    thread_id: activeT.thread_id,
    message_id: '',
    references: addressed.references,
    from_name: 'You',
    from_addr: addressed.from || activeAcc?.email || '',
    to: addressed.to,
    cc: addressed.cc,
    subject,
    preview: text || (sendAttachments.length > 0 ? `[Attachment: ${sendAttachments[0].filename}]` : ''),
    body: text,
    date: Math.floor(Date.now() / 1000),
    unread: false,
    starred: false,
    has_attachments: sendAttachments.length > 0,
    send_status: 'sending',
    // Inline the just-sent attachment bytes as a data: URL so the chat bubble
    // can display the image immediately — without waiting for IMAP sync to
    // pull the message back from Sent and assign it a media key. Treated as
    // local media by the renderer (no remote-image gate).
    attachments: sendAttachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      size: a.size,
      key: null,
      url: a.mime.startsWith('image/') || a.mime.startsWith('video/') ? `data:${a.mime};base64,${a.data}` : null,
    })),
  }
  mail$.messages.push(sent)

  // Clear the composer optimistically, in the same tick as the click: nothing
  // has happened since that could make the box anyone else's. On a failed SMTP
  // send the message stays in the pane with a "failed" status (retry from the
  // bubble or delete from the context menu), so we don't restore the draft.
  // Retry replays the stored PendingSend payload below, not the (now-cleared)
  // composer text, so clearing here doesn't affect retry.
  const box = {
    text: compose$.composer.peek(),
    attachments,
    from: compose$.quickReplyFrom.peek(),
    signature: compose$.quickReplySignature.peek(),
  }
  compose$.composerAttachments.set([])
  clearQuickReplyDraftOwnership()
  // Back to a fresh quick reply rather than an empty box: the next reply in
  // this thread gets a signature just like the one just sent did.
  seedQuickReplySignature()
  const clearedGeneration = quickReplyBoxGeneration

  const dropUnsentBubble = () => {
    const index = mail$.messages.peek().findIndex((message) => message.id === tempId)
    if (index >= 0) mail$.messages.splice(index, 1)
  }
  // Nothing went out and the box is someone else's now, so the reply cannot go
  // back where it came from — and without a claimed draft the bubble is its
  // only copy. Write it out as a draft of its own, and keep the bubble standing
  // (failed, so it doesn't read as sent) until that lands: a rescue that fails
  // too — the identity allocation this save needs is often the very thing that
  // just failed — must not take the last copy down with it. The bubble's Retry
  // then re-runs this, so the reply is recoverable once the backend is back.
  // One id across the rescue's attempts: a failed save may have written its copy
  // anyway, and a retry under a fresh id would put the reply in Drafts twice.
  // The same goes for a copy an autosave of this reply may have left.
  let rescueDraftId = ''
  const rescueUnsentQuickReply = async (): Promise<void> => {
    // Registered before the first await (see abortUnsent), so deleting the
    // bubble mid-rescue is seen here rather than missing the registration and
    // resurrecting it afterwards.
    const rescue = getUnsentRescue(tempId)
    if (!rescue || rescue.cancelled) return
    rescue.inFlight = true
    let savedDraftId = ''
    try {
      // An autosave that started before the click may still be writing this
      // very reply; it hands its id to the claim when it lands. Wait for that
      // rather than racing it — a second draft alongside the one it saves is
      // the same reply twice in Drafts. The claim has to stay in the pending
      // list until then for the handover to find it.
      while (quickReplyDraftSaveInFlight) await quickReplyDraftSaveInFlight
      if (!claim.draftId) {
        savedDraftId = await saveComposedDraft({
          accountId: replyAccountId,
          from: addressed.from,
          to: addressed.to,
          cc: addressed.cc,
          subject,
          rich: false,
          content: text,
          inReplyTo: addressed.in_reply_to,
          references: addressed.references,
          draftMessageId: rescueDraftId || claim.unconfirmedDraftId || newDraftMessageId(),
          attachments: box.attachments,
        })
        unconfirmedDraftIds.delete(normalizeMessageId(savedDraftId))
      }
    } catch (saveError) {
      console.error('Failed to save unsent quick reply as a draft:', saveError)
      rescueDraftId = draftIdOfFailedSave(saveError) ?? rescueDraftId
      if (rescueDraftId) unconfirmedDraftIds.add(normalizeMessageId(rescueDraftId))
      rescue.inFlight = false
      // The bubble is still the reply: say so, since the thrown send error goes
      // nowhere the user can see. Its Retry runs this again.
      if (!rescue.cancelled) showToast(t('compose.toast.sendFailed'), 'error')
      return
    } finally {
      // Whether or not a copy reached the server, the claim is done: left in
      // the pending list it would keep hiding its draft and could adopt the id
      // of a save for the reply the user has moved on to.
      releaseUnsentQuickReplyClaim(claim)
    }
    discardUnsentRescue(tempId)
    dropUnsentBubble()
    // The user deleted the bubble while this was on the wire — they threw the
    // reply away, so the copy it just wrote (its own, or the one the autosave
    // handed over) goes with it.
    const strandedDraftId = rescue.cancelled ? savedDraftId || claim.draftId : ''
    if (strandedDraftId) {
      await discardSavedDraftCopy({
        threadId: activeT.thread_id,
        messageId: '',
        folderId: '',
        accountId: replyAccountId,
        draftMessageId: strandedDraftId,
      })
    }
  }

  // A send that dies before anything went out takes its bubble back down and
  // puts the reply back in the box — unless the user has typed or navigated
  // since, in which case the box is theirs and the reply lives on in a draft:
  // the one the claim took, or the one the rescue writes for it.
  const abortUnsent = (error: unknown): unknown => {
    if (quickReplyBoxGeneration === clearedGeneration) {
      compose$.composer.set(box.text)
      compose$.composerAttachments.set(box.attachments)
      compose$.quickReplyFrom.set(box.from)
      compose$.quickReplySignature.set(box.signature)
      // The box holds the claim's reply again, so releasing hands it back.
      claim.generation = quickReplyBoxGeneration
      dropUnsentBubble()
      releaseUnsentQuickReplyClaim(claim)
    } else if (claim.draftId) {
      // The box has moved on, but the reply survives in the draft the claim
      // took and is about to unsuppress — the bubble is redundant.
      dropUnsentBubble()
      releaseUnsentQuickReplyClaim(claim)
    } else {
      // The bubble is the reply until the rescue lands a copy on the server, so
      // it stands as failed and the rescue is registered before it starts:
      // deleting the bubble in that window has to reach the rescue. Releases
      // the claim itself, once the autosave it may be waiting on has had its
      // chance to hand a draft over.
      setSendStatus(tempId, 'failed')
      const bubble = mail$.messages.peek().find((message) => message.id === tempId) ?? sent
      setUnsentRescue(tempId, {
        threadId: activeT.thread_id,
        bubble: { ...bubble, send_status: 'failed' },
        retry: rescueUnsentQuickReply,
        inFlight: false,
        cancelled: false,
      })
      void rescueUnsentQuickReply()
    }
    return error
  }

  // Guarantee the open thread is loaded *with Message-IDs* before choosing a
  // reply target. A message synced from its envelope (e.g. one opened straight
  // from a notification) carries no Message-ID until its body is fetched —
  // upsert_messages persists only the recipient lists. If we reply off such a
  // header-less copy (or fall back to the thread card, which also has no
  // Message-ID), buildReplyThreading produces empty In-Reply-To/References and
  // the reply starts an orphan thread on the recipient's side. Loading the
  // thread runs each message through read_cached_or_fetch, which back-fills the
  // Message-ID. Checking only for *a* loaded message isn't enough — it's
  // satisfied by the header-less copy we need to refetch.
  const hasThreadingTarget = () =>
    mail$.messages.get().some((m) => m.thread_id === activeT.thread_id && (m.message_id || '').trim())
  if (!hasThreadingTarget()) {
    try {
      await loadThread(activeT.thread_id)
    } catch (error) {
      throw abortUnsent(error)
    }
    addressed = addressReply()
  }

  let messageId: string
  try {
    messageId = await allocateMessageIdentity(replyAccountId, false)
  } catch (error) {
    throw abortUnsent(error)
  }
  const payload: PendingSend = {
    account_id: replyAccountId,
    to: addressed.to,
    cc: addressed.cc,
    subject,
    body: text,
    html,
    in_reply_to: addressed.in_reply_to,
    references: addressed.references,
    from: addressed.from,
    message_id: messageId,
    attachments: sendAttachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      data: a.data,
      inline_id: a.inlineId ?? '',
    })),
  }
  // Carry the real Message-ID and References chain on the bubble so a follow-up
  // reply sent before this one syncs back threads against it
  // (buildReplyThreading reads message_id + references) instead of starting a
  // fresh thread. The bubble may be gone by now — a thread switch during the
  // awaits above drops it from the list — in which case there is nothing to
  // patch; the send itself is unaffected.
  const bubbleIndex = mail$.messages.peek().findIndex((message) => message.id === tempId)
  if (bubbleIndex >= 0) {
    mail$.messages[bubbleIndex].assign({
      message_id: messageId,
      references: addressed.references,
      to: addressed.to,
      cc: addressed.cc,
      from_addr: addressed.from || activeAcc?.email || '',
    })
  }
  // From this point the current quick reply belongs to the optimistic send.
  // Keep background thread refreshes from hydrating its still-persisted draft
  // back into the editor until SMTP and the post-send discard settle. This is
  // keyed by the send so navigation cannot accidentally reopen the race.
  // The claim becomes the guard, carrying whatever a save that landed during the
  // allocation above handed to it.
  pendingQuickReplySendClaims.splice(pendingQuickReplySendClaims.indexOf(claim), 1)
  quickReplySendHydrationGuards.set(tempId, {
    threadId: activeT.thread_id,
    accountId: replyAccountId,
    draftId: claim.draftId,
    unconfirmedDraftId: claim.unconfirmedDraftId,
    claimTick,
    inFlight: true,
    suppressDraft: true,
  })
  publishSendingDraftIds()
  setPendingSend(tempId, payload)

  await dispatchSend(tempId)
}

// Set the send lifecycle status on the optimistic message with the given id.
function setSendStatus(tempId: string, status: Message['send_status']) {
  const idx = mail$.messages.get().findIndex((m) => m.id === tempId)
  if (idx >= 0) mail$.messages[idx].send_status.set(status)
}

// Fire the `mail.send` bridge call for a pending message and reconcile its
// status. On success the payload is dropped; on failure it's kept so the user
// can retry. Shared by the initial send and retrySend().
async function dispatchSend(tempId: string) {
  const payload = getPendingSend(tempId)
  if (!payload) return
  const guard = quickReplySendHydrationGuards.get(tempId)
  if (guard) {
    guard.inFlight = true
    guard.suppressDraft = true
    publishSendingDraftIds()
  }
  setSendStatus(tempId, 'sending')
  try {
    await invoke('mail.send', payload)
    discardPendingSend(tempId)
    setSendStatus(tempId, 'sent')
    void finishQuickReplySendLifecycle(tempId)
  } catch (error) {
    settleFailedQuickReplySendGuard(tempId)
    setSendStatus(tempId, 'failed')
    const message = error instanceof Error ? error.message : t('compose.toast.sendFailed')
    // A submission server whose certificate we cannot validate (a local bridge
    // with a self-signed one, or one that rotated since setup) is unreachable
    // until that certificate is pinned. Offer it and retry on acceptance
    // instead of leaving a failed bubble the user can only retry into the same
    // failure.
    if (await offerCertificateTrust(payload.account_id, message, () => dispatchSend(tempId))) return
    showToast(message, 'error')
  }
}

async function finishQuickReplySendLifecycle(tempId: string) {
  const guard = quickReplySendHydrationGuards.get(tempId)
  if (!guard) return
  // An autosave this send overtook is still writing the reply's draft. Let it
  // land — discarding ahead of it would delete nothing and leave the copy
  // behind — and pick up the id it hands over.
  while (quickReplyDraftSaveInFlight) await quickReplyDraftSaveInFlight
  // Both the saved copy and any a failed autosave may have left went out with
  // the reply — except one opened in the full editor while the send was out
  // (from the Drafts list, or after a failure the user retried): it is the
  // user's again, and emptying the editor they are typing in is worse than
  // leaving a draft behind.
  const leftovers = [guard.draftId, guard.unconfirmedDraftId].filter((id) => id && !draftOpenInComposeTab(id))
  await Promise.all(
    leftovers.map((draftMessageId) => {
      unconfirmedDraftIds.delete(normalizeMessageId(draftMessageId))
      return discardSentDraft({
        threadId: guard.threadId,
        messageId: '',
        folderId: '',
        accountId: guard.accountId,
        draftMessageId,
      })
    }),
  )
  if (quickReplySendHydrationGuards.get(tempId) !== guard) return
  // The send is settled either way. A copy the discard couldn't remove yet is
  // not a safety net to hand back like a failed send's: the message went out,
  // so discardSentDraft keeps it suppressed and retries on its own.
  quickReplySendHydrationGuards.delete(tempId)
  publishSendingDraftIds()
}

function settleFailedQuickReplySendGuard(tempId: string) {
  const guard = quickReplySendHydrationGuards.get(tempId)
  if (!guard) return
  guard.inFlight = false
  if (ui$.selectedThread.peek() !== guard.threadId) guard.suppressDraft = false
  publishSendingDraftIds()
}

// Re-attempt a previously failed send, triggered by clicking the failed bubble.
export async function retrySend(messageId: string) {
  // A bubble whose reply never reached the server retries the rescue, not a
  // send that never had a payload. It stays registered throughout — a delete
  // during the attempt must still find it — so a second click is turned away
  // by the in-flight flag instead.
  const rescue = getUnsentRescue(messageId)
  if (rescue) {
    if (!rescue.inFlight) await rescue.retry()
    return
  }
  if (!getPendingSend(messageId)) return
  await dispatchSend(messageId)
}
