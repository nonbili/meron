import { observable } from '@legendapp/state'
import { t } from '../lib/i18n'
import type { Account, Alias, Attachment, ComposeDraft, ComposerAttachment, Message, MessageTab } from '../types'
import { invoke } from '../lib/bridge'
import { CONVERSATION_PAGE_SIZE } from '../lib/pagination'
import { ui$, showToast } from './ui'
import { accounts$, isSendableAccount, accountIdentities } from './accounts'
import { mail$, getActiveThread, isDraftFolder, isInboxFolder, loadThread, discardSavedDraftCopy } from './mail'
import { LOCAL_SEND_PREFIX, type PendingSend, setPendingSend, getPendingSend, discardPendingSend } from './pendingSends'
import { htmlToText, resolveInlineCids } from '../lib/html'
import { parseMailto } from '../lib/mailto'
import { splitAddressList, bareAddr } from '../lib/address'
import {
  bodyWithSignature,
  bodyWithSwappedSignature,
  resolveSignature,
  signatureForms,
  type ComposeBody,
  type Signature,
  type SignatureMark,
  type SignaturePlacement,
  type SignatureTracking,
} from '../lib/signature'
import { settings$ } from './settings'
import { revealRemote, thread$ } from './thread'
import { formatFullTimestamp } from '../components/chat/messageHelpers'
import { closeComposeSession, forgetComposeSession, pruneComposeSessions } from './composeSessions'
import { offerCertificateTrust } from './certificateTrust'

// Compose/reader-tab + draft state. Reader tabs open using the account's
// conversation view preference; compose tabs hold a full-editor draft. The
// quick-reply composer (composer / composerAttachments) and per-thread quick
// drafts live here too. Persisted to localStorage (volatile editor buffers, not
// DB settings).

// Persisted full-editor drafts. Attachments are intentionally NOT persisted —
// their base64 payloads would quickly blow past localStorage's ~5MB budget.
// Only text fields survive restarts; the user reattaches files if needed.
const COMPOSE_TABS_KEY = 'meron-compose-tabs'

// Local placeholder used until the first save asks meron-core for the stable
// RFC Message-ID. It is never sent to IMAP/SMTP.
/**
 * A placeholder draft id, replaced by a server-allocated one on the first save.
 * Exported because a draft that moves to another account needs a new one: the
 * allocated id belongs to the account whose Drafts folder holds that copy.
 */
export const newDraftMessageId = () => `local-draft-${Date.now()}-${Math.random().toString(36).substring(2, 11)}`

async function allocateMessageIdentity(accountId: string, draft: boolean): Promise<string> {
  const result = await invoke<{ message_id: string }>('mail.allocateIdentity', { account_id: accountId, draft })
  if (!result.message_id) throw new Error('Core did not allocate a message identity')
  return result.message_id
}

const newInlineImageId = () => `meron-image-${Date.now()}-${Math.random().toString(36).substring(2, 9)}@meron`

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

function textToHtml(text: string): string {
  if (!text.trim()) return ''
  return text
    .split(/\n{2,}/)
    .map((para) => `<p>${para.split('\n').map(escapeHtml).join('<br>')}</p>`)
    .join('')
}

function headerLine(label: string, value?: string): string {
  const trimmed = value?.trim()
  return trimmed ? `${label}: ${trimmed}` : ''
}

function formatMessageFrom(message: Message): string {
  return message.from_name ? `${message.from_name} <${message.from_addr}>` : message.from_addr
}

function forwardedSubject(subject: string): string {
  const trimmed = subject.trim()
  if (!trimmed) return 'Fwd: (no subject)'
  return /^fwd?:/i.test(trimmed) ? trimmed : `Fwd: ${trimmed}`
}

function forwardedPlainBody(message: Message): string {
  const header = [
    '---------- Forwarded message ---------',
    headerLine('From', formatMessageFrom(message)),
    headerLine('Date', formatFullTimestamp(message.date)),
    headerLine('Subject', message.subject || '(no subject)'),
    headerLine('To', message.to),
    headerLine('Cc', message.cc),
  ].filter(Boolean)
  return `\n\n${header.join('\n')}\n\n${message.body ?? ''}`
}

function forwardedHtmlBody(message: Message): string {
  const rows: Array<[string, string | undefined]> = [
    ['From', formatMessageFrom(message)],
    ['Date', formatFullTimestamp(message.date)],
    ['Subject', message.subject || '(no subject)'],
    ['To', message.to],
    ['Cc', message.cc],
  ]
  const presentRows = rows.filter((row): row is [string, string] => !!row[1]?.trim())
  const header = presentRows
    .map(([label, value]) => `<div><strong>${escapeHtml(label)}:</strong> ${escapeHtml(value)}</div>`)
    .join('')
  const body = message.body_html || textToHtml(message.body ?? '')
  return `<p><br></p><div class="meron-forwarded-message"><p>---------- Forwarded message ---------</p>${header}<br>${body}</div>`
}

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

type PersistedComposeTab = {
  id: string
  subject: string
  compose: Omit<ComposeDraft, 'attachments'> & { attachments: [] }
}
function hasExtraComposeHeaders(compose: Pick<ComposeDraft, 'cc' | 'bcc'>): boolean {
  return !!(compose.cc?.trim() || compose.bcc?.trim())
}
function loadPersistedComposeTabs(): MessageTab[] {
  try {
    const raw = localStorage.getItem(COMPOSE_TABS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as PersistedComposeTab[]
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter((t) => t && t.compose && typeof t.compose.to === 'string')
      .map<MessageTab>((t) => ({
        id: t.id,
        kind: 'compose',
        messageId: '',
        threadId: '',
        subject: t.subject || t.compose.subject || 'New message',
        from: '',
        body: '',
        viewMode: 'plain',
        compose: {
          ...t.compose,
          fromEmail: t.compose.fromEmail ?? '',
          showCcBcc: t.compose.showCcBcc && hasExtraComposeHeaders(t.compose),
          // Backfill for tabs persisted before draftMessageId existed, so the
          // first autosave after restart still replaces rather than duplicates.
          draftMessageId: t.compose.draftMessageId || newDraftMessageId(),
          attachments: [],
        },
      }))
  } catch {
    return []
  }
}
const initialComposeTabs = loadPersistedComposeTabs()

export const compose$ = observable({
  // Reader tabs for messages opened in "HTML mode"; activeTab "" = conversation view.
  // Re-hydrate any compose tabs whose drafts were persisted from the previous run.
  tabs: initialComposeTabs as MessageTab[],
  activeTab: '',
  // The thread shown by the "Current" conversation tab. The message pane renders a
  // single selectedThread, so a thread tab has to retarget selectedThread to load
  // its own messages. We remember the Current tab's thread here so switching to a
  // thread tab and back restores the conversation it was showing instead of
  // adopting the thread tab's. Kept in sync below while the Current tab is active.
  conversationThread: '',
  composer: '',
  composerAttachments: [] as ComposerAttachment[],
  // Server-side draft backing the active thread's quick reply, shared with the
  // full composer's saveDraft/discardDraft mechanism (mail.saveDraft/
  // mail.discardDraft) rather than a separate persistence path. Reset on
  // thread switch; re-derived from the thread's tail message when it's a
  // saved draft (see hydrateQuickReplyFromTailDraft below).
  quickReplyDraftId: '',
  quickReplyDraftSaved: false,
  // Send-as address explicitly chosen for the active thread's quick reply, set
  // by the From indicator's picker. Empty means "auto" — fall back to the alias
  // the original was delivered to (detectAliasFrom). Reset on thread switch, so
  // an override never leaks into the next conversation.
  quickReplyFrom: '',
  // The signature this app seeded into the quick reply, or null when there is
  // none to account for — the account sends none, or the body was hydrated from
  // a saved draft that already carries its own.
  //
  // Unlike the full composer's tracking (see lib/signature's SignatureTracking)
  // this needs no third "inserted nothing, but managed" state: a quick reply
  // cannot change sending account, since its From picker only offers aliases of
  // the one account and those all share a signature. With nothing to swap, the
  // only question ever asked of this is which part of the box is the user's.
  quickReplySignature: null as SignatureMark | null,
})

// While the Current conversation tab is active (activeTab ""), mirror every
// selectedThread change into conversationThread so it always remembers the live
// navigation. Thread-tab activations set activeTab first, so this guard skips
// their selectedThread retarget and leaves the Current tab's thread intact.
ui$.selectedThread.onChange(({ value }) => {
  if (compose$.activeTab.peek() === '') compose$.conversationThread.set(value)
})

// Return to the Current conversation tab, restoring the thread it was showing
// before a thread tab took over selectedThread.
export function activateConversationTab() {
  ui$.selectedThread.set(compose$.conversationThread.peek())
  compose$.activeTab.set('')
}

// Activation trail of tab ids ("" = Current), oldest first. Closing the active
// tab walks this back to the tab you were on before opening it — e.g. opening a
// message in a new tab from a thread tab and closing it returns to that thread
// tab, not whatever happens to sit next in the strip.
const tabHistory: string[] = []
compose$.activeTab.onChange(({ value }) => {
  // Collapse immediate repeats so re-activating the current tab is a no-op.
  if (tabHistory[tabHistory.length - 1] === value) return
  tabHistory.push(value)
  if (tabHistory.length > 50) tabHistory.shift()
})

// Drop the closed tab from the trail, then return the most recent entry that's
// still a live tab (or "" for Current). Pops dead entries as it goes.
function popToPreviousTab(closedId: string, remaining: MessageTab[]): string {
  for (let i = tabHistory.length - 1; i >= 0; i--) {
    if (tabHistory[i] === closedId) tabHistory.splice(i, 1)
  }
  while (tabHistory.length > 0) {
    const candidate = tabHistory[tabHistory.length - 1]
    if (candidate === '' || remaining.some((tab) => tab.id === candidate)) return candidate
    tabHistory.pop()
  }
  return ''
}

// Delete orphaned inline-image files from earlier sessions. writeMediaFile
// writes one loose file per inline paste into the media root and nothing ever
// reclaims them, so a discarded or sent draft leaks its images. On boot we
// collect the `/media/<key>` keys still referenced by the rehydrated compose
// tabs and let the backend remove every other loose file. Fire-and-forget.
export function pruneComposerMedia() {
  const keys = new Set<string>()
  // Match a root-level media key (no slash) — exactly writeMediaFile's output.
  // Per-account refs look like /media/<account>/… and are skipped by the regex
  // (and ignored by the backend, which only deletes regular files at the root).
  const re = /\/media\/([^/"'\s)>\\]+)/g
  for (const tab of compose$.tabs.get()) {
    const html = tab.compose?.html
    if (!html) continue
    for (const m of html.matchAll(re)) keys.add(m[1])
  }
  invoke('composer.pruneMedia', { keys: [...keys] }).catch(() => {})
}

// Persist compose tabs (full editor) on every tab change. We only store the
// text portion of each compose draft — attachments are dropped because their
// base64 payloads can blow past localStorage's quota. On boot the tabs come
// back; the user reattaches files if needed.
compose$.tabs.onChange(({ value: tabs }) => {
  pruneComposeSessions(new Set(tabs.map((tab) => tab.id)))
  const persisted: PersistedComposeTab[] = tabs
    .filter((t) => t.kind === 'compose' && t.compose)
    .map((t) => ({
      id: t.id,
      subject: t.subject,
      compose: { ...(t.compose as ComposeDraft), attachments: [] },
    }))
  try {
    localStorage.setItem(COMPOSE_TABS_KEY, JSON.stringify(persisted))
  } catch {
    // localStorage quota exceeded — drop silently.
  }
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
function quickReplyWithoutSignature(): { text: string; found: boolean } {
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
function quickReplyOutgoingText(): string {
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
let quickReplyDraftSaveInFlight: Promise<void> | null = null

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
  const ownAddrs = ownAddressSet(accounts)
  const { to, cc } = buildReplyRecipients(target, ownAddrs)
  const { in_reply_to, references } = buildReplyThreading(target)
  const fromEmail = resolveQuickReplyFrom(target, activeAcc)
  const subject = activeT.subject.startsWith('Re:') ? activeT.subject : `Re: ${activeT.subject}`
  const draftId = compose$.quickReplyDraftId.peek() || newDraftMessageId()
  compose$.quickReplyDraftId.set(draftId)

  let savedDraftId = draftId
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
    if (savedDraftId !== draftId) compose$.quickReplyDraftId.set(savedDraftId)
  } catch (error) {
    console.error('Quick reply draft autosave failed:', error)
    return
  }

  // The composer may have gone blank while the RPC was in flight (cleared by
  // the user, a send, or escalation to the full editor) — any discard that ran
  // in that window saw quickReplyDraftSaved still false and bailed. Drop the
  // draft we just wrote instead of letting it resurrect on the next thread open.
  const sameThread = ui$.selectedThread.peek() === activeT.thread_id
  if (sameThread && isQuickReplyBlank()) {
    if (compose$.quickReplyDraftId.peek() === savedDraftId) {
      compose$.quickReplyDraftId.set('')
      compose$.quickReplyDraftSaved.set(false)
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
  if (sameThread && compose$.quickReplyDraftId.peek() === savedDraftId) {
    compose$.quickReplyDraftSaved.set(true)
  }
}

// Discard the quick reply's server-side draft once the user has cleared the
// text/attachments back to blank, mirroring the full composer's discard flow.
export async function discardQuickReplyDraftIfEmpty() {
  // Wait out any in-flight autosave before peeking the flags below; the save
  // itself re-checks on completion and self-discards if the composer is blank.
  while (quickReplyDraftSaveInFlight) await quickReplyDraftSaveInFlight
  if (!isQuickReplyBlank()) return
  if (!compose$.quickReplyDraftSaved.peek()) return
  const draftId = compose$.quickReplyDraftId.peek()
  if (!draftId) return
  const activeT = getActiveThread()

  compose$.quickReplyDraftId.set('')
  compose$.quickReplyDraftSaved.set(false)
  await discardSavedDraftCopy({
    threadId: activeT?.thread_id ?? '',
    messageId: '',
    folderId: '',
    accountId: activeT?.account_id,
    draftMessageId: draftId,
  })
}

// Hide the saved draft currently hydrated into the quick-reply editor. The
// optimistic sent bubble is appended after the draft, so matching only the
// conversation tail would briefly reveal the draft again while sending.
// Other drafts in the thread remain visible.
export function withoutHydratedQuickReplyDraft(
  messages: Message[],
  draftMessageId: string,
  draftSaved: boolean,
): Message[] {
  if (!draftSaved || !draftMessageId) return messages
  return messages.filter(
    (message) =>
      !((message.message_id || message.id) === draftMessageId && isDraftFolder(message.folder_id, message.account_id)),
  )
}

let quickReplyDraftSaveTimer: ReturnType<typeof setTimeout> | null = null
const QUICK_REPLY_DRAFT_SAVE_DELAY_MS = 1200
type QuickReplySendHydrationGuard = {
  threadId: string
  accountId: string
  draftId: string
  inFlight: boolean
  suppressDraft: boolean
}
const quickReplySendHydrationGuards = new Map<string, QuickReplySendHydrationGuard>()

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
  if (value) return
  cancelQuickReplyDraftSave()
  compose$.composer.set('')
  compose$.composerAttachments.set([])
  compose$.quickReplyDraftId.set('')
  compose$.quickReplyDraftSaved.set(false)
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
  const tailDraftId = tail.message_id || tail.id
  const normalizedTailDraftId = normalizeQuickReplyDraftId(tailDraftId)
  const guarded = [...quickReplySendHydrationGuards.values()].some(
    (guard) =>
      guard.threadId === activeThreadId &&
      (guard.inFlight ||
        (guard.suppressDraft &&
          !!guard.draftId &&
          normalizeQuickReplyDraftId(guard.draftId) === normalizedTailDraftId)),
  )
  if (guarded) return
  if (compose$.quickReplyDraftId.peek() === tailDraftId) return

  compose$.composer.set(tail.body ?? '')
  compose$.composerAttachments.set([])
  compose$.quickReplyDraftId.set(tailDraftId)
  compose$.quickReplyDraftSaved.set(true)
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

function normalizeQuickReplyDraftId(value: string): string {
  return value.trim().replace(/^<|>$/g, '').toLowerCase()
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
  hydrateQuickReplyFromTailDraft(value)
})

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

// Open a single message in its own reader tab. The HTML is already on the
// message (shipped with threadRead), so this is instant — no fetch. Re-opening
// an already-open message just re-activates its tab.
export function openMessageTab(message: Message) {
  const existing = compose$.tabs.get().find((tab) => tab.messageId === message.id)
  if (existing) {
    compose$.activeTab.set(existing.id)
    return
  }
  const account = accounts$.get().find((acc) => acc.id === message.account_id)
  const preferHtml = account?.conversation_html ?? true
  const tab: MessageTab = {
    id: message.id,
    kind: 'reader',
    messageId: message.id,
    threadId: message.thread_id,
    // Carried so the tab can resolve this message's remote-content policy.
    accountId: message.account_id,
    revealRemote: !!thread$.revealedRemote.peek()[message.id],
    // Snapshotted rather than re-derived: the tab keeps no Message, and the
    // From address alone misses an alias or a send still in flight (the same
    // rule useMessageView applies in the conversation).
    outgoing:
      !!message.send_status ||
      message.outgoing === true ||
      (!!account &&
        accountIdentities(account).some(
          (identity) => identity.email.trim().toLowerCase() === message.from_addr.trim().toLowerCase(),
        )),
    subject: message.subject || '(no subject)',
    from: message.from_name || message.from_addr,
    fromRaw: message.from_name ? `${message.from_name} <${message.from_addr}>` : message.from_addr,
    to: message.to,
    cc: message.cc,
    bcc: message.bcc,
    replyTo: message.reply_to,
    date: message.date,
    body: message.body,
    bodyHtml: message.body_html,
    attachments: message.attachments,
    viewMode: message.body_html && preferHtml ? 'html' : 'plain',
  }
  compose$.tabs.push(tab)
  compose$.activeTab.set(tab.id)
}

// Open a conversation itself as a tab. The tab stores enough thread metadata to
// render even when the current mailbox/rail selection does not contain it.
export function openThreadTab(thread: Message) {
  const id = `thread-${thread.thread_id}`
  const existing = compose$.tabs.get().find((tab) => tab.id === id)
  if (!existing) {
    const tab: MessageTab = {
      id,
      kind: 'thread',
      messageId: '',
      threadId: thread.thread_id,
      accountId: thread.account_id,
      folderId: thread.folder_id,
      subject: thread.subject || '(no subject)',
      from: thread.from_name || thread.from_addr,
      body: '',
      viewMode: 'plain',
    }
    compose$.tabs.push(tab)
  }
  // Activate the tab before retargeting selectedThread so the conversationThread
  // mirror skips this change and the Current tab keeps its own thread.
  compose$.activeTab.set(id)
  ui$.selectedThread.set(thread.thread_id)
  ui$.mobilePane.set('conversation')
}

function newestMessage(messages: Message[]): Message | null {
  return messages.reduce<Message | null>((newest, message) => {
    if (!newest) return message
    return message.date > newest.date ? message : newest
  }, null)
}

export async function openThreadTabById(threadId: string) {
  if (!threadId) return
  const id = `thread-${threadId}`
  const existing = compose$.tabs.get().find((tab) => tab.id === id)
  if (existing) {
    compose$.activeTab.set(id)
    ui$.selectedThread.set(threadId)
    ui$.mobilePane.set('conversation')
    return
  }

  try {
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: threadId,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const message = newestMessage(result.messages ?? [])
    if (!message) {
      showToast(t('compose.toast.couldNotOpenNotificationThread'), 'error')
      return
    }
    openThreadTab(message)
    // Seed the conversation with the messages we just fetched so the reply target
    // (and its Message-ID) is available immediately. Without this, a reply sent
    // before the selectedThread effect re-fetches falls back to the thread card,
    // which carries no Message-ID, and the reply lands unthreaded.
    if (ui$.selectedThread.get() === message.thread_id) {
      mail$.messages.set(result.messages ?? [])
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenNotificationThread'), 'error')
  }
}

/**
 * The signature an account sends, in both body forms. Both are resolved even for
 * a rich draft: the composer can be toggled to plaintext at any time, and a
 * half-tracked signature could then be neither found nor replaced.
 */
function resolveSignatureFor(account: Account | undefined): Signature {
  return signatureForms(account ? resolveSignature(account, settings$.signature.peek()) : '')
}

/**
 * Move a draft to another From account, swapping the signature it carries for
 * the new account's. Sending account B's mail under account A's signature is
 * worse than no signature at all, so this runs on every account change; an
 * edited signature is left alone (see `bodyWithSwappedSignature`).
 */
function withSignatureForAccount(draft: ComposeDraft, partial: Partial<ComposeDraft>): Partial<ComposeDraft> {
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
type ComposeSeed = Partial<ComposeDraft> & {
  title?: string
  threadId?: string
  /** Where the signature lands; 'aboveQuote' for a seeded quote (forwards). */
  signaturePlacement?: SignaturePlacement
  /**
   * Skip the signature. Set when the body is an existing message being re-opened
   * (a saved draft, "Edit as New Message") — it already carries whatever
   * signature it was written with, and a second copy is not wanted.
   */
  noSignature?: boolean
}

export function openComposeTab(seed?: ComposeSeed): string | undefined {
  const sendable = accounts$.get().filter(isSendableAccount)
  if (sendable.length === 0) return undefined
  const selected = ui$.selectedAccount.get()
  const accountId = seed?.accountId ?? sendable.find((acc) => acc.id === selected)?.id ?? sendable[0].id
  // Only a truly blank compose (no seeded body) should pick up the account's
  // HTML/plain preference; reply/forward/mailto paths that seed plain `text`
  // stay plain since the editor only ever hydrates from `html`.
  const hasSeededBody = !!(seed?.text || seed?.html)
  const account = sendable.find((acc) => acc.id === accountId)

  // The signature is inserted into the body up front so it is editable (and
  // visible) like the rest of the draft, rather than appearing at send time.
  const seeded = {
    rich: seed?.rich ?? (hasSeededBody ? false : (account?.conversation_html ?? true)),
    html: seed?.html ?? '',
    text: seed?.text ?? '',
  }
  // A body this app did not compose (a saved draft, "Edit as New Message") may
  // already carry a signature, so it stays unmanaged: `undefined`, not `null`.
  const placement = seed?.signaturePlacement ?? 'belowText'
  const signature = seed?.noSignature ? undefined : resolveSignatureFor(account)
  // The placement is recorded even when the account sends no signature, so a
  // forward that later moves to an account with one still puts it above the
  // quote rather than after it.
  const tracking: SignatureTracking = signature ? { ...signature, placement } : undefined
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
// No-op when there's no active conversation.
export function openReplyInFullEditor() {
  const t = getActiveThread()
  if (!t) return
  const subject = t.subject.startsWith('Re:') ? t.subject : `Re: ${t.subject}`
  const target = pickReplyTarget(t)
  const accounts = accounts$.get()
  const ownAddrs = ownAddressSet(accounts)
  const { to, cc } = buildReplyRecipients(target, ownAddrs)
  const { in_reply_to, references } = buildReplyThreading(target)
  const replyAcc = accounts.find((acc) => acc.id === t.account_id)
  // Hand off any draft already saved for this quick reply so the full editor
  // continues editing the same server-side draft instead of creating a
  // duplicate one.
  const existingDraftId = compose$.quickReplyDraftSaved.peek() ? compose$.quickReplyDraftId.peek() : undefined
  cancelQuickReplyDraftSave()
  // The seeded signature is handed over stripped, so the full composer inserts
  // and tracks its own copy — the account is the same, so this is the identical
  // text, now swappable if the draft later changes identity. When it can't be
  // found the user has written into it: it stays in the body as theirs, and the
  // composer is told not to add a second.
  const carried = quickReplyWithoutSignature()
  openComposeTab({
    accountId: t.account_id || undefined,
    fromEmail: resolveQuickReplyFrom(target, replyAcc),
    to,
    cc,
    showCcBcc: !!cc.trim(),
    subject,
    text: carried.text,
    noSignature: !carried.found && !!compose$.quickReplySignature.peek()?.text,
    attachments: compose$.composerAttachments.get(),
    inReplyTo: in_reply_to,
    references,
    draftMessageId: existingDraftId,
    title: subject,
    threadId: t.thread_id,
  })
  compose$.composerAttachments.set([])
  compose$.quickReplyDraftId.set('')
  compose$.quickReplyDraftSaved.set(false)
  compose$.quickReplyFrom.set('')
  // The thread stays open behind the new tab, so the box the user comes back to
  // is a fresh quick reply — signature and all.
  seedQuickReplySignature()
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
  const id = openComposeTab({
    accountId: message.account_id || undefined,
    to: message.to ?? '',
    cc: message.cc ?? '',
    showCcBcc: !!message.cc?.trim(),
    subject: message.subject ?? '',
    rich,
    html: rich ? html : '',
    text: rich ? '' : (message.body ?? ''),
    title: message.subject || 'New message',
    noSignature: true,
  })
  if (!id) return

  const valid = await readComposerAttachments(message.attachments ?? [], rich ? html : '')
  if (valid.length === 0) return

  // The tab may have been edited/closed while we fetched; bail if it's gone,
  // otherwise merge onto whatever attachments it now holds.
  const tab = compose$.tabs.get().find((t) => t.id === id)
  if (!tab?.compose) return
  updateComposeDraft(id, { attachments: [...tab.compose.attachments, ...valid] })
}

function composeFromDraftMessage(message: Message): ComposeSeed {
  const rich = !!message.body_html
  return {
    accountId: message.account_id || undefined,
    fromEmail: message.from_addr ?? '',
    to: message.to ?? '',
    cc: message.cc ?? '',
    bcc: message.bcc ?? '',
    replyTo: message.reply_to ?? '',
    showCcBcc: hasExtraComposeHeaders({ cc: message.cc ?? '', bcc: message.bcc ?? '' }),
    subject: message.subject ?? '',
    rich,
    html: rich ? (message.body_html ?? '') : '',
    text: rich ? '' : (message.body ?? ''),
    inReplyTo: '',
    references: message.references ?? '',
    draftMessageId: message.message_id || newDraftMessageId(),
    sourceDraft: {
      threadId: message.thread_id,
      messageId: message.id,
      folderId: message.folder_id,
    },
    title: message.subject || 'New message',
    noSignature: true,
  }
}

function activateOpenDraftCompose(draft: Message): boolean {
  const existing = compose$.tabs
    .get()
    .find(
      (tab) =>
        tab.kind === 'compose' &&
        tab.compose?.sourceDraft &&
        (tab.compose.sourceDraft.messageId === draft.id ||
          (!!draft.message_id && tab.compose.draftMessageId === draft.message_id) ||
          (tab.compose.sourceDraft.threadId === draft.thread_id &&
            tab.compose.sourceDraft.folderId === draft.folder_id)),
    )
  if (!existing) return false
  compose$.activeTab.set(existing.id)
  return true
}

export function draftShouldOpenConversation(messages: Message[], draft: Message): boolean {
  return (
    messages.some((message) => !isDraftFolder(message.folder_id, message.account_id)) ||
    !!draft.references?.trim() ||
    !!draft.original_thread_id?.trim()
  )
}

async function openDraftMessageInCompose(draft: Message) {
  if (activateOpenDraftCompose(draft)) return true
  const id = openComposeTab(composeFromDraftMessage(draft))
  if (!id) return true

  const valid = await readComposerAttachments(draft.attachments ?? [], draft.body_html ?? '')
  if (valid.length > 0) {
    const tab = compose$.tabs.get().find((t) => t.id === id)
    if (tab?.compose) updateComposeDraft(id, { attachments: [...tab.compose.attachments, ...valid] })
  }
  return true
}

// Restore a saved server-side Drafts row into the full composer. Drafts are
// stored as normal IMAP messages, but clicking one should resume editing rather
// than open a read-only conversation.
export async function openDraftCompose(thread: Message) {
  if (!isDraftFolder(thread.folder_id, thread.account_id)) return false
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast(t('compose.toast.addMailAccountBeforeComposing'))
    return true
  }

  if (activateOpenDraftCompose(thread)) return true

  try {
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: thread.thread_id,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const draft =
      newestMessage(
        (result.messages ?? []).filter((message) => isDraftFolder(message.folder_id, message.account_id)),
      ) ?? thread
    await openDraftMessageInCompose(draft)
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenDraft'), 'error')
  }
  return true
}

export async function openDraftConversationOrCompose(thread: Message) {
  if (!isDraftFolder(thread.folder_id, thread.account_id)) return false

  try {
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: thread.thread_id,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const messages = result.messages ?? []
    const draft =
      newestMessage(messages.filter((message) => isDraftFolder(message.folder_id, message.account_id))) ?? thread

    if (draftShouldOpenConversation(messages, draft)) {
      compose$.activeTab.set('')
      ui$.selectedThread.set(thread.thread_id)
      ui$.mobilePane.set('conversation')
      if (ui$.selectedThread.peek() === thread.thread_id) {
        mail$.messages.set(messages)
        mail$.messagesCursor.set('')
        mail$.messagesLoadingMore.set(false)
        mail$.threadLoading.set(false)
      }
      return true
    }

    if (accounts$.get().filter(isSendableAccount).length === 0) {
      showToast(t('compose.toast.addMailAccountBeforeComposing'))
      return true
    }
    await openDraftMessageInCompose(draft)
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenDraft'), 'error')
  }
  return true
}

// Open a clean compose draft for forwarding a message. Recipients and threading
// headers are intentionally blank; the original content is quoted in the body
// and non-inline attachments are copied back into the composer.
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
  })
  if (!id) return

  const valid = await readComposerAttachments(message.attachments ?? [], html)
  if (valid.length === 0) return

  const tab = compose$.tabs.get().find((t) => t.id === id)
  if (!tab?.compose) return
  updateComposeDraft(id, { attachments: [...tab.compose.attachments, ...valid] })
}

async function readComposerAttachments(attachments: Attachment[], inlineHtml: string): Promise<ComposerAttachment[]> {
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

// Merge a partial draft into a compose tab, keeping the tab label in sync with
// the subject line.
export function updateComposeDraft(id: string, partial: Partial<ComposeDraft>) {
  compose$.tabs.set(
    compose$.tabs.get().map((tab) => {
      if (tab.id !== id || !tab.compose) return tab
      const compose = { ...tab.compose, ...withSignatureForAccount(tab.compose, partial) }
      const subject = partial.subject !== undefined ? partial.subject.trim() || 'New message' : tab.subject
      return { ...tab, compose, subject }
    }),
  )
}

// Remove a tab after any compose lifecycle work has completed.
export function finishClosingMessageTab(id: string) {
  const tabs = compose$.tabs.get()
  const index = tabs.findIndex((tab) => tab.id === id)
  if (index === -1) return
  forgetComposeSession(id)
  const next = tabs.filter((tab) => tab.id !== id)
  compose$.tabs.set(next)
  if (compose$.activeTab.get() === id) {
    const target = popToPreviousTab(id, next)
    const nextTab = target ? next.find((tab) => tab.id === target) : null
    if (!nextTab) {
      activateConversationTab()
    } else if (nextTab.kind === 'thread') {
      compose$.activeTab.set(nextTab.id)
      ui$.selectedThread.set(nextTab.threadId)
    } else {
      compose$.activeTab.set(nextTab.id)
    }
  }
}

// The single close entry point used by tab buttons, hotkeys and the palette.
// Mounted composers register their save queue here so no caller can bypass it.
export async function closeMessageTab(id: string) {
  const tabs = compose$.tabs.get()
  const index = tabs.findIndex((tab) => tab.id === id)
  if (index === -1) return
  if (tabs[index].kind !== 'compose') return finishClosingMessageTab(id)
  const draft = tabs[index].compose
  if (!draft) return finishClosingMessageTab(id)
  return closeComposeSession(id, async () => {
    const remoteId = draft.draftMessageId?.startsWith('local-draft-') ? undefined : draft.draftMessageId
    try {
      if (remoteId || draft.sourceDraft) {
        await discardSavedDraftCopy(
          {
            threadId: draft.sourceDraft?.threadId ?? '',
            messageId: draft.sourceDraft?.messageId ?? '',
            folderId: draft.sourceDraft?.folderId ?? '',
            accountId: draft.accountId,
            draftMessageId: remoteId,
          },
          { throwOnError: true },
        )
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : ''
      showToast(
        message
          ? `${t('composer.status.couldNotDiscardDraft')}: ${message}`
          : t('composer.status.couldNotDiscardDraft'),
        'error',
      )
    } finally {
      finishClosingMessageTab(id)
    }
  })
}

// Reveal a message's remote content in the conversation and on any reader tab
// already open for it. The tab needs its own copy because `resetThreadView`
// clears the conversation's reveal map on the next thread switch while the tab
// stays open — and an inactive tab is unmounted, so it cannot copy this itself.
export function revealMessageRemote(messageId: string) {
  revealRemote(messageId)
  compose$.tabs.set(
    compose$.tabs
      .get()
      .map((tab) => (tab.kind === 'reader' && tab.messageId === messageId ? { ...tab, revealRemote: true } : tab)),
  )
}

export function setTabViewMode(id: string, mode: 'html' | 'plain') {
  compose$.tabs.set(compose$.tabs.get().map((tab) => (tab.id === id ? { ...tab, viewMode: mode } : tab)))
}

// Send a composed message via the same mail.send path used by replies. When
// `rich`, the HTML is sent with a derived plaintext fallback. Throws on failure
// so the caller can surface the error inline.
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
}) {
  const html = args.rich ? args.content : ''
  const body = args.rich ? htmlToText(args.content) : args.content
  await invoke('mail.send', {
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
    attachments: args.attachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      size: a.size,
      key: null,
      url: a.mime.startsWith('image/') || a.mime.startsWith('video/') ? `data:${a.mime};base64,${a.data}` : null,
    })),
  }
  mail$.messages.push(sent)
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
}): Promise<string> {
  const html = args.rich ? args.content : ''
  const body = args.rich ? htmlToText(args.content) : args.content
  const draftMessageId =
    !args.draftMessageId || args.draftMessageId.startsWith('local-draft-')
      ? await allocateMessageIdentity(args.accountId, true)
      : args.draftMessageId
  await invoke('mail.saveDraft', {
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
  return draftMessageId
}

/** Pick the source message to reply to: the most recent loaded message in the
 * active thread that wasn't sent by us — its Reply-To/Cc are the headers we
 * should honor. Falls back to the thread header when no loaded message matches. */
export function pickReplyTarget(activeT: Message): Message {
  const messages = mail$.messages.get()
  const ownAddrs = ownAddressSet(accounts$.get())
  const inThread = messages.filter((m) => m.thread_id === activeT.thread_id)
  for (let i = inThread.length - 1; i >= 0; i--) {
    const m = inThread[i]
    if (!sentByUs(m, ownAddrs)) return m
  }
  return inThread[inThread.length - 1] ?? activeT
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

/** Build the To/Cc for a reply: To is the Reply-To header (or From), Cc is
 * the source Cc minus our own address and minus anything already in To.
 *
 * When the target was sent by us (e.g. replying inside a Sent-folder thread,
 * where every message is ours), treat it as a follow-up instead: address the
 * original recipients (target.To) rather than bouncing the message back to
 * ourselves. */
export function buildReplyRecipients(target: Message, ownAddrs: Set<string>): { to: string; cc: string } {
  const isOwnTarget = ownAddrs.has((target.from_addr || '').toLowerCase())
  const replyTo = splitAddressList(target.reply_to)
  const fromEntry = target.from_name ? `${target.from_name} <${target.from_addr}>` : target.from_addr
  const toList = isOwnTarget ? splitAddressList(target.to) : replyTo.length > 0 ? replyTo : [fromEntry]
  const toAddrs = new Set(toList.map(bareAddr))

  const ccList = splitAddressList(target.cc).filter((entry) => {
    const addr = bareAddr(entry)
    return !ownAddrs.has(addr) && !toAddrs.has(addr)
  })

  return { to: toList.join(', '), cc: ccList.join(', ') }
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
    await loadThread(activeT.thread_id)
  }

  const target = pickReplyTarget(activeT)
  const ownAddrs = ownAddressSet(accounts)
  const { to, cc } = buildReplyRecipients(target, ownAddrs)
  const { in_reply_to, references } = buildReplyThreading(target)
  // Reply from the identity picked in the From indicator, else the alias the
  // original was delivered to.
  const fromEmail = resolveQuickReplyFrom(target, activeAcc)

  const text = composerText
  const prepared = prepareConversationAttachments(attachments)
  const sendAttachments = prepared.attachments
  const html = prepared.hasInlineImages ? conversationHtmlBody(text, sendAttachments) : ''
  const subject = activeT.subject.startsWith('Re:') ? activeT.subject : `Re: ${activeT.subject}`

  // Render the sent bubble optimistically — before the SMTP round-trip resolves —
  // so sending feels instant. The status starts as "sending" and flips to "sent"
  // or "failed" once the backend responds.
  const tempId = `${LOCAL_SEND_PREFIX}${Date.now()}`
  const messageId = await allocateMessageIdentity(replyAccountId, false)
  const payload: PendingSend = {
    account_id: replyAccountId,
    to,
    cc,
    subject,
    body: text,
    html,
    in_reply_to,
    references,
    from: fromEmail,
    message_id: messageId,
    attachments: sendAttachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      data: a.data,
      inline_id: a.inlineId ?? '',
    })),
  }
  const sent: Message = {
    id: tempId,
    account_id: replyAccountId,
    folder_id: activeT.folder_id,
    thread_id: activeT.thread_id,
    // Carry the real Message-ID and References chain so a follow-up reply sent
    // before this one syncs back threads against it (buildReplyThreading reads
    // message_id + references) instead of starting a fresh thread.
    message_id: messageId,
    references,
    from_name: 'You',
    from_addr: fromEmail || activeAcc?.email || '',
    to,
    cc,
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
  const savedDraftId = compose$.quickReplyDraftSaved.peek() ? compose$.quickReplyDraftId.peek() : ''
  // From this point the current quick reply belongs to the optimistic send.
  // Keep background thread refreshes from hydrating its still-persisted draft
  // back into the editor until SMTP and the post-send discard settle. This is
  // keyed by the send so navigation cannot accidentally reopen the race.
  quickReplySendHydrationGuards.set(tempId, {
    threadId: activeT.thread_id,
    accountId: replyAccountId,
    draftId: savedDraftId,
    inFlight: true,
    suppressDraft: true,
  })
  setPendingSend(tempId, payload)
  mail$.messages.push(sent)
  // Clear the composer optimistically. On failure the message stays in the pane
  // with a "failed" status (retry from the bubble or delete from the context
  // menu), so we don't restore the draft. Retry replays the stored PendingSend
  // payload above, not the (now-cleared) composer text, so clearing here
  // doesn't affect retry.
  cancelQuickReplyDraftSave()
  compose$.composerAttachments.set([])
  compose$.quickReplyDraftId.set('')
  compose$.quickReplyDraftSaved.set(false)
  // Back to a fresh quick reply rather than an empty box: the next reply in
  // this thread gets a signature just like the one just sent did.
  seedQuickReplySignature()

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
  if (!guard.draftId) {
    quickReplySendHydrationGuards.delete(tempId)
    return
  }
  const discarded = await discardSavedDraftCopy({
    threadId: guard.threadId,
    messageId: '',
    folderId: '',
    accountId: guard.accountId,
    draftMessageId: guard.draftId,
  })
  if (quickReplySendHydrationGuards.get(tempId) !== guard) return
  if (discarded) {
    quickReplySendHydrationGuards.delete(tempId)
  } else {
    settleFailedQuickReplySendGuard(tempId)
  }
}

function settleFailedQuickReplySendGuard(tempId: string) {
  const guard = quickReplySendHydrationGuards.get(tempId)
  if (!guard) return
  guard.inFlight = false
  if (ui$.selectedThread.peek() !== guard.threadId) guard.suppressDraft = false
}

// Re-attempt a previously failed send, triggered by clicking the failed bubble.
export async function retrySend(messageId: string) {
  if (!getPendingSend(messageId)) return
  await dispatchSend(messageId)
}
