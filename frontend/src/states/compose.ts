import { observable } from '@legendapp/state'
import type { Account, Attachment, ComposeDraft, ComposerAttachment, Message, MessageTab } from '../types'
import { invoke } from '../lib/bridge'
import { ui$, showToast } from './ui'
import { accounts$, isSendableAccount, accountIdentities } from './accounts'
import { mail$, getActiveThread, isDraftFolder, loadThread } from './mail'
import { LOCAL_SEND_PREFIX, type PendingSend, setPendingSend, getPendingSend, discardPendingSend } from './pendingSends'
import { htmlToText } from '../lib/html'
import { parseMailto } from '../lib/mailto'
import { splitAddressList, bareAddr } from '../lib/address'
import { formatFullTimestamp } from '../components/chat/messageHelpers'

// Compose/reader-tab + draft state. Reader tabs open using the account's
// conversation view preference; compose tabs hold a full-editor draft. The
// quick-reply composer (composer / composerAttachments) and per-thread quick
// drafts live here too. Persisted to localStorage (volatile editor buffers, not
// DB settings).

// Persisted drafts: per-thread quick-reply text and per-tab full-editor drafts.
// Attachments are intentionally NOT persisted — their base64 payloads would
// quickly blow past localStorage's ~5MB budget. Only text fields survive
// restarts; the user reattaches files if needed.
const DRAFTS_KEY = 'meron-drafts'
const COMPOSE_TABS_KEY = 'meron-compose-tabs'

/** Stable Message-ID for a draft, reused across autosaves so the server-side
 * Drafts copy is replaced rather than duplicated. */
const newDraftMessageId = () => `meron-draft-${Date.now()}-${Math.random().toString(36).substring(2, 11)}@meron`

/** Message-ID minted on the client for an outgoing send, so the optimistic
 * bubble carries the real id the Sent copy will have. A quick follow-up reply
 * can then thread against it before IMAP syncs the sent message back. The
 * domain mirrors the sender so the id looks native to the account. */
const newSendMessageId = (fromEmail: string) => {
  const domain = fromEmail.split('@')[1] || 'meron'
  return `meron-${Date.now()}-${Math.random().toString(36).substring(2, 11)}@${domain}`
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
  const rows = [
    ['From', formatMessageFrom(message)],
    ['Date', formatFullTimestamp(message.date)],
    ['Subject', message.subject || '(no subject)'],
    ['To', message.to],
    ['Cc', message.cc],
  ].filter(([, value]) => value?.trim())
  const header = rows
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

type PersistedDrafts = { quick: Record<string, string> }
function loadPersistedDrafts(): PersistedDrafts {
  try {
    const raw = localStorage.getItem(DRAFTS_KEY)
    if (!raw) return { quick: {} }
    const parsed = JSON.parse(raw)
    return { quick: parsed?.quick && typeof parsed.quick === 'object' ? parsed.quick : {} }
  } catch {
    return { quick: {} }
  }
}
const initialDrafts = loadPersistedDrafts()

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
  composer: '',
  composerAttachments: [] as ComposerAttachment[],
  // Per-thread quick-reply draft text. The active textarea reads/writes via
  // selectedThread; persisted to localStorage so a crash or restart doesn't
  // lose an in-progress reply.
  quickDrafts: initialDrafts.quick as Record<string, string>,
})

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

// Per-thread quick-reply draft persistence. Hydration (loading the draft when a
// thread is selected) lives in MessagePane so we never reassign the textarea's
// value from inside an observable listener — doing that fights React's input
// handling and drops characters under fast typing.
let draftSaveTimer: ReturnType<typeof setTimeout> | null = null
const DRAFT_SAVE_DELAY_MS = 500

/** Save the visible textarea text under `threadId` to localStorage, debounced. */
export function persistQuickDraft(threadId: string, text: string) {
  if (!threadId) return
  if (draftSaveTimer) clearTimeout(draftSaveTimer)
  draftSaveTimer = setTimeout(() => {
    draftSaveTimer = null
    const map = { ...compose$.quickDrafts.peek() }
    if (text.trim()) {
      map[threadId] = text
    } else {
      delete map[threadId]
    }
    compose$.quickDrafts.set(map)
    try {
      localStorage.setItem(DRAFTS_KEY, JSON.stringify({ quick: map }))
    } catch {
      // localStorage quota exceeded — drop silently.
    }
  }, DRAFT_SAVE_DELAY_MS)
}

/** Read the saved draft for a thread, "" when none. */
export function readQuickDraft(threadId: string): string {
  return compose$.quickDrafts.peek()[threadId] ?? ''
}

/** Discard the persisted draft for a thread after a successful send. */
export function clearQuickDraft(threadId: string) {
  if (!threadId) return
  const map = { ...compose$.quickDrafts.peek() }
  if (!(threadId in map)) return
  delete map[threadId]
  compose$.quickDrafts.set(map)
  try {
    localStorage.setItem(DRAFTS_KEY, JSON.stringify({ quick: map }))
  } catch {
    // ignore
  }
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
  ui$.selectedThread.set(thread.thread_id)
  compose$.activeTab.set(id)
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
    ui$.selectedThread.set(threadId)
    compose$.activeTab.set(id)
    ui$.mobilePane.set('conversation')
    return
  }

  try {
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', { thread_id: threadId, limit: 30 })
    const message = newestMessage(result.messages ?? [])
    if (!message) {
      showToast("Couldn't open notification thread", 'error')
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
    showToast(error instanceof Error ? error.message : "Couldn't open notification thread", 'error')
  }
}

let composeSeq = 0

// Open a full-pane compose/reply editor as a new tab. Returns silently if no
// account can send mail. `seed` pre-fills a reply (recipient, subject, body…).
export function openComposeTab(
  seed?: Partial<ComposeDraft> & { title?: string; threadId?: string },
): string | undefined {
  const sendable = accounts$.get().filter(isSendableAccount)
  if (sendable.length === 0) return undefined
  const selected = ui$.selectedAccount.get()
  const accountId = seed?.accountId ?? sendable.find((acc) => acc.id === selected)?.id ?? sendable[0].id

  const draft: ComposeDraft = {
    accountId,
    fromEmail: seed?.fromEmail ?? '',
    to: seed?.to ?? '',
    cc: seed?.cc ?? '',
    bcc: seed?.bcc ?? '',
    replyTo: seed?.replyTo ?? '',
    subject: seed?.subject ?? '',
    rich: seed?.rich ?? false,
    html: seed?.html ?? '',
    text: seed?.text ?? '',
    showCcBcc:
      seed?.showCcBcc ??
      hasExtraComposeHeaders({
        cc: seed?.cc ?? '',
        bcc: seed?.bcc ?? '',
      }),
    inReplyTo: seed?.inReplyTo ?? '',
    references: seed?.references ?? '',
    draftMessageId: seed?.draftMessageId ?? newDraftMessageId(),
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
  openComposeTab({
    accountId: t.account_id || undefined,
    fromEmail: replyAcc ? detectAliasFrom(target, replyAcc) : '',
    to,
    cc,
    showCcBcc: !!cc.trim(),
    subject,
    text: compose$.composer.get(),
    attachments: compose$.composerAttachments.get(),
    inReplyTo: in_reply_to,
    references,
    title: subject,
    threadId: t.thread_id,
  })
  compose$.composer.set('')
  compose$.composerAttachments.set([])
}

export function openMailtoCompose(raw: string) {
  const draft = parseMailto(raw)
  if (!draft) return
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast('Add a mail account before composing')
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
    showToast('Add a mail account before composing')
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

function composeFromDraftMessage(message: Message): Partial<ComposeDraft> & { title?: string; threadId?: string } {
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
  }
}

// Restore a saved server-side Drafts row into the full composer. Drafts are
// stored as normal IMAP messages, but clicking one should resume editing rather
// than open a read-only conversation.
export async function openDraftCompose(thread: Message) {
  if (!isDraftFolder(thread.folder_id)) return false
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast('Add a mail account before composing')
    return true
  }

  try {
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', { thread_id: thread.thread_id, limit: 30 })
    const draft = newestMessage((result.messages ?? []).filter((message) => isDraftFolder(message.folder_id))) ?? thread
    const id = openComposeTab(composeFromDraftMessage(draft))
    if (!id) return true

    const valid = await readComposerAttachments(draft.attachments ?? [], draft.body_html ?? '')
    if (valid.length > 0) {
      const tab = compose$.tabs.get().find((t) => t.id === id)
      if (tab?.compose) updateComposeDraft(id, { attachments: [...tab.compose.attachments, ...valid] })
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : "Couldn't open draft", 'error')
  }
  return true
}

// Open a clean compose draft for forwarding a message. Recipients and threading
// headers are intentionally blank; the original content is quoted in the body
// and non-inline attachments are copied back into the composer.
export async function forwardMessage(message: Message) {
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast('Add a mail account before composing')
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
      const compose = { ...tab.compose, ...partial }
      const subject = partial.subject !== undefined ? partial.subject.trim() || 'New message' : tab.subject
      return { ...tab, compose, subject }
    }),
  )
}

// Close a reader tab, activating the previous tab (or the conversation view).
export function closeMessageTab(id: string) {
  const tabs = compose$.tabs.get()
  const index = tabs.findIndex((tab) => tab.id === id)
  if (index === -1) return
  const next = tabs.filter((tab) => tab.id !== id)
  compose$.tabs.set(next)
  if (compose$.activeTab.get() === id) {
    const nextTab = next[index - 1] ?? next[index] ?? null
    if (nextTab?.kind === 'thread') ui$.selectedThread.set(nextTab.threadId)
    compose$.activeTab.set(nextTab?.id ?? '')
  }
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
  const html = args.rich ? args.content : ''
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
}) {
  const html = args.rich ? args.content : ''
  const body = args.rich ? htmlToText(args.content) : args.content
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
    draft_id: args.draftMessageId,
    attachments: args.attachments.map((a) => ({
      filename: a.filename,
      mime: a.mime,
      data: a.data,
      inline_id: a.inlineId ?? '',
    })),
  })
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
    if (!ownAddrs.has((m.from_addr || '').toLowerCase())) return m
  }
  return inThread[inThread.length - 1] ?? activeT
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
  const composerText = compose$.composer.get()
  const attachments = compose$.composerAttachments.get()
  const activeT = getActiveThread()
  const selectedAcc = ui$.selectedAccount.get()

  if ((!composerText.trim() && attachments.length === 0) || !activeT) return

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
  // Reply from the alias the original was delivered to, when there is one.
  const fromEmail = activeAcc ? detectAliasFrom(target, activeAcc) : ''

  const text = composerText
  const prepared = prepareConversationAttachments(attachments)
  const sendAttachments = prepared.attachments
  const html = prepared.hasInlineImages ? conversationHtmlBody(text, sendAttachments) : ''
  const subject = activeT.subject.startsWith('Re:') ? activeT.subject : `Re: ${activeT.subject}`

  // Render the sent bubble optimistically — before the SMTP round-trip resolves —
  // so sending feels instant. The status starts as "sending" and flips to "sent"
  // or "failed" once the backend responds.
  const tempId = `${LOCAL_SEND_PREFIX}${Date.now()}`
  const messageId = newSendMessageId(fromEmail || activeAcc?.email || '')
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
  setPendingSend(tempId, payload)
  mail$.messages.push(sent)
  // Clear the composer optimistically. On failure the message stays in the pane
  // with a "failed" status (retry from the bubble or delete from the context
  // menu), so we don't restore the draft.
  compose$.composer.set('')
  compose$.composerAttachments.set([])
  clearQuickDraft(activeT.thread_id)

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
  setSendStatus(tempId, 'sending')
  try {
    await invoke('mail.send', payload)
    discardPendingSend(tempId)
    setSendStatus(tempId, 'sent')
  } catch (error) {
    setSendStatus(tempId, 'failed')
    showToast(error instanceof Error ? error.message : 'Send failed', 'error')
  }
}

// Re-attempt a previously failed send, triggered by clicking the failed bubble.
export async function retrySend(messageId: string) {
  if (!getPendingSend(messageId)) return
  await dispatchSend(messageId)
}
