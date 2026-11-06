import type { Account, Alias, Message } from '../types'
import { ui$ } from './ui'
import { accounts$, isSendableAccount, accountIdentities } from './accounts'
import { mail$, getActiveThread } from './mail'
import { isDraftFolder, isInboxFolder } from './mailFolders'
import { bareAddr, splitAddressList } from '../lib/address'
import { compose$ } from './composeState'

// Reply addressing: which message to answer, from which identity, and to whom.

/** Pick the source message to reply to: the most recent loaded message in the
 * active thread that wasn't sent by us — its Reply-To/Cc are the headers we
 * should honor — preferring one that accepts replies, so a no-reply
 * notification or a bounce doesn't take the reply (the core decides which, see
 * `accepts_replies`). Falls back to the thread header when no loaded message
 * matches. */
// An encrypted message's subject until it is decrypted (RFC 9788 protected
// headers: `[...]`, Thunderbird `...`), reply prefixes aside.
function isEncryptedPlaceholder(subject: string): boolean {
  const core = subject.replace(/^\s*((re|aw|sv|wg|antw|fwd?)\s*:\s*)+/i, '').trim()
  return core === '[...]' || core === '...'
}

/**
 * A reply's subject: `Re: ` before `subject` unless it has one. When `subject`
 * is still an encrypted message's placeholder, the decrypted subject of the
 * message being answered (`target`, from `pickReplyTarget`) stands in, so an
 * encrypted reply never carries `Re: [...]` inside its encryption: that subject
 * groups as no subject at all and splits the thread for whoever reads it. Only
 * that message: scanning loaded messages once took another thread's subject.
 * With it undecrypted too, the reply has no subject rather than one made from
 * the placeholder.
 */
export function replySubject(subject: string, target?: Message): string {
  let base = subject
  if (isEncryptedPlaceholder(subject)) {
    if (!target || isEncryptedPlaceholder(target.subject)) return ''
    base = target.subject
  }
  return base.startsWith('Re:') ? base : `Re: ${base}`
}

export function pickReplyTarget(activeT: Message, excludedId = ''): Message {
  const messages = mail$.messages.get()
  const ownAddrs = ownAddressSet(accounts$.get())
  const inThread = messages.filter((m) => m.thread_id === activeT.thread_id && m.id !== excludedId)
  const received = inThread.filter((m) => !sentByUs(m, ownAddrs))
  const replyable = received.filter((m) => m.reply?.accepts_replies !== false)
  return replyable[replyable.length - 1] ?? received[received.length - 1] ?? inThread[inThread.length - 1] ?? activeT
}

/** Whether a loaded message is one we sent, as opposed to one we received.
 * The core settles this from the message's own delivery headers whenever it has
 * the body cached. Until then `outgoing` — like the address fallback kept here
 * for rows shaped before that flag existed — falls back to matching From against
 * our identities, which also fires for a colleague's mail from a shared alias;
 * sitting in the inbox vetoes that match. An optimistic send (`send_status`) is
 * ours whatever folder it claims. */
function sentByUs(m: Message, ownAddrs: Set<string>): boolean {
  if (m.send_status) return true
  if (isInboxFolder(m.folder_id, m.account_id)) return false
  return m.outgoing === true || ownAddrs.has((m.from_addr || '').trim().toLowerCase())
}

/** Every address the user owns across all accounts (primary + aliases),
 * lowercased — used to keep our own addresses out of reply recipients. */
export function ownAddressSet(accounts: Account[]): Set<string> {
  const out = new Set<string>()
  for (const acc of accounts) {
    for (const id of accountIdentities(acc)) {
      const addr = id.email.trim().toLowerCase()
      if (addr) out.add(addr)
    }
  }
  return out
}

/** Pick the send-as address for a reply: if the original was delivered to one of
 * the account's identities (primary or an alias) via To/Cc, reply from that
 * address; otherwise fall back to the account's primary. Returns "" when the
 * primary should be used (the draft treats "" as the primary). */
export function detectAliasFrom(target: Message, acc: Account): string {
  const recipients = new Set(
    [...splitAddressList(target.to), ...splitAddressList(target.cc)].map((e) => bareAddr(e).toLowerCase()),
  )
  const match = accountIdentities(acc).find((id) => recipients.has(id.email.trim().toLowerCase()))
  // Use the matched address, but leave "" when it's just the primary so the
  // draft's default (primary) handling stays in effect.
  return match && match.email.toLowerCase() !== acc.email.toLowerCase() ? match.email : ''
}

/** The send-as address for a copy of a message ("Edit as New"): the identity
 * the original was sent from, when it is one of this account's; otherwise the
 * primary. Like {@link detectAliasFrom}, "" stands for the primary. */
export function detectSenderFrom(message: Message, acc: Account): string {
  const from = (message.from_addr || '').trim().toLowerCase()
  const match = accountIdentities(acc).find((id) => id.email.trim().toLowerCase() === from)
  return match && match.email.toLowerCase() !== acc.email.toLowerCase() ? match.email : ''
}

/** Continue a thread with the identity used by its most recent outgoing
 * message. Returns null when the loaded thread has no message from one of this
 * account's configured identities; an empty string means the primary identity. */
function detectRecentThreadFrom(target: Message, acc: Account): string | null {
  const identities = accountIdentities(acc)
  const byEmail = new Map(identities.map((identity) => [identity.email.trim().toLowerCase(), identity.email]))
  const ownAddrs = ownAddressSet(accounts$.get())
  const messages = mail$.messages.get()
  for (let i = messages.length - 1; i >= 0; i--) {
    const message = messages[i]
    if (
      message.thread_id !== target.thread_id ||
      message.account_id !== acc.id ||
      isDraftFolder(message.folder_id, message.account_id)
    )
      continue
    if (!sentByUs(message, ownAddrs)) continue
    const email = byEmail.get((message.from_addr || '').trim().toLowerCase())
    if (!email) continue
    return email.toLowerCase() === acc.email.toLowerCase() ? '' : email
  }
  return null
}

/** The address the active thread's quick reply sends from: the identity the user
 * picked in the From indicator, the identity used by the newest outgoing
 * message, or the alias detected from the inbound reply target. Like
 * {@link detectAliasFrom} this returns "" for the account primary, which every
 * send/draft path reads as "use the default". Peeks rather than gets: the send
 * and autosave paths are not reactive. */
export function resolveQuickReplyFrom(target: Message, acc: Account | null | undefined): string {
  const override = compose$.quickReplyFrom.peek()
  if (!acc) return ''
  if (override) return override.toLowerCase() === acc.email.toLowerCase() ? '' : override
  const recent = detectRecentThreadFrom(target, acc)
  if (recent !== null) return recent
  return detectAliasFrom(target, acc)
}

/** Backs the quick reply's From indicator: the identities the active thread's
 * account can send as, plus the one currently resolved. `identities` is empty
 * when there is nothing to choose between (no thread, an unsendable account, or
 * a single identity) — the indicator hides itself in that case rather than
 * stating the obvious. Reactive: safe to read from a component. */
export function quickReplyFromState(): { identities: Alias[]; selected: Alias | null } {
  const none = { identities: [] as Alias[], selected: null }
  const thread = getActiveThread()
  if (!thread) return none
  const accounts = accounts$.get()
  const accountId = thread.account_id || ui$.selectedAccount.get()
  const acc = accounts.find((a) => a.id === accountId) ?? accounts[0] ?? null
  if (!isSendableAccount(acc) || !acc) return none
  const identities = accountIdentities(acc)
  if (identities.length < 2) return none
  const override = compose$.quickReplyFrom.get()
  const target = pickReplyTarget(thread)
  const recent = detectRecentThreadFrom(target, acc)
  const email = override || (recent === null ? detectAliasFrom(target, acc) : recent) || acc.email
  const selected = identities.find((id) => id.email.toLowerCase() === email.toLowerCase()) ?? identities[0]
  return { identities, selected }
}

/** Backs the quick reply's recipient line: the To/Cc the box would send to as
 * it stands, as raw address-list strings. Empty for a thread that cannot be
 * replied to, which hides the line. Reactive: safe to read from a component. */
export function quickReplyRecipients(): { to: string; cc: string } {
  const none = { to: '', cc: '' }
  const thread = getActiveThread()
  if (!thread) return none
  const accounts = accounts$.get()
  const acc = accounts.find((a) => a.id === (thread.account_id || ui$.selectedAccount.get())) ?? accounts[0] ?? null
  if (!isSendableAccount(acc)) return none
  return addressQuickReply(pickReplyTarget(thread), compose$.quickReplyAdded.get())
}

/** The people Reply all would add to a reply to `target`, as address-list
 * entries with their display names: everyone in its reply-all form whom the
 * plain reply doesn't reach. */
export function replyAllExtras(target: Message): string[] {
  if (!replyAllAddsRecipients(target)) return []
  const plain = buildReplyRecipients(target)
  const reached = new Set(splitAddressList(`${plain.to}, ${plain.cc}`).map(bareAddr))
  const all = buildReplyRecipients(target, true)
  return splitAddressList(`${all.to}, ${all.cc}`).filter((entry) => !reached.has(bareAddr(entry)))
}

/** The plain reply to `target`, with those of Reply all's extra people whose
 * (bare, lowercased) addresses are in `added` copied as well. */
export function addressQuickReply(target: Message, added: string[]): { to: string; cc: string } {
  const plain = buildReplyRecipients(target)
  if (added.length === 0) return plain
  const extra = replyAllExtras(target).filter((entry) => added.includes(bareAddr(entry)))
  return { to: plain.to, cc: [plain.cc, ...extra].filter((part) => part.trim()).join(', ') }
}

/** The To/Cc the quick reply sends to and saves under: the plain reply plus
 * whoever was added from the recipient line. Not reactive. */
export function quickReplyAddressing(target: Message): { to: string; cc: string } {
  return addressQuickReply(target, compose$.quickReplyAdded.peek())
}

/** The people the recipient line offers to add to the quick reply: Reply all's
 * extras not added yet. Reactive. */
export function quickReplyAddable(): string[] {
  const thread = getActiveThread()
  if (!thread) return []
  const accounts = accounts$.get()
  const acc = accounts.find((a) => a.id === (thread.account_id || ui$.selectedAccount.get())) ?? accounts[0] ?? null
  if (!isSendableAccount(acc)) return []
  const added = compose$.quickReplyAdded.get()
  return replyAllExtras(pickReplyTarget(thread)).filter((entry) => !added.includes(bareAddr(entry)))
}

/** The To/Cc a reply to this message gets, as the core decided them (see
 * meron-core/src/reply.rs): the sender is addressed, the original Cc is copied,
 * and our own addresses stay out of both. `replyAll` picks the form that also
 * copies the other original recipients.
 *
 * A target with no `reply` field carries no recipient headers to decide from — a
 * thread card standing in for a thread whose messages have not loaded, or an RSS
 * item. Addressing its sender is all that can be said. */
export function buildReplyRecipients(target: Message, replyAll = false): { to: string; cc: string } {
  const reply = target.reply
  if (!reply) {
    const from = target.from_name ? `${target.from_name} <${target.from_addr}>` : target.from_addr || ''
    return { to: from, cc: '' }
  }
  return replyAll ? { to: reply.all_to, cc: reply.all_cc } : { to: reply.to, cc: reply.cc }
}

/** Whether replying to all would reach anyone a plain reply does not — the
 * message carries other recipients besides us and the sender. False makes the
 * two actions identical, and the menus hide the reply-all item rather than
 * offering a second way to do the same thing. */
export function replyAllAddsRecipients(target: Message): boolean {
  return target.reply?.all_adds_recipients === true
}

/** The active conversation's reply target has other recipients to reply to.
 * Reactive: safe to read from a component. */
export function canReplyAllToThread(): boolean {
  const thread = getActiveThread()
  if (!thread) return false
  return replyAllAddsRecipients(pickReplyTarget(thread))
}

/** One message has other recipients to reply to. Not reactive; the message
 * menus read it once, for the message they were opened on. */
export function messageCanReplyAll(message: Message): boolean {
  return replyAllAddsRecipients(message)
}

/** Build the `In-Reply-To` (parent Message-ID) and `References` chain (parent's
 * References + parent's Message-ID) for a reply. Both are bare ids — the
 * backend wraps them in angle brackets when emitting headers. */
export function buildReplyThreading(target: Message): {
  in_reply_to: string
  references: string
} {
  const parentId = (target.message_id || '').trim()
  if (!parentId) return { in_reply_to: '', references: '' }
  const parentRefs = (target.references || '')
    .split(/\s+/)
    .map((s) => s.trim())
    .filter(Boolean)
  // Append the parent's own Message-ID to its References chain so the reply
  // links to the entire ancestry, not just the immediate parent.
  const refs = parentRefs.includes(parentId) ? parentRefs : [...parentRefs, parentId]
  return { in_reply_to: parentId, references: refs.join(' ') }
}

/** The message the quick reply answers, or null (no thread, or an account
 * that can't send). Reactive. */
export function quickReplyTarget(): Message | null {
  const thread = getActiveThread()
  if (!thread) return null
  const accounts = accounts$.get()
  const acc = accounts.find((a) => a.id === (thread.account_id || ui$.selectedAccount.get())) ?? accounts[0] ?? null
  if (!isSendableAccount(acc)) return null
  return pickReplyTarget(thread)
}

/** Whether the quick reply is encrypted: the user's choice for this thread,
 * or by default on when the message answered was itself encrypted (sticky:
 * an encrypted conversation stays encrypted unless the user says otherwise). */
export function quickReplyEncrypts(): boolean {
  const target = quickReplyTarget()
  if (!target) return false
  const choice = compose$.quickReplyEncrypt.get()
  if (choice && choice.threadId === target.thread_id) return choice.on
  return !!target.crypto?.encrypted
}
