import { forgetThreadView } from './threadView'
import { arrayMove } from '@dnd-kit/sortable'
import { t } from '../lib/i18n'
import { isNarrowWindow } from '../lib/useNarrowWindow'
import type { Account, Attachment, ComposeDraft, ComposerAttachment, Message, MessageTab } from '../types'
import { invoke } from '../lib/bridge'
import { CONVERSATION_PAGE_SIZE } from '../lib/pagination'
import { ui$, showToast, showUndoToast } from './ui'
import { contextualErrorMessage } from '../lib/errors'
import { inlineRichStyles, prepareInlineImages } from '../components/composer/composerHelpers'
import { accounts$, isSendableAccount, accountIdentities } from './accounts'
import { getActiveThread, normalizeMessageId } from './mail'
// From the leaf module: this one subscribes to it while loading, which can be
// inside an import cycle through mail.ts.
import { mail$ } from './mailState'
import { isDraftFolder } from './mailFolders'
import { LOCAL_SEND_PREFIX } from './pendingSends'
import { htmlToText, resolveInlineCids } from '../lib/html'
import { parseMailto } from '../lib/mailto'
import {
  bodyWithSignature,
  bodyWithSwappedSignature,
  type ComposeBody,
  isBodyBlankWithoutSignature,
  resolveSignature,
  signatureForms,
  type Signature,
  type SignaturePlacement,
  type SignatureTracking,
} from '../lib/signature'
import { settings$ } from './settings'
import { closeCurrentConversation, kanban$ } from './kanban'
import { resolveConversationMode, revealRemote, thread$ } from './thread'
import { formatFullTimestamp } from '../components/chat/messageHelpers'
import {
  closeComposeSession,
  draftSavesSettled,
  forgetComposeSession,
  getComposeSession,
  pruneComposeSessions,
  type SavedDraft,
  trackDraftSave,
} from './composeSessions'
import {
  buildReplyRecipients,
  quickReplyAddressing,
  buildReplyThreading,
  detectAliasFrom,
  detectSenderFrom,
  pickReplyTarget,
  resolveQuickReplyFrom,
} from './composeReply'
import {
  allocateMessageIdentity,
  compose$,
  COMPOSE_TABS_KEY,
  hasExtraComposeHeaders,
  newDraftMessageId,
  TAB_VIEW_KEY,
  type PersistedComposeTab,
  type PersistedMailTab,
} from './composeState'
import {
  cancelQuickReplyDraftSave,
  clearQuickReplyDraftOwnership,
  discardClosedDraft,
  discardingDraftIds,
  quickReplyAbandonedSaves,
  nextQuickReplyOwnershipTick,
  quickReplyDraftSaveInFlight,
  quickReplyWithoutSignature,
  reserveOpeningDraft,
  unconfirmedQuickReplyDraftId,
  seedQuickReplySignature,
} from './quickReply'

// Reader and compose tabs: opening threads and drafts, the full editor, forwarding,
// and saving or sending composed messages.

export function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

export function textToHtml(text: string): string {
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

/**
 * The message a reply answers, quoted under it, when the user quotes replies
 * (`settings.quoteReplies`): an "On …, Jane wrote:" line over the original,
 * `> `-prefixed in plain text and in a `<blockquote type="cite">` in HTML —
 * the Apple Mail and Thunderbird shape, which the conversation view folds in
 * our own sent replies as it does in theirs. `rich` says whether the original
 * was HTML. Undefined when quoting is off or the original has no body loaded.
 */
export function replyQuote(message: Message): ComposeBody | undefined {
  if (!settings$.quoteReplies.peek()) return undefined
  const body = message.body ?? ''
  if (!message.body_html && !body.trim()) return undefined
  const attribution = `On ${formatFullTimestamp(message.date)}, ${formatMessageFrom(message)} wrote:`
  const quoted = body
    .replace(/\s+$/, '')
    .split('\n')
    .map((line) => (line ? `> ${line}` : '>'))
    .join('\n')
  return {
    rich: !!message.body_html,
    html: `<p>${escapeHtml(attribution)}</p><blockquote type="cite">${message.body_html || textToHtml(body)}</blockquote>`,
    text: `${attribution}\n${quoted}`,
  }
}

/** Whether a body already quotes the message `quote` answers — a saved quoted
 * reply carried back into a reply — by the attribution line it opens with. */
export function bodyHasReplyQuote(body: string, quote: ComposeBody): boolean {
  const attribution = quote.text.split('\n', 1)[0]
  return !!attribution && body.includes(attribution)
}

/** What a draft sends and saves: its body, with a folded quote under it. */
export function composedContent(draft: Pick<ComposeDraft, 'rich' | 'html' | 'text' | 'quote'>): string {
  const own = draft.rich ? draft.html : draft.text
  if (!draft.quote) return own
  if (draft.rich) return `${own}${draft.quote.html}`
  return own.trim() ? `${own.replace(/\s+$/, '')}\n\n${draft.quote.text}` : draft.quote.text
}

/**
 * A reopened draft's body with the quote at its tail folded away again, as it
 * was while the reply was being written. The core marks that tail when it reads
 * the draft back: `data-meron-quote` on the HTML elements, `body_quote_start`
 * in the plain body. A draft without one comes back whole.
 */
function foldDraftQuote(message: Message): ComposeBody & { quote?: ComposeDraft['quote'] } {
  if (message.body_html) {
    const doc = new DOMParser().parseFromString(message.body_html, 'text/html')
    // The core marks only a quote that ends the message, so everything from the
    // first marked element to the end of the document is the quote — at the top
    // level, inside a wrapper the reply shares with it, or (Outlook) running on
    // past that wrapper. The range splits whatever wrappers it crosses.
    const first = doc.body.querySelector('[data-meron-quote]')
    if (!first) return { rich: true, html: message.body_html, text: '' }
    const from = (node: Node) => {
      const siblings: Node[] = []
      for (let at: Node | null = node; at; at = at.nextSibling) siblings.push(at)
      return siblings
    }
    let tail = from(first)
    for (let wrapper = first.parentNode; wrapper && wrapper !== doc.body; wrapper = wrapper.parentNode) {
      const following = wrapper.nextSibling ? from(wrapper.nextSibling) : []
      const part = wrapper.cloneNode(false)
      for (const node of tail) part.appendChild(node)
      tail = [part, ...following]
    }
    const folded = doc.createElement('div')
    for (const node of tail) folded.appendChild(node)
    for (const element of Array.from(folded.querySelectorAll('[data-meron-quote]'))) {
      element.removeAttribute('data-meron-quote')
    }
    const html = folded.innerHTML.trim()
    return { rich: true, html: doc.body.innerHTML, text: '', quote: { html, text: htmlToText(html) } }
  }
  const body = message.body ?? ''
  const start = message.body_quote_start
  if (start == null || start <= 0 || start >= body.length) return { rich: false, html: '', text: body }
  const text = body.slice(start)
  return {
    rich: false,
    html: '',
    text: body.slice(0, start).replace(/\s+$/, ''),
    quote: { html: `<blockquote type="cite">${textToHtml(text)}</blockquote>`, text },
  }
}

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

// Persist the tabs on every tab change. We only store the text portion of each
// compose draft — attachments are dropped because their base64 payloads can blow
// past localStorage's quota. On boot the tabs come back; the user reattaches
// files if needed. Thread and reader tabs are stored as references.
const threadTabScopes = (tabs: MessageTab[]) =>
  new Set(tabs.filter((tab) => tab.kind === 'thread').map((tab) => `${tab.id}:${tab.threadId}`))
let previousThreadTabScopes = threadTabScopes(compose$.tabs.peek())
compose$.tabs.onChange(({ value: tabs }) => {
  const scopes = threadTabScopes(tabs)
  for (const key of previousThreadTabScopes) if (!scopes.has(key)) forgetThreadView(key)
  previousThreadTabScopes = scopes

  pruneComposeSessions(new Set(tabs.map((tab) => tab.id)))
  const persisted = tabs.flatMap<PersistedComposeTab | PersistedMailTab>((t) => {
    if (t.kind !== 'compose') {
      const { id, kind, messageId, threadId, accountId, folderId, subject, from, fromAddr, feedIcon, viewMode } = t
      return [{ id, kind, messageId, threadId, accountId, folderId, subject, from, fromAddr, feedIcon, viewMode }]
    }
    if (!t.compose) return []
    return [{ id: t.id, subject: t.subject, compose: { ...t.compose, attachments: [] } }]
  })
  try {
    localStorage.setItem(COMPOSE_TABS_KEY, JSON.stringify(persisted))
  } catch {
    // localStorage quota exceeded — drop silently.
  }
})

function persistTabView() {
  try {
    localStorage.setItem(
      TAB_VIEW_KEY,
      JSON.stringify({
        activeTab: compose$.activeTab.peek(),
        conversationThread: compose$.conversationThread.peek(),
      }),
    )
  } catch {
    // localStorage quota exceeded — drop silently.
  }
}
compose$.activeTab.onChange(persistTabView)
compose$.conversationThread.onChange(persistTabView)

// Keep reader snapshots in sync with the loaded conversation: the body when
// background fetching completes, and the read and star flags whenever they
// change there, so the tab's menu still toggles the right way once that
// conversation is no longer loaded.
mail$.messages.onChange(({ value: messages }) => {
  const byId = new Map(messages.map((message) => [message.id, message]))
  for (const [index, tab] of compose$.tabs.peek().entries()) {
    if (tab.kind !== 'reader') continue
    const message = byId.get(tab.messageId)
    if (!message) continue
    if (tab.bodyMissing && !message.body_missing) {
      compose$.tabs[index].assign({
        body: message.body,
        bodyHtml: message.body_html,
        bodyMissing: false,
        attachments: message.attachments,
        // The bottom bar forwards from this: without the body it would send
        // an empty forward with none of the attachments the reader now shows.
        message,
      })
    } else if (tab.message && (tab.message.unread !== message.unread || tab.message.starred !== message.starred)) {
      compose$.tabs[index].assign({ message: { ...tab.message, unread: message.unread, starred: message.starred } })
    }
  }
})

// Open a single message in its own reader tab. The HTML is already on the
// message (shipped with threadRead), so this is instant — no fetch. Re-opening
// an already-open message just re-activates its tab.
// What the tab strip's avatar needs, as ThreadListItem draws it: a feed item
// shows its feed's icon rather than resolving the sender address.
function tabAvatar(message: Message): Pick<MessageTab, 'fromAddr' | 'feedIcon'> {
  if (message.feed_url) return { feedIcon: message.feed_icon }
  return { fromAddr: message.from_addr }
}

function readerTab(message: Message): MessageTab {
  const account = accounts$.get().find((acc) => acc.id === message.account_id)
  const preferHtml = account?.conversation_html ?? true
  return {
    id: message.id,
    kind: 'reader',
    messageId: message.id,
    threadId: message.thread_id,
    // Carried so the tab can resolve this message's remote-content policy.
    accountId: message.account_id,
    revealRemote: !!thread$.revealedRemote.peek()[message.id],
    // Snapshotted at open time: the From address alone misses an alias or a
    // send still in flight (the same rule useMessageView applies in the
    // conversation).
    outgoing:
      !!message.send_status ||
      message.outgoing === true ||
      (!!account &&
        accountIdentities(account).some(
          (identity) => identity.email.trim().toLowerCase() === message.from_addr.trim().toLowerCase(),
        )),
    subject: message.subject || '(no subject)',
    from: message.from_name || message.from_addr,
    ...tabAvatar(message),
    fromRaw: message.from_name ? `${message.from_name} <${message.from_addr}>` : message.from_addr,
    to: message.to,
    cc: message.cc,
    bcc: message.bcc,
    replyTo: message.reply_to,
    date: message.date,
    body: message.body,
    bodyHtml: message.body_html,
    bodyMissing: message.body_missing,
    attachments: message.attachments,
    viewMode: message.body_html && preferHtml ? 'html' : 'plain',
    message,
  }
}

export function openMessageTab(message: Message) {
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  const existing = compose$.tabs.get().find((tab) => tab.messageId === message.id)
  if (existing) {
    compose$.activeTab.set(existing.id)
    return
  }
  const tab = readerTab(message)
  compose$.tabs.push(tab)
  compose$.activeTab.set(tab.id)
}

// Put the persisted view back on screen. Call once at boot, after the kanban
// session: a board's conversation is its open card. The conversation behind a
// reader or compose tab stays unloaded until the user returns to it, so
// restoring never reads a thread that isn't on screen.
export function restoreTabSession() {
  const onBoard = !!kanban$.activeBoardId.peek()
  if (onBoard) {
    compose$.conversationThread.set(kanban$.paneThreadId.peek())
  } else {
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
  }
  const active = compose$.tabs.peek().find((tab) => tab.id === compose$.activeTab.peek())
  const conversation = compose$.conversationThread.peek()
  if (active?.kind === 'thread') ui$.selectedThread.set(active.threadId)
  else if (!active && conversation) ui$.selectedThread.set(conversation)
  else if (!active) closeCurrentConversation()
  if (compose$.activeTab.peek() || conversation) ui$.mobilePane.set('conversation')
  void reconcileRestoredTabs()
}

async function restoredThreadPage(threadId: string, cursor: string) {
  return invoke<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
    thread_id: threadId,
    limit: CONVERSATION_PAGE_SIZE,
    ...(cursor ? { before_cursor: cursor } : {}),
  })
}

// Check each restored conversation against the mailbox, one at a time: anything
// deleted or moved while the app was closed closes, and reader tabs get their
// bodies back. A failed read proves nothing (offline, say), so it closes nothing.
async function reconcileRestoredTabs() {
  const threadIds = new Set(
    compose$.tabs
      .peek()
      .filter((tab) => tab.kind !== 'compose')
      .map((tab) => tab.threadId),
  )
  if (compose$.conversationThread.peek()) threadIds.add(compose$.conversationThread.peek())
  for (const threadId of threadIds) {
    try {
      const wanted = new Set(
        compose$.tabs
          .peek()
          .filter((tab) => tab.kind === 'reader' && tab.threadId === threadId)
          .map((tab) => tab.messageId),
      )
      const found = new Map<string, Message>()
      const cursors = new Set<string>()
      let cursor = ''
      let empty = true
      do {
        cursors.add(cursor)
        const page = await restoredThreadPage(threadId, cursor)
        if (page.messages.length > 0) empty = false
        for (const message of page.messages) if (wanted.has(message.id)) found.set(message.id, message)
        cursor = page.next_cursor ?? ''
      } while (cursor && !cursors.has(cursor) && found.size < wanted.size)

      const tabs = compose$.tabs.peek()
      closeMessageTabs(
        tabs
          .filter((tab) => tab.threadId === threadId && (empty || (tab.kind === 'reader' && !found.has(tab.messageId))))
          .map((tab) => tab.id),
      )
      if (empty && compose$.conversationThread.peek() === threadId) closeCurrentConversation()
      for (const [index, tab] of compose$.tabs.peek().entries()) {
        const message = tab.kind === 'reader' ? found.get(tab.messageId) : undefined
        if (message) compose$.tabs[index].assign({ ...readerTab(message), viewMode: tab.viewMode })
      }
    } catch (error) {
      console.error('restoring tab failed', error)
    }
  }
}

// Open a conversation itself as a tab. The tab stores enough thread metadata to
// render even when the current mailbox/rail selection does not contain it.
export function openThreadTab(thread: Message) {
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  // The conversation shows in the mail pane, which the calendar replaces —
  // even when its tab is the active one already, so no tab change says so.
  ui$.calendarOpen.set(false)
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
      ...tabAvatar(thread),
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

export function newestMessage(messages: Message[]): Message | null {
  return messages.reduce<Message | null>((newest, message) => {
    if (!newest) return message
    return message.date > newest.date ? message : newest
  }, null)
}

/**
 * Open a thread in its own tab from an id alone — a notification tap, a task's
 * mail link. `notFoundMessage` is the toast for a thread the read cannot find
 * (deleted, expunged), since each caller knows what the user was pointing at.
 */
export async function openThreadTabById(
  threadId: string,
  notFoundMessage = t('compose.toast.couldNotOpenNotificationThread'),
) {
  if (!threadId) return
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  // The conversation shows in the mail pane, which the calendar replaces —
  // even when its tab is the active one already, so no tab change says so.
  ui$.calendarOpen.set(false)
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
      showToast(notFoundMessage, 'error')
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
    showToast(error instanceof Error ? error.message : notFoundMessage, 'error')
  }
}

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
    quote: quote ? { html: quote.html, text: quote.text } : undefined,
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
export function openReplyInFullEditor(options?: { replyAll?: boolean }) {
  const t = getActiveThread()
  if (!t) return
  const subject = t.subject.startsWith('Re:') ? t.subject : `Re: ${t.subject}`
  const target = pickReplyTarget(t)
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
  // The seeded signature is handed over stripped, so the full composer inserts
  // and tracks its own copy — the account is the same, so this is the identical
  // text, now swappable if the draft later changes identity. When it can't be
  // found the user has written into it: it stays in the body as theirs, and the
  // composer is told not to add a second.
  const carried = quickReplyWithoutSignature()
  // A bar holding a saved quoted reply already carries its quote.
  const quote = replyQuote(target)
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
    quote: quote && !bodyHasReplyQuote(carried.text, quote) ? quote : undefined,
  })
  compose$.composerAttachments.set([])
  clearQuickReplyDraftOwnership()
  compose$.quickReplyFrom.set('')
  compose$.quickReplyAdded.set([])
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
  })
  if (!id) return

  await fetchComposeTabAttachments(id, message.attachments ?? [], rich ? html : '')
}

// Fill a tab just opened with a message's attachments. Until they land the
// session holds the fetch, so an Esc save can wait for them: it would otherwise
// keep the message without its attachments, and a saved draft replaced with
// that copy would lose them for good. The tab may have been edited or closed
// meanwhile; bail if it's gone, otherwise merge onto what it now holds.
async function fetchComposeTabAttachments(id: string, attachments: Attachment[], inlineHtml: string) {
  const session = getComposeSession(id)
  const fetch = readComposerAttachments(attachments, inlineHtml)
  session.attachmentFetches.add(fetch)
  const valid = await fetch.finally(() => session.attachmentFetches.delete(fetch))
  if (valid.length === 0) return
  const tab = compose$.tabs.get().find((t) => t.id === id)
  if (!tab?.compose) return
  updateComposeDraft(id, { attachments: [...tab.compose.attachments, ...valid] })
}

function composeFromDraftMessage(message: Message): ComposeSeed {
  const body = foldDraftQuote(message)
  return {
    accountId: message.account_id || undefined,
    fromEmail: message.from_addr ?? '',
    to: message.to ?? '',
    cc: message.cc ?? '',
    bcc: message.bcc ?? '',
    replyTo: message.reply_to ?? '',
    showCcBcc: hasExtraComposeHeaders({ cc: message.cc ?? '', bcc: message.bcc ?? '' }),
    subject: message.subject ?? '',
    rich: body.rich,
    html: body.html,
    text: body.text,
    quote: body.quote && { rich: body.rich, ...body.quote },
    inReplyTo: '',
    references: message.references ?? '',
    // Not an id already being expunged: that copy is going away, so this tab's
    // content belongs in a draft of its own.
    draftMessageId:
      message.message_id && !discardingDraftIds.has(normalizeMessageId(message.message_id))
        ? message.message_id
        : newDraftMessageId(),
    sourceDraft: {
      threadId: message.thread_id,
      messageId: message.id,
      folderId: message.folder_id,
    },
    title: message.subject || 'New message',
    noSignature: true,
  }
}

/** Whether a compose draft is the one this Drafts row holds a copy of. */
function composeHoldsDraft(compose: ComposeDraft, draft: Message): boolean {
  const source = compose.sourceDraft
  return (
    (!!draft.message_id && normalizeMessageId(compose.draftMessageId) === normalizeMessageId(draft.message_id)) ||
    source?.messageId === draft.id ||
    (source?.threadId === draft.thread_id && source?.folderId === draft.folder_id)
  )
}

function activateOpenDraftCompose(draft: Message): boolean {
  const existing = compose$.tabs
    .get()
    .find((tab) => tab.kind === 'compose' && !!tab.compose && composeHoldsDraft(tab.compose, draft))
  if (!existing) return false
  compose$.activeTab.set(existing.id)
  return true
}

/**
 * Before opening a Drafts row, let this draft's Esc save finish. The row can be
 * the copy that save is replacing, and a tab opened from it would autosave the
 * older text over the newer. The tab comes back as it closed, as the store may
 * not hold the new copy yet; when the save failed, the tab is already back.
 * True when a tab now holds the draft.
 */
async function reopenAfterDraftSaves(draft: Message): Promise<boolean> {
  const [saved] = await draftSavesSettled((compose) => composeHoldsDraft(compose, draft))
  saved?.reopen()
  return !!saved || activateOpenDraftCompose(draft)
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

  await fetchComposeTabAttachments(id, draft.attachments ?? [], draft.body_html ?? '')
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

  const release = reserveOpeningDraft(thread)
  try {
    if (await reopenAfterDraftSaves(thread)) return true
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: thread.thread_id,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const draft =
      newestMessage(
        (result.messages ?? []).filter((message) => isDraftFolder(message.folder_id, message.account_id)),
      ) ?? thread
    // The read can resolve to a copy under a different id; hold that one too for
    // the (synchronous) hop to the tab that now owns it.
    const releaseResolved = reserveOpeningDraft(draft)
    try {
      await openDraftMessageInCompose(draft)
    } finally {
      releaseResolved()
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenDraft'), 'error')
  } finally {
    release()
  }
  return true
}

export async function openDraftConversationOrCompose(thread: Message) {
  if (!isDraftFolder(thread.folder_id, thread.account_id)) return false

  const release = reserveOpeningDraft(thread)
  try {
    if (await reopenAfterDraftSaves(thread)) return true
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: thread.thread_id,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const messages = result.messages ?? []
    const draft =
      newestMessage(messages.filter((message) => isDraftFolder(message.folder_id, message.account_id))) ?? thread

    if (draftShouldOpenConversation(messages, draft)) {
      // This one opens a conversation, not an editor: hand the draft to the
      // quick reply by releasing before the messages land, or the hold that was
      // protecting it would suppress the very hydration that takes it over.
      release()
      compose$.activeTab.set('')
      ui$.selectedThread.set(thread.thread_id)
      ui$.mobilePane.set('conversation')
      if (ui$.selectedThread.peek() === thread.thread_id) {
        mail$.messages.set(messages)
        mail$.messagesCursor.set('')
        mail$.messagesCursorThread.set(thread.thread_id)
        mail$.messagesLoadingMore.set(false)
        mail$.threadLoading.set(false)
      }
      return true
    }

    if (accounts$.get().filter(isSendableAccount).length === 0) {
      showToast(t('compose.toast.addMailAccountBeforeComposing'))
      return true
    }
    const releaseResolved = reserveOpeningDraft(draft)
    try {
      await openDraftMessageInCompose(draft)
    } finally {
      releaseResolved()
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenDraft'), 'error')
  } finally {
    release()
  }
  return true
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
  const subject = message.subject.startsWith('Re:') ? message.subject : `Re: ${message.subject}`
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

// Remove a tab once saves have drained; remote cleanup may still be running.
export function finishClosingMessageTab(id: string) {
  const tabs = compose$.tabs.get()
  const index = tabs.findIndex((tab) => tab.id === id)
  if (index === -1) return
  forgetComposeSession(id)
  const next = tabs.filter((tab) => tab.id !== id)
  compose$.tabs.set(next)
  if (compose$.activeTab.get() === id) {
    const target = popToPreviousTab(id, next)
    // Falling back to the Current tab needs a conversation to fall back to.
    // With none — a tab opened from tasks or a notification over an empty pane
    // — Current is not even offered in the strip (and in kanban view it would
    // close the pane), so hand over to a tab that's still open instead.
    const fallback = !target && !compose$.conversationThread.get() ? next[next.length - 1]?.id : ''
    const resolved = fallback || target
    const nextTab = resolved ? next.find((tab) => tab.id === resolved) : null
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

// Bring a closed compose tab back as it was, or go to the tab already holding
// that draft: it may have been reopened from Drafts in the meantime.
export function reopenComposeTab(tab: MessageTab, compose: ComposeDraft) {
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  // The composer opens in the mail pane, which the calendar replaces.
  ui$.calendarOpen.set(false)
  const draftId = normalizeMessageId(compose.draftMessageId)
  const open = compose$.tabs
    .peek()
    .find((t) => t.id === tab.id || (t.compose && normalizeMessageId(t.compose.draftMessageId) === draftId))
  if (!open) compose$.tabs.set([...compose$.tabs.peek(), { ...tab, compose }])
  compose$.activeTab.set(open?.id ?? tab.id)
}

// Bring back a compose tab whose last save failed. The tab is the only copy of
// these edits, so it always comes back. If the draft was reopened from Drafts
// meanwhile, that tab holds an older copy under the same id: this one becomes a
// draft of its own, so neither can overwrite or discard the other.
export function restoreComposeTabAfterFailedSave(tab: MessageTab, compose: ComposeDraft) {
  ui$.calendarOpen.set(false)
  const draftId = normalizeMessageId(compose.draftMessageId)
  const held = compose$.tabs.peek().some((t) => t.compose && normalizeMessageId(t.compose.draftMessageId) === draftId)
  const restored = held ? { ...compose, draftMessageId: newDraftMessageId(), sourceDraft: undefined } : compose
  compose$.tabs.set([...compose$.tabs.peek(), { ...tab, compose: restored }])
  compose$.activeTab.set(tab.id)
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
    finishClosingMessageTab(id)
    if (remoteId || draft.sourceDraft) {
      await discardClosedDraft({
        threadId: draft.sourceDraft?.threadId ?? '',
        messageId: draft.sourceDraft?.messageId ?? '',
        folderId: draft.sourceDraft?.folderId ?? '',
        accountId: draft.accountId,
        draftMessageId: remoteId,
        // A reply escalated out of a conversation has no draft row of its
        // own to take the card away with it; the conversation's card stays
        // and only its Draft badge goes.
        replyThreadId: draft.sourceDraft ? '' : tabs[index].threadId,
      })
    }
  })
}

// Close several tabs at once, e.g. those showing a renamed folder's mail.
// Compose tabs are skipped: closing one discards its draft, which a bulk close
// must never do behind the user's back. (The tab strip's bulk close uses
// closeMessageTabsKeepingDrafts, which saves them to Drafts instead.)
export function closeMessageTabs(ids: string[]) {
  const wanted = new Set(ids)
  const closing = compose$.tabs
    .get()
    .filter((tab) => wanted.has(tab.id) && tab.kind !== 'compose')
    .map((tab) => tab.id)
  // Close the active tab last, so it falls back to a tab that stays open.
  const active = compose$.activeTab.get()
  for (const id of closing) if (id !== active) finishClosingMessageTab(id)
  if (closing.includes(active)) finishClosingMessageTab(active)
}

/**
 * A new message (not a reply, forward or reopened draft) with no recipients,
 * subject or attachments and nothing in its body but the signature, if any.
 */
export function isUnwrittenNewMessage(draft: ComposeDraft): boolean {
  return (
    !draft.sourceDraft &&
    !draft.inReplyTo &&
    !draft.quote &&
    !draft.to.trim() &&
    !draft.cc.trim() &&
    !draft.bcc.trim() &&
    !draft.subject.trim() &&
    draft.attachments.length === 0 &&
    isBodyBlankWithoutSignature(draft, draft.signature)
  )
}

/**
 * Close a compose tab but keep the message in Drafts (Esc, and the tab strip's
 * bulk close). This works whether or not the tab's composer is mounted: the
 * save runs from the draft the tab holds. The tab closes at once, since a
 * Drafts server can take seconds. The last save then runs from a copy of the
 * draft, after any save already running (which, finding the tab gone, cleans
 * up any copy it allocated). If the save fails, the tab comes back with the
 * error, so nothing typed is lost.
 *
 * A new message with nothing written in it goes through `discard` instead of
 * filling Drafts with empty mail. Returns the save, which resolves to the
 * saved draft (null when nothing was saved), or null when the tab can't
 * close this way (no account, or already sending or closing).
 */
export function closeComposeTabKeepingDraft(
  tabId: string,
  discard: () => Promise<void>,
): Promise<SavedDraft | null> | null {
  const session = getComposeSession(tabId)
  const closed = compose$.tabs.peek().find((t) => t.id === tabId)
  const current = closed?.compose
  if (!closed || !current?.accountId || session.savesStopped) return null
  const fetches = [...session.attachmentFetches]
  if (fetches.length === 0 && isUnwrittenNewMessage(current)) return discard().then(() => null)
  // Stop autosaving and let whatever is already running finish first.
  session.savesStopped = true
  const drained = session.saveChain
  finishClosingMessageTab(tabId)
  const save = drained.then(async (): Promise<SavedDraft | null> => {
    // The attachments still on their way, which the closed tab can't take in.
    const fetched = (await Promise.all(fetches)).flat()
    const closing = { ...current, attachments: [...current.attachments, ...fetched] }
    try {
      let content = composedContent(current)
      let attachments = closing.attachments
      if (current.rich) {
        const prepared = await prepareInlineImages(content, attachments)
        content = inlineRichStyles(prepared.html)
        attachments = prepared.attachments
      }
      const draftMessageId = await saveComposedDraft({
        accountId: current.accountId,
        from: current.fromEmail,
        to: current.to.trim(),
        cc: current.cc.trim(),
        bcc: current.bcc.trim(),
        replyTo: current.replyTo.trim(),
        subject: current.subject.trim(),
        rich: current.rich,
        content,
        inReplyTo: current.inReplyTo,
        references: current.references,
        draftMessageId: current.draftMessageId,
        attachments,
      })
      return { draftMessageId, reopen: () => reopenComposeTab(closed, { ...closing, draftMessageId }) }
    } catch (err) {
      console.error('Saving the draft on close failed:', err)
      // Reopened under the id the failed save was writing, so a copy that
      // landed anyway is replaced by the next save rather than duplicated.
      const draftMessageId = draftIdOfFailedSave(err) ?? current.draftMessageId
      restoreComposeTabAfterFailedSave(closed, { ...closing, draftMessageId })
      showToast(contextualErrorMessage(err, t('composer.status.draftAutosaveFailed')), 'error')
      return null
    }
  })
  trackDraftSave(save, current)
  return save
}

/** Says where closed drafts went, with a way to bring them all back. */
export function announceSavedDrafts(saved: SavedDraft[]) {
  if (saved.length === 0) return
  showUndoToast(
    t('composer.status.savedToDrafts'),
    () => {
      for (const draft of saved) draft.reopen()
    },
    'app',
    t('composer.actions.openDraft'),
  )
}

/**
 * The tab strip's close others/left/right/all. Unlike closeMessageTabs, this
 * closes compose tabs too, keeping their messages in Drafts as Esc does (never
 * discarding them, as the × does). One toast covers every saved draft.
 */
export function closeMessageTabsKeepingDrafts(ids: string[]) {
  const wanted = new Set(ids)
  const closing = compose$.tabs.get().filter((tab) => wanted.has(tab.id))
  // Close the active tab last, so it falls back to a tab that stays open.
  const active = compose$.activeTab.get()
  const ordered = [...closing.filter((tab) => tab.id !== active), ...closing.filter((tab) => tab.id === active)]
  const saves: Promise<SavedDraft | null>[] = []
  for (const tab of ordered) {
    if (tab.kind !== 'compose' || !tab.compose) {
      finishClosingMessageTab(tab.id)
      continue
    }
    // A tab already closing (its × or Esc) is left to that close.
    if (getComposeSession(tab.id).closing) continue
    const save = closeComposeTabKeepingDraft(tab.id, () => closeMessageTab(tab.id))
    if (save) saves.push(save)
  }
  if (saves.length === 0) return
  void Promise.all(saves).then((results) => announceSavedDrafts(results.filter((saved) => saved !== null)))
}

// Put a tab where another one sits, for reordering the strip. Either may have
// closed during the drag, in which case the order stands.
export function moveMessageTab(id: string, overId: string) {
  const tabs = compose$.tabs.get()
  const from = tabs.findIndex((tab) => tab.id === id)
  const to = tabs.findIndex((tab) => tab.id === overId)
  if (from === -1 || to === -1 || from === to) return
  compose$.tabs.set(arrayMove(tabs, from, to))
}

// Reveal a message's remote content in the conversation and on any reader tab
// already open for it. The tab keeps its own copy, which printing reads, and
// which outlives the session map being cleared when an account is removed — an
// inactive tab is unmounted, so it cannot copy this itself.
export function revealMessageRemote(messageId: string) {
  revealRemote(messageId)
  compose$.tabs.set(
    compose$.tabs
      .get()
      .map((tab) => (tab.kind === 'reader' && tab.messageId === messageId ? { ...tab, revealRemote: true } : tab)),
  )
}

/** Keeps a reader tab's copy of its message in step with a change made from
 *  the tab, for when the thread it came from is no longer loaded. */
export function patchTabMessage(id: string, patch: Partial<Message>) {
  compose$.tabs.set(
    compose$.tabs
      .get()
      .map((tab) => (tab.id === id && tab.message ? { ...tab, message: { ...tab.message, ...patch } } : tab)),
  )
}

export function setTabViewMode(id: string, mode: 'html' | 'plain') {
  compose$.tabs.set(compose$.tabs.get().map((tab) => (tab.id === id ? { ...tab, viewMode: mode } : tab)))
}

/** Switches what's in front between HTML and plain text, for the shortcut:
 *  a reader tab flips its own view, a conversation flips its account's
 *  session override, as the header's "View as" menu does. A thread tab's
 *  account comes from the tab. Returns false when there was nothing to switch. */
export function toggleConversationMode(accountId: string | undefined): boolean {
  const activeTabId = compose$.activeTab.peek()
  const tab = activeTabId ? compose$.tabs.peek().find((candidate) => candidate.id === activeTabId) : undefined
  if (tab?.kind === 'reader') {
    // A message with no HTML part reads the same either way.
    if (!tab.bodyHtml) return false
    setTabViewMode(tab.id, tab.viewMode === 'html' ? 'plain' : 'html')
    return true
  }
  if (tab?.kind === 'compose') return false
  const id = tab?.kind === 'thread' && tab.accountId ? tab.accountId : accountId
  const accounts = accounts$.peek()
  if (!id || !accounts.some((account) => account.id === id)) return false
  const current = resolveConversationMode(accounts, thread$.conversationModeOverrides.peek(), id)
  thread$.conversationModeOverrides[id].set(current === 'html' ? 'plain' : 'html')
  return true
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
}): Promise<string> {
  const html = args.rich ? args.content : ''
  const body = args.rich ? htmlToText(args.content) : args.content
  const draftMessageId =
    !args.draftMessageId || args.draftMessageId.startsWith('local-draft-')
      ? await allocateMessageIdentity(args.accountId, true)
      : args.draftMessageId
  const request = invoke('mail.saveDraft', {
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
