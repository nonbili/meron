import { t } from '../lib/i18n'
import { isNarrowWindow } from '../lib/useNarrowWindow'
import type { Account, Attachment, ComposeDraft, ComposerAttachment, Message } from '../types'
import { invoke } from '../lib/bridge'
import { focusQuickReply, ui$, showToast } from './ui'
import { accounts$, isSendableAccount } from './accounts'
import { getActiveThread } from './mail'
import { parseMailto } from '../lib/mailto'
import {
  bodyWithSignature,
  bodyWithSwappedSignature,
  type ComposeBody,
  optedOutMark,
  resolveSignature,
  signatureForms,
  type Signature,
  type SignaturePlacement,
  type SignatureTracking,
  reopenedSignatureTracking,
} from '../lib/signature'
import { settings$ } from './settings'
import {
  actionBarTarget,
  buildReplyRecipients,
  quickReplyAddressing,
  buildReplyThreading,
  detectAliasFrom,
  detectSenderFrom,
  pickReplyTarget,
  quickReplyEncryptOff,
  quickReplyEncrypts,
  quickReplySigns,
  quickReplyHoldsEncryptedDraft,
  quickReplyOffByUser,
  resolveQuickReplyFrom,
  replySubject,
} from './composeReply'
import { compose$, hasExtraComposeHeaders, newDraftMessageId } from './composeState'
import {
  cancelQuickReplyDraftSave,
  clearQuickReplyDraftOwnership,
  quickReplyAbandonedSaves,
  quickReplyBoxShown,
  nextQuickReplyOwnershipTick,
  quickReplyDraftSaveInFlight,
  quickReplyScope,
  waitForQuickReplyRead,
  unconfirmedQuickReplyDraftId,
  startFreshQuickReply,
} from './quickReply'
import { fetchComposeTabAttachments } from './composeDrafts'
import {
  bodyHasReplyQuote,
  forwardedHtmlBody,
  forwardedPlainBody,
  forwardedSubject,
  replyQuote,
  replyQuoteWithheld,
  textToHtml,
} from './composeQuote'
export * from './composeQuote'
export * from './messageTabs'
export * from './composeDrafts'
export * from './composeSend'

// Reader and compose tabs: opening threads and drafts, the full editor, forwarding,
// and saving or sending composed messages.

// From the leaf module: this one subscribes to it while loading, which can be
// inside an import cycle through mail.ts.

/**
 * The signature an account sends, in both body forms. Both are resolved even for
 * a rich draft: the composer can be toggled to plaintext at any time, and a
 * half-tracked signature could then be neither found nor replaced.
 */
export function resolveSignatureFor(account: Account | undefined): Signature {
  return signatureForms(account ? resolveSignature(account, settings$.signature.peek()) : '')
}

/**
 * Move a draft to another From account, swapping the signature it carries for
 * the new account's. Sending account B's mail under account A's signature is
 * worse than no signature at all, so this runs on every account change; an
 * edited signature is left alone (see `bodyWithSwappedSignature`).
 */
export function withSignatureForAccount(draft: ComposeDraft, partial: Partial<ComposeDraft>): Partial<ComposeDraft> {
  if (!partial.accountId || partial.accountId === draft.accountId) return partial
  const account = accounts$.peek().find((acc) => acc.id === partial.accountId)
  // Swap over the draft as this update leaves it, so a body supplied in the
  // same call is what gets the new signature — not the body it replaced.
  const updated = { ...draft, ...partial }
  const swapped = bodyWithSwappedSignature(updated, draft.signature, resolveSignatureFor(account))
  return { ...partial, html: swapped.body.html, text: swapped.body.text, signature: swapped.tracking }
}

let composeSeq = 0

// Open a full-pane compose/reply editor as a new tab. Returns silently if no
// account can send mail. `seed` pre-fills a reply (recipient, subject, body…).
export type ComposeSeed = Partial<ComposeDraft> & {
  title?: string
  threadId?: string
  /** Where the signature lands; 'aboveQuote' for a seeded quote (forwards). */
  signaturePlacement?: SignaturePlacement
  /**
   * The message a reply answers, quoted under the seeded body (see
   * {@link replyQuote}) but folded away until the user unfolds it. An HTML
   * original makes the draft rich.
   */
  quote?: ComposeBody
  /**
   * Skip the signature. Set when the body is an existing message being re-opened
   * (a saved draft, "Edit as New Message") — it already carries whatever
   * signature it was written with, and a second copy is not wanted.
   */
  noSignature?: boolean
  /**
   * Insert no signature, but keep it switchable: the user turned it off where
   * this draft came from (the quick reply), and may turn it back on here.
   */
  signatureOff?: boolean
}

export function openComposeTab(seed?: ComposeSeed): string | undefined {
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  const sendable = accounts$.get().filter(isSendableAccount)
  if (sendable.length === 0) return undefined
  // The composer opens in the mail pane, which the calendar replaces.
  ui$.calendarOpen.set(false)
  const selected = ui$.selectedAccount.get()
  // A seed from a thread in local folders names an account that cannot send;
  // the reply then goes out from the selected or first sendable account.
  const seededAccountId = sendable.some((acc) => acc.id === seed?.accountId) ? seed?.accountId : undefined
  const accountId = seededAccountId ?? sendable.find((acc) => acc.id === selected)?.id ?? sendable[0].id
  // Only a truly blank compose (no seeded body) should pick up the account's
  // HTML/plain preference; reply/forward/mailto paths that seed plain `text`
  // stay plain since the editor only ever hydrates from `html`.
  const hasSeededBody = !!(seed?.text || seed?.html)
  const account = sendable.find((acc) => acc.id === accountId)

  // The signature is inserted into the body up front so it is editable (and
  // visible) like the rest of the draft, rather than appearing at send time.
  const quote = seed?.quote
  const rich = seed?.rich ?? (hasSeededBody ? false : (account?.conversation_html ?? true))
  // An HTML original is quoted as HTML, so the reply goes rich with it.
  const seeded =
    quote?.rich && !rich
      ? { rich: true, html: seed?.html || textToHtml(seed?.text ?? ''), text: '' }
      : { rich, html: seed?.html ?? '', text: seed?.text ?? '' }
  const placement = seed?.signaturePlacement ?? 'belowText'
  const accountSignature = resolveSignatureFor(account)
  const signature = seed?.noSignature || seed?.signatureOff ? undefined : accountSignature
  // The placement is recorded even when the account sends no signature, so a
  // forward that later moves to an account with one still puts it above the
  // quote rather than after it. A body this app did not compose (a saved draft,
  // "Edit as New Message") may already carry a signature: the account's, ending
  // the body as the composer inserts it, is tracked (and switchable); a body
  // with no trace of it is off (and switchable); anything else stays unmanaged,
  // `undefined`, so nothing adds a second (reopenedSignatureTracking).
  const tracking: SignatureTracking = signature
    ? { ...signature, placement }
    : seed?.signatureOff
      ? optedOutMark(placement)
      : reopenedSignatureTracking(seeded, accountSignature)
  const body = signature ? bodyWithSignature(seeded, signature, placement) : seeded

  const draft: ComposeDraft = {
    accountId,
    fromEmail: seed?.fromEmail ?? '',
    to: seed?.to ?? '',
    cc: seed?.cc ?? '',
    bcc: seed?.bcc ?? '',
    replyTo: seed?.replyTo ?? '',
    subject: seed?.subject ?? '',
    ...body,
    showCcBcc:
      seed?.showCcBcc ??
      hasExtraComposeHeaders({
        cc: seed?.cc ?? '',
        bcc: seed?.bcc ?? '',
      }),
    inReplyTo: seed?.inReplyTo ?? '',
    references: seed?.references ?? '',
    draftMessageId: seed?.draftMessageId ?? newDraftMessageId(),
    signature: tracking,
    quote: quote ? { html: quote.html, text: quote.text } : undefined,
    encrypt: seed?.encrypt,
    sign: seed?.sign,
    sensitiveSource: seed?.sensitiveSource,
    decryptedInBody: seed?.decryptedInBody || undefined,
    encryptedDraft: seed?.encryptedDraft || undefined,
    encryptOffByUser: seed?.encryptOffByUser || undefined,
    quoteWithheld: seed?.quoteWithheld || undefined,
    sourceDraft: seed?.sourceDraft,
    attachments: seed?.attachments ?? [],
  }
  const id = `compose-${Date.now()}-${composeSeq++}`
  compose$.tabs.push({
    id,
    kind: 'compose',
    messageId: '',
    threadId: seed?.threadId ?? '',
    subject: seed?.title || draft.subject || 'New message',
    from: '',
    body: '',
    viewMode: 'plain',
    compose: draft,
  })
  compose$.activeTab.set(id)
  return id
}

// Escalate the active thread's quick reply into a full-window composer tab,
// seeded as a reply (recipients, "Re:" subject, threading headers) and carrying
// over whatever's been typed/attached. Clears the quick reply on success.
// No-op when there's no active conversation. `replyAll` seeds the wider
// recipient list; the quick reply box has no recipient fields to show it in, so
// reply-all always lands here rather than in the box.
/** Whether a reader tab with its message is in front, for the reply shortcut,
 * which answers it even with no conversation selected behind it. */
export function readerTabInFront(): boolean {
  const tab = compose$.tabs.peek().find((item) => item.id === compose$.activeTab.peek())
  return tab?.kind === 'reader' && !!tab.message
}

/** The reply shortcut. A reader tab in front answers the message it shows;
 * otherwise the conversation's reply goes into the quick-reply box or, where
 * the conversation-footer setting leaves none, the full editor. */
export function replyInConversation() {
  const tab = compose$.tabs.peek().find((item) => item.id === compose$.activeTab.peek())
  if (tab?.kind === 'reader') {
    // As the tab's own Reply button: not for a feed's article.
    if (tab.message && !isRssMessage(tab.message)) replyToMessage(tab.message)
    return
  }
  // A compose tab in front: the conversation behind it is not the one on screen.
  if (tab?.kind === 'compose') return
  if (quickReplyBoxShown()) return focusQuickReply()
  // The message the action bar's Reply takes, so never a draft; and, as there
  // is no bar under a feed, no reply to one.
  const target = actionBarTarget()
  if (target && !isRssMessage(target)) replyToMessage(target)
}

function isRssMessage(message: Message): boolean {
  const account = accounts$.peek().find((acc) => acc.id === message.account_id)
  return account?.provider === 'rss' || account?.auth_type === 'rss'
}

export function openReplyInFullEditor(options?: { replyAll?: boolean }) {
  const loading = waitForQuickReplyRead()
  if (loading) {
    const scope = quickReplyScope
    void loading.then(() => {
      if (scope === quickReplyScope) openReplyInFullEditor(options)
    })
    return
  }
  const t = getActiveThread()
  if (!t) return
  const target = pickReplyTarget(t)
  const subject = replySubject(t.subject, target)
  const accounts = accounts$.get()
  // Everyone the quick reply was already going to, when it was.
  const { to, cc } = options?.replyAll ? buildReplyRecipients(target, true) : quickReplyAddressing(target)
  const { in_reply_to, references } = buildReplyThreading(target)
  const replyAcc = accounts.find((acc) => acc.id === t.account_id)
  // Hand off any draft already saved for this quick reply so the full editor
  // continues editing the same server-side draft instead of creating a
  // duplicate one — or the id a failed save may have left a copy under, which
  // the editor's next save then overwrites instead of stranding.
  const existingDraftId =
    (compose$.quickReplyDraftSaved.peek() ? compose$.quickReplyDraftId.peek() : unconfirmedQuickReplyDraftId()) ||
    undefined
  // Nothing to hand over while the first autosave is still allocating its id, so
  // the tab starts a draft of its own — mark the copy that save lands as
  // abandoned rather than leaving it in Drafts with nothing pointing at it.
  if (!existingDraftId && quickReplyDraftSaveInFlight) {
    quickReplyAbandonedSaves.set(t.thread_id, nextQuickReplyOwnershipTick())
  }
  cancelQuickReplyDraftSave()
  // The box holds only the user's text. With the signature on, the full
  // composer inserts and tracks its own copy (the account is the same, so this
  // is the identical text, now swappable if the draft changes identity); with
  // it off, the composer adds none.
  const carried = compose$.composer.peek()
  // A bar holding a saved quoted reply already carries its quote.
  const quote = replyQuote(target)
  openComposeTab({
    accountId: t.account_id || undefined,
    fromEmail: resolveQuickReplyFrom(target, replyAcc),
    to,
    cc,
    showCcBcc: !!cc.trim(),
    subject,
    text: carried,
    // A box holding a signature it can't switch hands it over as found, for
    // the full composer to look at the same way.
    ...(compose$.quickReplySignatureUnmanaged.peek()
      ? { noSignature: true }
      : { signatureOff: compose$.quickReplySignatureOff.peek() }),
    attachments: compose$.composerAttachments.get(),
    inReplyTo: in_reply_to,
    references,
    draftMessageId: existingDraftId,
    title: subject,
    // The quick reply's encryption choice carries over, and so does the lock
    // of a draft that was saved encrypted. Switched off there on purpose, it
    // stays off: `false`, not unset (which the core would read as "answers
    // encrypted mail, so encrypt"), and the toggle doesn't turn it back on.
    encrypt: quickReplyEncryptOff(target) ? false : quickReplyEncrypts() || undefined,
    encryptOffByUser: quickReplyOffByUser(target) || undefined,
    sign: quickReplySigns(),
    sensitiveSource: target.crypto?.encrypted ? 'reply' : undefined,
    encryptedDraft: quickReplyHoldsEncryptedDraft(t.thread_id) || undefined,
    quoteWithheld: replyQuoteWithheld(target),
    threadId: t.thread_id,
    quote: quote && !bodyHasReplyQuote(carried, quote) ? quote : undefined,
  })
  clearQuickReplyDraftOwnership()
  compose$.quickReplyFrom.set('')
  compose$.quickReplyAdded.set([])
  // The thread stays open behind the new tab, so the box the user comes back to
  // is a fresh quick reply.
  startFreshQuickReply()
}

export function openMailtoCompose(raw: string) {
  const draft = parseMailto(raw)
  if (!draft) return
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast(t('compose.toast.addMailAccountBeforeComposing'))
    return
  }
  openComposeTab({
    to: draft.to,
    cc: draft.cc,
    bcc: draft.bcc,
    subject: draft.subject,
    text: draft.body,
    showCcBcc: !!draft.cc || !!draft.bcc,
    title: draft.subject || 'New message',
  })
}

// Open a message as a brand-new editable draft ("Edit as New Message", à la
// Apple Mail / Thunderbird). Works on any message regardless of folder — it's a
// duplicate-into-compose, not a reply: the subject/recipients/body/attachments
// are copied and the user edits from there. Deliberately carries NO In-Reply-To/
// References, so the copy starts a fresh conversation instead of threading into
// the original.
//
// Body: the original HTML is carried into the rich editor when present (else the
// plaintext body). Attachments live in the media cache as files keyed by
// `/media/<key>`; we read their bytes back as base64 composer attachments.
// Inline images (referenced as `/media/<key>` inside the carried HTML) are NOT
// re-attached here — the composer's send path re-inlines them from those refs,
// so adding them again would duplicate. Only genuine, non-inline attachments are
// pulled in as file chips.
export async function editAsNewMessage(message: Message) {
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast(t('compose.toast.addMailAccountBeforeComposing'))
    return
  }
  const rich = !!message.body_html
  const html = message.body_html ?? ''
  const acc = accounts$.get().find((a) => a.id === message.account_id)
  const id = openComposeTab({
    accountId: message.account_id || undefined,
    fromEmail: acc ? detectSenderFrom(message, acc) : '',
    to: message.to ?? '',
    cc: message.cc ?? '',
    showCcBcc: !!message.cc?.trim(),
    subject: message.subject ?? '',
    rich,
    html: rich ? html : '',
    text: rich ? '' : (message.body ?? ''),
    title: message.subject || 'New message',
    noSignature: true,
    // A copy of encrypted mail carries its decrypted text: encrypted only.
    encrypt: !!message.crypto?.encrypted || undefined,
    decryptedInBody: !!message.crypto?.encrypted || undefined,
  })
  if (!id) return

  await fetchComposeTabAttachments(id, message.attachments ?? [], rich ? html : '')
}

// Open a clean compose draft for forwarding a message. Recipients and threading
// headers are intentionally blank; the original content is quoted in the body
// and non-inline attachments are copied back into the composer.
/** Reply-all to one message, rather than to the conversation's reply target:
 * the message menu acts on the message under the cursor. Opens a composer tab
 * — the recipients are the point of the action, and only the full editor shows
 * them. The quick reply's own draft is left alone; this is a separate reply. */
export function replyAllToMessage(message: Message) {
  replyToMessage(message, true)
}

/** Reply (or reply-all) to one message in a compose tab, rather than to the
 *  conversation's reply target in the quick reply bar. */
export function replyToMessage(message: Message, replyAll = false) {
  const accounts = accounts$.get()
  if (accounts.filter(isSendableAccount).length === 0) {
    showToast(t('compose.toast.addMailAccountBeforeComposing'))
    return
  }
  const { to, cc } = buildReplyRecipients(message, replyAll)
  const { in_reply_to, references } = buildReplyThreading(message)
  const acc = accounts.find((a) => a.id === message.account_id)
  const subject = replySubject(message.subject, message)
  openComposeTab({
    threadId: message.thread_id,
    accountId: message.account_id || undefined,
    fromEmail: acc ? detectAliasFrom(message, acc) : '',
    to,
    cc,
    showCcBcc: !!cc.trim(),
    subject,
    title: subject,
    inReplyTo: in_reply_to,
    references,
    quote: replyQuote(message),
    // An encrypted conversation stays encrypted (the user can turn it off,
    // which drops the quote).
    encrypt: !!message.crypto?.encrypted || undefined,
    sensitiveSource: message.crypto?.encrypted ? 'reply' : undefined,
    quoteWithheld: replyQuoteWithheld(message),
  })
}

export async function forwardMessage(message: Message) {
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast(t('compose.toast.addMailAccountBeforeComposing'))
    return
  }
  const rich = !!message.body_html
  const html = rich ? forwardedHtmlBody(message) : ''
  const id = openComposeTab({
    accountId: message.account_id || undefined,
    to: '',
    cc: '',
    bcc: '',
    showCcBcc: false,
    subject: forwardedSubject(message.subject),
    rich,
    html,
    text: rich ? '' : forwardedPlainBody(message),
    title: forwardedSubject(message.subject),
    signaturePlacement: 'aboveQuote',
    // A forward of encrypted mail carries its decrypted text in the body:
    // it can only go out encrypted.
    encrypt: !!message.crypto?.encrypted || undefined,
    sensitiveSource: message.crypto?.encrypted ? 'forward' : undefined,
    decryptedInBody: !!message.crypto?.encrypted || undefined,
  })
  if (!id) return

  await fetchComposeTabAttachments(id, message.attachments ?? [], html)
}

export async function readComposerAttachments(
  attachments: Attachment[],
  inlineHtml: string,
): Promise<ComposerAttachment[]> {
  const toFetch = attachments.filter((a) => a.key && !inlineHtml.includes(`/media/${a.key}`))
  if (toFetch.length === 0) return []

  const fetched = await Promise.all(
    toFetch.map(async (a) => {
      try {
        const res = await invoke<{ data: string; mime: string; size: number }>('mail.readAttachment', { key: a.key })
        if (!res?.data) return null
        const att: ComposerAttachment = {
          id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
          filename: a.filename,
          mime: a.mime || res.mime || 'application/octet-stream',
          size: a.size || res.size,
          data: res.data,
        }
        return att
      } catch {
        return null
      }
    }),
  )
  return fetched.filter((a): a is ComposerAttachment => a !== null)
}
