import { threadWindowSession } from '../lib/windowSession'
import { standaloneCompose, STANDALONE_COMPOSE_KEY } from '../lib/composeWindow'
import { forgetThreadView } from './threadView'
import { arrayMove } from '@dnd-kit/sortable'
import { t } from '../lib/i18n'
import { isNarrowWindow } from '../lib/useNarrowWindow'
import type { ComposeDraft, Message, MessageTab } from '../types'
import { invoke } from '../lib/bridge'
import { CONVERSATION_PAGE_SIZE } from '../lib/pagination'
import { ui$, showToast, showUndoToast } from './ui'
import { contextualErrorMessage } from '../lib/errors'
import { inlineRichStyles, prepareInlineImages } from '../components/composer/composerHelpers'
import { accounts$, accountIdentities } from './accounts'
import { normalizeMessageId } from './mail'
import { mail$ } from './mailState'
import { isBodyBlankWithoutSignature } from '../lib/signature'
import { closeCurrentConversation, kanban$ } from './kanban'
import { resolveConversationMode, revealRemote, thread$ } from './thread'
import {
  closeComposeSession,
  forgetComposeSession,
  getComposeSession,
  pruneComposeSessions,
  type SavedDraft,
  trackDraftSave,
} from './composeSessions'
import {
  loadPersistedTabs,
  compose$,
  COMPOSE_TABS_KEY,
  newDraftMessageId,
  TAB_VIEW_KEY,
  type PersistedComposeTab,
  type PersistedMailTab,
} from './composeState'
import { discardClosedDraft } from './quickReply'
import { withSignatureForAccount } from './compose'
import { composedContent } from './composeQuote'
import { draftIdOfFailedSave, saveComposedDraft } from './composeSend'

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

// Recover auxiliary editor buffers into the main session after a restart.
export async function restoreThreadWindowDrafts() {
  if (threadWindowSession || standaloneCompose) return
  const keys = Array.from({ length: localStorage.length }, (_, i) => localStorage.key(i)).filter(
    (key): key is string => !!key && /^meron-thread-.+-(tabs|view)$/.test(key),
  )
  let sessions: string[]
  try {
    sessions = await invoke<string[]>('threadWindow.sessions')
  } catch {
    return
  }
  if (!Array.isArray(sessions)) return
  for (const key of keys) {
    const [, session, kind] = key.match(/^meron-thread-(.+)-(tabs|view)$/)!
    if (sessions.includes(session)) continue
    if (kind === 'view') {
      if (localStorage.getItem(key.replace(/-view$/, '-tabs')) === null) localStorage.removeItem(key)
      continue
    }
    const recovered = loadPersistedTabs(key).filter((tab) => tab.compose)
    const current = compose$.tabs.peek()
    const held = new Set(
      current.flatMap((tab) => (tab.compose ? [normalizeMessageId(tab.compose.draftMessageId)] : [])),
    )
    const extra = recovered.filter((tab) => !held.has(normalizeMessageId(tab.compose!.draftMessageId)))
    if (extra.length) compose$.tabs.set([...current, ...extra])
    try {
      const persisted = loadPersistedTabs(COMPOSE_TABS_KEY)
      if (
        recovered.every((tab) =>
          persisted.some(
            (saved) =>
              saved.compose &&
              normalizeMessageId(saved.compose.draftMessageId) === normalizeMessageId(tab.compose!.draftMessageId),
          ),
        )
      ) {
        localStorage.removeItem(key)
        localStorage.removeItem(key.replace(/-tabs$/, '-view'))
      }
    } catch {
      /* Leave the recovery buffer intact if storage is unavailable. */
    }
  }
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
  let tabs = compose$.tabs.get()
  if (!standaloneCompose) {
    try {
      const saved = JSON.parse(localStorage.getItem(STANDALONE_COMPOSE_KEY) || '[]')
      if (Array.isArray(saved)) tabs = [...tabs, ...saved.filter((tab) => tab && typeof tab === 'object')]
    } catch {
      /* Invalid recovery buffer. */
    }
  }
  for (const tab of tabs) {
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
    // Text of encrypted mail never goes to localStorage (plain files on disk):
    // neither text from encrypted mail nor a message being written encrypted.
    // Such a draft is autosaved encrypted to Drafts, so it comes back from
    // there rather than with the tabs.
    if (t.compose.sensitiveSource || t.compose.decryptedInBody || t.compose.encryptedDraft || t.compose.encrypt)
      return []
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
 * error, so nothing typed is lost. A standalone window keeps the editor
 * mounted until saving succeeds, since closing its webview destroys its work.
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
  if (standaloneCompose || threadWindowSession) {
    session.savingToClose = true
    session.setClosingState?.(true)
  }
  session.savesStopped = true
  const drained = session.saveChain
  if (!standaloneCompose && !threadWindowSession) finishClosingMessageTab(tabId)
  const snapshot = current
  const save = drained
    .then(async (): Promise<SavedDraft | null> => {
      // A still-mounted standalone editor adopts an id allocated by the save we
      // just drained. Its final save must replace that copy, not allocate another.
      const current =
        standaloneCompose || threadWindowSession
          ? (compose$.tabs.peek().find((tab) => tab.id === tabId)?.compose ?? snapshot)
          : snapshot
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
          encrypt: current.encrypt,
          decryptedInBody: current.decryptedInBody,
          sign: current.sign,
        })
        if (standaloneCompose || threadWindowSession) finishClosingMessageTab(tabId)
        return { draftMessageId, reopen: () => reopenComposeTab(closed, { ...closing, draftMessageId }) }
      } catch (err) {
        console.error('Saving the draft on close failed:', err)
        // Reopened under the id the failed save was writing, so a copy that
        // landed anyway is replaced by the next save rather than duplicated.
        const draftMessageId = draftIdOfFailedSave(err) ?? current.draftMessageId
        if (standaloneCompose || threadWindowSession) {
          session.savesStopped = false
          updateComposeDraft(tabId, { draftMessageId })
        } else restoreComposeTabAfterFailedSave(closed, { ...closing, draftMessageId })
        showToast(contextualErrorMessage(err, t('composer.status.draftAutosaveFailed')), 'error')
        return null
      }
    })
    .finally(() => {
      if (standaloneCompose || threadWindowSession) {
        session.savingToClose = false
        session.setClosingState?.(false)
      }
    })
  trackDraftSave(save, current)
  if ((standaloneCompose || threadWindowSession) && !session.closing) {
    // Grouped closes call this helper directly. Keep every other exit (tab ×,
    // window close) behind the same final save instead of racing a discard.
    const closing = save
      .then(() => {})
      .finally(() => {
        if (session.closing === closing) session.closing = undefined
      })
    session.closing = closing
  }
  return save
}

/** Says where closed drafts went, with a way to bring them all back. */
export function announceSavedDrafts(saved: SavedDraft[]) {
  if (saved.length === 0) return
  if (threadWindowSession) {
    // Saved drafts resume through their bubble and the main window. A local
    // snapshot action could reopen a draft its quick reply has since claimed.
    showToast(t('composer.status.savedToDrafts'))
    return
  }
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
