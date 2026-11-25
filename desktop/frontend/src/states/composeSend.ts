import { threadWindowSession } from '../lib/windowSession'
import { t } from '../lib/i18n'
import type { Attachment, ComposerAttachment, Message } from '../types'
import { invoke } from '../lib/bridge'
import { ui$ } from './ui'
import { accounts$ } from './accounts'
import { getActiveThread } from './mail'
import { mail$ } from './mailState'
import { LOCAL_SEND_PREFIX } from './pendingSends'
import { ownSentCrypto } from '../lib/crypto'
import { htmlToText, resolveInlineCids } from '../lib/html'
import { undeliveredOf } from './undelivered'
import { thread$ } from './thread'
import { allocateMessageIdentity } from './composeState'

// Send a composed message via the same mail.send path used by replies. When
// `rich`, the HTML is sent with a derived plaintext fallback. Throws on failure
// so the caller can surface the error inline. Returns the addresses it didn't
// reach (see `undeliveredOf`).
export async function sendComposed(args: {
  accountId: string
  from?: string
  to: string
  cc?: string
  bcc?: string
  replyTo?: string
  subject: string
  rich: boolean
  content: string // HTML when rich, plaintext otherwise
  inReplyTo?: string
  references?: string
  attachments: ComposerAttachment[]
  /** End-to-end encrypt (PGP/MIME); the core refuses rather than downgrade. */
  encrypt?: boolean
  /** The body holds decrypted text (an unfolded quote): always encrypted. */
  decryptedInBody?: boolean
  /** The composer's sign switch (unencrypted mail only). Unset: the account's
   * default, decided by the core. */
  sign?: boolean
}) {
  const html = args.rich ? args.content : ''
  const body = args.rich ? htmlToText(args.content) : args.content
  const encrypt = args.decryptedInBody ? true : args.encrypt
  const result = await invoke('mail.send', {
    ...(args.sign === undefined || encrypt ? {} : { sign: args.sign }),
    encrypt: !!encrypt,
    // Plain only when encryption was switched off (`false`), never because the
    // flag is missing: the core encrypts a reply to encrypted mail that says
    // neither. A body holding decrypted text is never sent plain.
    plain: encrypt === false,
    account_id: args.accountId,
    from: args.from ?? '',
    to: args.to,
    cc: args.cc ?? '',
    bcc: args.bcc ?? '',
    reply_to: args.replyTo ?? '',
    subject: args.subject,
    body,
    html,
    in_reply_to: args.inReplyTo ?? '',
    references: args.references ?? '',
    attachments: args.attachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      data: a.data,
      inline_id: a.inlineId ?? '',
    })),
  })
  return undeliveredOf(result)
}

// Surface a just-sent compose-tab message in the open conversation so the user
// sees their reply immediately, without waiting for the next IMAP sync to pull
// it back from the Sent folder. No-op unless the message belongs to the
// currently open thread. The optimistic bubble (id prefixed with
// LOCAL_SEND_PREFIX) is replaced by the real DB row on the next thread reload.
export function appendSentMessage(args: {
  threadId: string
  accountId: string
  from: string
  to: string
  cc?: string
  bcc?: string
  subject: string
  rich: boolean
  content: string // HTML when rich, plaintext otherwise
  references?: string
  attachments: ComposerAttachment[]
  /** Sent encrypted: the bubble shows the lock its Sent copy will have. */
  encrypt?: boolean
}) {
  if (!args.threadId || ui$.selectedThread.get() !== args.threadId) return
  const activeT = getActiveThread()
  if (!activeT || activeT.thread_id !== args.threadId) return

  const account = accounts$.get().find((acc) => acc.id === args.accountId)
  // The content still carries `cid:` refs (what actually went out on the wire);
  // resolve them against the attachment bytes so this local copy renders.
  const html = args.rich ? resolveInlineCids(args.content, args.attachments) : ''
  const body = args.rich ? htmlToText(args.content) : args.content
  const sent: Message = {
    id: `${LOCAL_SEND_PREFIX}${Date.now()}`,
    account_id: args.accountId,
    folder_id: activeT.folder_id,
    thread_id: args.threadId,
    from_name: 'You',
    from_addr: args.from || account?.email || '',
    to: args.to,
    cc: args.cc ?? '',
    bcc: args.bcc ?? '',
    references: args.references ?? '',
    subject: args.subject,
    preview: body || (args.attachments.length > 0 ? `[Attachment: ${args.attachments[0].filename}]` : ''),
    body,
    body_html: html || undefined,
    date: Math.floor(Date.now() / 1000),
    unread: false,
    starred: false,
    has_attachments: args.attachments.length > 0,
    send_status: 'sent',
    crypto: args.encrypt ? ownSentCrypto() : undefined,
    attachments: args.attachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      size: a.size,
      key: null,
      url: a.mime.startsWith('image/') || a.mime.startsWith('video/') ? `data:${a.mime};base64,${a.data}` : null,
    })),
  }
  mail$.messages.push(sent)
  thread$.pendingOwnSendId.set(sent.id)
}

// Save a composed draft message to the drafts folder on the server.
export async function saveComposedDraft(args: {
  accountId: string
  from?: string
  to: string
  cc?: string
  bcc?: string
  replyTo?: string
  subject: string
  rich: boolean
  content: string // HTML when rich, plaintext otherwise
  inReplyTo?: string
  references?: string
  draftMessageId: string
  attachments: ComposerAttachment[]
  /** Encrypt the saved draft to the user's own key (an encrypted message's
   * draft must not sit on the server in the clear). */
  encrypt?: boolean
  /** The body holds decrypted text (an unfolded quote): always encrypted. */
  decryptedInBody?: boolean
  /** The composer's sign switch, recorded with the draft (the draft itself
   * isn't signed) so reopening it keeps the choice. */
  sign?: boolean
}): Promise<string> {
  const html = args.rich ? args.content : ''
  const body = args.rich ? htmlToText(args.content) : args.content
  const draftMessageId =
    !args.draftMessageId || args.draftMessageId.startsWith('local-draft-')
      ? await allocateMessageIdentity(args.accountId, true)
      : args.draftMessageId
  if (threadWindowSession)
    await invoke('threadWindow.reserveDraft', {
      session: threadWindowSession,
      previous_id: args.draftMessageId,
      draft_id: draftMessageId,
    })
  const encrypt = args.decryptedInBody ? true : args.encrypt
  const request = invoke('mail.saveDraft', {
    ...(args.sign === undefined ? {} : { sign: args.sign }),
    encrypt: !!encrypt,
    // Plain only when encryption was switched off (`false`), never because the
    // flag is missing: the core encrypts a reply to encrypted mail that says
    // neither. A body holding decrypted text is never saved plain.
    plain: encrypt === false,
    account_id: args.accountId,
    from: args.from ?? '',
    to: args.to,
    cc: args.cc ?? '',
    bcc: args.bcc ?? '',
    reply_to: args.replyTo ?? '',
    subject: args.subject,
    body,
    html,
    in_reply_to: args.inReplyTo ?? '',
    references: args.references ?? '',
    draft_id: draftMessageId,
    attachments: args.attachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      data: a.data,
      inline_id: a.inlineId ?? '',
    })),
  })
  try {
    await request
  } catch (error) {
    // A failed save may still have written the draft (the host can give up
    // waiting while the core carries on), so the id it was written under has to
    // survive the failure: the next save then replaces that copy, and a send
    // discards it, instead of it sitting in Drafts with nothing pointing at it.
    throw Object.assign(error instanceof Error ? error : new Error(String(error)), {
      draftMessageId,
    })
  }
  return draftMessageId
}

/** The server draft id a failed [saveComposedDraft] was writing, if it had one. */
export function draftIdOfFailedSave(error: unknown): string | undefined {
  const id = (error as { draftMessageId?: unknown } | null)?.draftMessageId
  return typeof id === 'string' && id ? id : undefined
}
