import { observable } from '@legendapp/state'
import type { Folder, Message } from '../types'
import { invoke } from '../lib/bridge'
import { t } from '../lib/i18n'
import { clearBulkSelection, confirmAction, ui$, showToast, showUndoToast, type BulkSelectionItem } from './ui'
import { accounts$, unifiedAccounts } from './accounts'
import {
  kanban$,
  forgetDeletedMailViewFolder,
  getKanbanColumns,
  kanbanColumnKey,
  removeKanbanColumnsForFolder,
} from './kanban'
import { columnSearchActive, loadKanbanColumn } from '../lib/kanbanData'
import { filterThreads, isRssAccount } from '../lib/threadActions'
import { isUnifiedStarred, unifiedFolderRole, unifiedFolders } from '../lib/unifiedFolders'
import { isLocalSendId, discardPendingSend } from './pendingSends'
import { CONVERSATION_PAGE_SIZE } from '../lib/pagination'
import { bareAddr, splitAddressList } from '../lib/address'

// Mail data cache — the frontend view of the sidecar's `folders` and `messages`
// tables (threads are messages grouped by the sidecar). Ephemeral: repopulated
// from the sidecar on demand, never persisted on this side.
export const mail$ = observable({
  folders: [] as Folder[],
  // Real per-account folder lists, keyed by account id. `folders` above is the
  // view for the *selected* account (and is just a synthetic single inbox in the
  // unified view), so anything that needs a specific account's real folders —
  // e.g. the thread context menu's "Move to" in the unified inbox — reads here.
  foldersByAccount: {} as Record<string, Folder[]>,
  threads: [] as Message[],
  threadsCursor: '',
  threadAccountCursors: {} as Record<string, string>,
  threadsLoadingMore: false,
  // The view (`threadListViewKey`) whose threads are the ones in `threads`. An
  // empty list means "nothing here" only once this matches the view on screen:
  // before that the rows simply have not arrived, and saying the folder is empty
  // — at startup, or for the second or two a folder load takes — is wrong, then
  // wrong again when they land. Deliberately not a boolean set by `loadThreads`:
  // a selection or filter change repaints the list *before* the effect that
  // starts the load runs, and the client-side filter empties it on that very
  // render, so a flag set inside the load turns on a frame too late.
  threadsLoadedKey: '',
  messages: [] as Message[],
  // Opaque pagination cursor for older messages in the current thread; "" = no more.
  messagesCursor: '',
  // Loading flag for "Load earlier messages".
  messagesLoadingMore: false,
  // True while threadRead is in flight for a newly-selected thread. The reader
  // shows a spinner (instead of the previous thread's stale messages) when this
  // is set and the loaded messages don't yet belong to the active thread —
  // notably during the on-demand ancestor fetch, which adds a network round-trip.
  threadLoading: false,
  // Thread id whose last threadRead failed (backend timeout, network down).
  // The reader shows an error + retry instead of a silent blank pane; cleared
  // on the next load attempt for that thread.
  threadErrorId: '',
  readThreads: {} as Record<string, boolean>,
})

// Optimistic rollback for the keyed caches, key by key. Two reasons not to keep a
// whole `.get()` and restore that: Legend-State mutates a record's raw object in
// place when a *child* node is set (`kanban$.threads[key]`,
// `mail$.foldersByAccount[accountId]`), so the "previous state" keeps changing
// under us; and even a true whole-record snapshot would undo writes that landed
// for *other* keys while the mutation was in flight — a folder LIST for another
// account, another column's page. So each flow captures only the keys it is about
// to touch and puts back exactly those. Plain arrays (`mail$.threads`,
// `mail$.messages`, `mail$.folders`) are always replaced whole, so a bare `.get()`
// is already a snapshot there.
type KeyEntries<T> = [key: string, value: T | undefined][]

function captureKeys<T>(record: Record<string, T>, keys: string[]): KeyEntries<T> {
  return keys.map((key) => [key, record[key]])
}

/** Kanban columns holding a card for this thread: all `updateKanbanThread` and
 * `removeKanbanThread` can touch, so all a rollback has to put back. */
function kanbanKeysWithThread(threadId: string): string[] {
  return Object.entries(kanban$.threads.get())
    .filter(([, threads]) => threads.some((thread) => thread.thread_id === threadId))
    .map(([key]) => key)
}

// Re-read the list whose card stands for this thread after a change inside the
// conversation (a draft discarded, a message deleted) that the card's message
// count and Draft badge reflect. In the mail view that is the thread list. With
// a Kanban board up, `loadThreads` steps out — the list is off screen and
// reloads when the board closes — but the board's columns still show the card,
// and nothing else re-reads them: a reply sent from the board's pane left its
// card counting the draft the send had discarded until some later sync happened
// to touch that column.
//
// `columnKeys` are the columns that held the card, captured before any
// optimistic removal: a conversation whose only loaded message went is dropped
// from the columns ahead of the server round-trip, and looking the card up
// afterwards would find nothing to reload — yet the server may still hold older
// messages of the thread, which only a re-read of those columns brings back.
async function reloadThreadCards(columnKeys: string[]) {
  const boardId = kanban$.activeBoardId.peek()
  if (!boardId) {
    await loadThreads(false)
    return
  }
  const keys = new Set(columnKeys)
  const query = kanban$.searchQuery.peek().trim()
  const scope = kanban$.searchScope.peek()
  await Promise.all(
    getKanbanColumns(boardId)
      .filter((column) => keys.has(kanbanColumnKey(column)))
      .map((column) => {
        const key = kanbanColumnKey(column)
        return loadKanbanColumn(column, false, columnSearchActive(key, query, scope) ? query : '')
      }),
  )
}

function restoreAccountFolders(entries: KeyEntries<Folder[]>) {
  for (const [accountId, folders] of entries) {
    if (folders === undefined) mail$.foldersByAccount[accountId].delete()
    else mail$.foldersByAccount[accountId].set(folders)
  }
}

function restoreKanbanColumns(threads: KeyEntries<Message[]>, unreadCounts: KeyEntries<number> = []) {
  for (const [key, columnThreads] of threads) {
    if (columnThreads === undefined) kanban$.threads[key].delete()
    else kanban$.threads[key].set(columnThreads)
  }
  for (const [key, count] of unreadCounts) {
    if (count === undefined) kanban$.unreadCounts[key].delete()
    else kanban$.unreadCounts[key].set(count)
  }
}

// The one record a flow does restore whole: `loadThreads` owns the entire cursor
// set for the query it is loading, and clears it outright, so there are no other
// keys to preserve.
function snapshotRecord<T>(record: Record<string, T>): Record<string, T> {
  return { ...record }
}

function updateKanbanThread(threadId: string, update: (thread: Message) => Message) {
  const columns = kanban$.threads.get()
  const unreadCounts = kanban$.unreadCounts.get()
  const nextUnreadCounts = { ...unreadCounts }
  let changed = false
  const nextColumns = Object.fromEntries(
    Object.entries(columns).map(([key, threads]) => {
      let columnChanged = false
      const nextThreads = threads.map((thread) => {
        if (thread.thread_id !== threadId) return thread
        columnChanged = true
        const next = update(thread)
        const beforeUnread = thread.unread ? (thread.unread_count ?? 1) : 0
        const afterUnread = next.unread ? (next.unread_count ?? 1) : 0
        if (nextUnreadCounts[key] !== undefined) {
          nextUnreadCounts[key] = Math.max(0, nextUnreadCounts[key] + afterUnread - beforeUnread)
        }
        return next
      })
      if (columnChanged) changed = true
      return [key, columnChanged ? nextThreads : threads]
    }),
  )
  if (changed) {
    kanban$.threads.set(nextColumns)
    kanban$.unreadCounts.set(nextUnreadCounts)
  }
}

function folderMatches(folder: Folder, accountId: string | undefined, folderId: string | undefined): boolean {
  if (!folderId) return false
  const wanted = folderId.toLowerCase()
  const folderIsInbox = folder.role === 'inbox' || folder.id.toLowerCase() === 'inbox'
  const idMatches = wanted === 'inbox' ? folderIsInbox : folder.id === folderId
  return idMatches && (!accountId || folder.account_id === accountId || folder.account_id === 'unified')
}

function decrementFolderUnread(accountId: string | undefined, folderId: string | undefined, count: number) {
  if (count <= 0 || !folderId) return
  const dec = (folder: Folder) =>
    folderMatches(folder, accountId, folderId) ? { ...folder, unread: Math.max(0, folder.unread - count) } : folder

  mail$.folders.set(mail$.folders.get().map(dec))
  if (accountId) {
    const byAccount = mail$.foldersByAccount.get()
    const accountFolders = byAccount[accountId]
    if (accountFolders) {
      mail$.foldersByAccount.set({
        ...byAccount,
        [accountId]: accountFolders.map(dec),
      })
    }
  }
}

function reconcileThreadUnreadFromLoadedMessages(
  threadId: string,
  messages: Message[],
  nextCursor: string | undefined,
) {
  if (nextCursor) return
  const threadMessages = messages.filter((message) => message.thread_id === threadId)
  if (threadMessages.length === 0 || threadMessages.some((message) => message.unread)) return

  const thread = mail$.threads.get().find((item) => item.thread_id === threadId)
  if (!thread?.unread) return
  const unreadCount = Math.max(1, thread.unread_count ?? 0)
  mail$.threads.set(
    mail$.threads
      .get()
      .map((item) => (item.thread_id === threadId ? { ...item, unread: false, unread_count: 0 } : item)),
  )
  decrementFolderUnread(thread.account_id, thread.folder_id, unreadCount)
  updateKanbanThread(threadId, (item) => ({ ...item, unread: false, unread_count: 0 }))
}

function reconcileThreadDraftFromLoadedMessages(threadId: string, messages: Message[]) {
  if (!threadId) return
  const threadMessages = messages.filter((message) => message.thread_id === threadId)
  if (threadMessages.some((message) => isDraftFolder(message.folder_id, message.account_id))) return

  mail$.threads.set(
    mail$.threads.get().map((thread) => (thread.thread_id === threadId ? { ...thread, has_draft: false } : thread)),
  )
  updateKanbanThread(threadId, (thread) => ({ ...thread, has_draft: false }))
}

function removeKanbanThread(threadId: string) {
  const columns = kanban$.threads.get()
  let changed = false
  const nextColumns = Object.fromEntries(
    Object.entries(columns).map(([key, threads]) => {
      const nextThreads = threads.filter((thread) => thread.thread_id !== threadId)
      if (nextThreads.length !== threads.length) changed = true
      return [key, nextThreads]
    }),
  )
  if (changed) kanban$.threads.set(nextColumns)
}

// Thread ids encode the source folder, so a surviving row with the same id
// means the message copies are still in that folder — the server didn't
// actually apply the change, even though the call reported success. Checked
// after the post-action refresh so the toast reflects what the list shows.
function threadStillListed(threadId: string): boolean {
  return mail$.threads.get().some((item) => item.thread_id === threadId)
}

function assertDeleteAffected(res: unknown) {
  if (!res || typeof res !== 'object') return
  const deleted = (res as { deleted?: unknown }).deleted
  if (typeof deleted === 'number' && deleted <= 0) {
    throw new Error('Delete failed: no matching messages found')
  }
}

function assertMoveAffected(res: unknown, label = 'Move') {
  if (!res || typeof res !== 'object') return
  const moved = (res as { moved?: unknown }).moved
  if (typeof moved === 'number' && moved <= 0) {
    throw new Error(`${label} failed: no matching messages found`)
  }
}

function assertCopyAffected(res: unknown) {
  if (!res || typeof res !== 'object') return
  const copied = (res as { copied?: unknown }).copied
  if (typeof copied === 'number' && copied <= 0) {
    throw new Error('Copy failed: no matching messages found')
  }
}

function threadIdInFolder(threadId: string, accountId: string | undefined, folderId: string | undefined): string {
  const lastHash = threadId.lastIndexOf('#')
  if (!accountId || !folderId || lastHash <= 0) return threadId
  return `${accountId}#${folderId}#${threadId.slice(lastHash + 1)}`
}

function looksLikeTrashName(value: string): boolean {
  return ['trash', 'bin', 'deleted items', 'deleted messages', '[gmail]/trash'].includes(value.trim().toLowerCase())
}

export function isTrashFolder(folder?: Pick<Folder, 'id' | 'name' | 'role'> | null): boolean {
  if (!folder) return false
  return folder.role === 'trash' || looksLikeTrashName(folder.id) || looksLikeTrashName(folder.name)
}

export function isTrashFolderId(accountId: string, folderId: string): boolean {
  const accountFolders = mail$.foldersByAccount[accountId].get() ?? []
  const selectedFolders = mail$.folders.get()
  const folder = [...accountFolders, ...selectedFolders].find(
    (item) => item.account_id === accountId && item.id === folderId,
  )
  return isTrashFolder(folder) || looksLikeTrashName(folderId)
}

function looksLikeJunkName(value: string): boolean {
  return ['junk', 'spam', 'junk e-mail', 'junk email', 'bulk mail', '[gmail]/spam'].includes(value.trim().toLowerCase())
}

// Trash and Junk are the only folders that may be emptied: the delete is
// permanent, so anywhere else it would be an unrecoverable mis-tap. Takes the
// folder the caller already has in hand (so the lookup stays inside the
// caller's reactive folder list) and returns the role plus display name for the
// menu label and confirm wording, or null when the folder is not emptiable.
export function emptiableFolder(folder?: Folder | null): { role: 'trash' | 'junk'; name: string } | null {
  if (!folder) return null
  const name = folder.name || folder.id
  if (folder.role === 'junk' || looksLikeJunkName(folder.id) || looksLikeJunkName(folder.name)) {
    return { role: 'junk', name }
  }
  if (isTrashFolder(folder)) return { role: 'trash', name }
  return null
}

// Permanently delete every message in a Trash or Junk folder, server side and in
// the store. Confirms first — there is no Trash left to restore from. `target`
// comes from `emptiableFolder`; the sidecar re-checks the folder role anyway.
// Returns true when the folder was emptied, so the caller can refresh its view.
export async function emptyFolder(
  accountId: string,
  folderId: string,
  target: { role: 'trash' | 'junk'; name: string },
): Promise<boolean> {
  if (!accountId || !folderId || accountId === 'unified') return false

  if (
    !(await confirmAction({
      title: t('threads.emptyFolder.confirmTitle', { folder: target.name }),
      message: t('threads.emptyFolder.confirmMessage', { folder: target.name }),
      confirmLabel: t('threads.emptyFolder.confirmButton'),
      tone: 'danger',
    }))
  ) {
    return false
  }

  try {
    const res = await invoke<MutationResult>('mail.emptyFolder', { account_id: accountId, folder_id: folderId })
    applyMutationFolderUnreads(res)
    // The open conversation may have just been deleted along with the folder.
    const openThread = findLocalThread(ui$.selectedThread.get())
    if (openThread?.account_id === accountId && openThread?.folder_id === folderId) ui$.selectedThread.set('')
    void loadFolders(accountId, false)
    showToast(t('threads.emptyFolder.done', { folder: target.name }))
    return true
  } catch (error) {
    showToast(
      error instanceof Error ? error.message : t('threads.emptyFolder.failed', { folder: target.name }),
      'error',
    )
    return false
  }
}

// Only an ordinary folder can be deleted on the server: special-use mailboxes
// carry the app's own routing (Inbox/Sent/Drafts/Trash/Junk/Archive), and that
// covers anything nested under the folder too, because deleting it takes the
// whole subtree along. Takes the account's folder list so the nesting check
// stays inside the caller's reactive list, and returns the display name for the
// menu label plus the number of subfolders that would go with it, or null when
// not deletable.
export function deletableFolder(
  folder: Folder | undefined | null,
  folders: Folder[],
): { name: string; nested: number } | null {
  if (!folder || !folder.id || folder.account_id === 'unified') return null
  if (folder.role && folder.role !== 'folder') return null
  const prefix = `${folder.id}${folder.delimiter || '/'}`
  const nested = folders.filter((item) => item.id !== folder.id && item.id.startsWith(prefix))
  if (nested.some((item) => item.role && item.role !== 'folder')) return null
  return { name: folder.name || folder.id, nested: nested.length }
}

// Delete a folder on the server, along with its subfolders, their cached
// messages and any board column that showed one of them. Confirms first — the
// mail goes with them and the server keeps no copy. Core re-checks that the
// subtree is deletable. Returns true when the folder is gone, so the caller can
// move its view elsewhere.
export async function deleteFolder(accountId: string, folderId: string, name?: string, nested = 0): Promise<boolean> {
  if (!accountId || !folderId || accountId === 'unified') return false
  const label = name || folderId

  if (
    !(await confirmAction({
      title: t('folders.delete.confirmTitle', { folder: label }),
      message: nested
        ? t('folders.delete.confirmMessageNested', { folder: label, count: nested })
        : t('folders.delete.confirmMessage', { folder: label }),
      confirmLabel: t('folders.delete.confirmButton'),
      tone: 'danger',
    }))
  ) {
    return false
  }

  try {
    const res = await invoke<{ folders?: Folder[]; removed?: string[]; warning?: string }>('mail.folderDelete', {
      account_id: accountId,
      folder_id: folderId,
    })
    // Core reports the whole subtree it took down; fall back to the folder
    // itself if an older core answers without the list.
    const removed = res?.removed?.length ? res.removed : [folderId]
    const folders = res?.folders
    if (folders) {
      mail$.foldersByAccount[accountId].set(folders)
      if (ui$.selectedAccount.get() === accountId) mail$.folders.set(folders)
    } else {
      void loadFolders(accountId, false)
    }
    // Capture this before removing the Kanban column, which also drops the
    // column's thread cache and would make the selected conversation unfindable.
    const openThread = findLocalThread(ui$.selectedThread.get())
    for (const gone of removed) removeKanbanColumnsForFolder(accountId, gone)
    // The open conversation and the mailbox view may have been inside any of
    // the deleted folders, not just the one the action targeted.
    if (openThread?.account_id === accountId && removed.includes(openThread?.folder_id ?? '')) {
      ui$.selectedThread.set('')
    }
    if (ui$.selectedAccount.get() === accountId) {
      if (removed.includes(ui$.selectedFolder.get())) ui$.selectedFolder.set('inbox')
      // While a board is open the visible folder is the open card's, not the
      // mail view's — that one is stashed for the board's close, so it needs
      // the same check.
      forgetDeletedMailViewFolder(removed)
    }
    if (res?.warning) {
      showToast(res.warning, 'error')
      return false
    }
    showToast(t('folders.delete.done', { folder: label }))
    return true
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('folders.delete.failed', { folder: label }), 'error')
    return false
  }
}

export function findLocalThread(threadId: string): Message | undefined {
  const thread = mail$.threads.get().find((item) => item.thread_id === threadId)
  if (thread) return thread
  for (const threads of Object.values(kanban$.threads.get())) {
    const match = threads.find((item) => item.thread_id === threadId)
    if (match) return match
  }
  return mail$.messages.get().find((item) => item.thread_id === threadId)
}

// After a read/unread toggle, refresh the cached folder unread counts that feed
// the side navigation badges. Refreshing the selected view (`loadFolders`) keeps the
// folder list and the unified/account badge for that view fresh — but it
// only reloads `selectedAccount`'s folders. In a cross-account view (unified,
// Starred) or an open Kanban board, `selectedAccount` is 'unified' or some
// unrelated account, so
// the *thread's own* account never gets reloaded and its side navigation unread badge —
// plus the unified total it sums into — drifts out of sync. Refresh that
// account too. Both calls are cache-only (refresh:false), so no IMAP traffic.
function refreshFoldersAfterFlagChange(accountId: string | undefined) {
  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) void loadFolders(selectedAcc, false)
  if (accountId && accountId !== selectedAcc) {
    void refreshAccountFoldersCache(accountId, false)
  }
}

// Click-to-open an attachment: the bridge hands it to the OS default
// application. Types off its allowlist (executables, macro-capable documents,
// archives — anything a default handler could turn into code execution) come
// back `opened: false` and fall through to the save dialog, which stays the
// behaviour for everything we won't open.
export async function openAttachment(att: { key: string | null; filename: string }) {
  if (!att.key) return
  try {
    const res = await invoke<{ opened: boolean; path?: string }>('mail.openAttachment', {
      key: att.key,
      filename: att.filename,
    })
    if (!res?.opened) await downloadAttachment(att)
  } catch {
    showToast(t('chat.couldNotOpenAttachment', { filename: att.filename }))
  }
}

// Save a local attachment to disk via the native save dialog. The bytes already
// live in the media cache (keyed); the bridge copies them to the chosen path.
export async function downloadAttachment(att: { key: string | null; filename: string }) {
  if (!att.key) return
  try {
    const res = await invoke<{ saved: boolean; path?: string }>('mail.saveAttachment', {
      key: att.key,
      filename: att.filename,
    })
    if (res?.saved) showToast(t('chat.savedAttachment', { filename: att.filename }))
  } catch {
    showToast(t('chat.couldNotSaveAttachment', { filename: att.filename }))
  }
}

// Save one message as a .eml file (its original RFC822 bytes) via the native
// save dialog. Unlike attachments, the raw bytes aren't in the media cache, so
// the bridge refetches them over IMAP — this fails offline, and a message still
// being sent has no server-side copy to fetch.
export async function saveMessageAsEml(message: Message) {
  if (!message?.id || isLocalSendId(message.id)) return
  try {
    const res = await invoke<{ saved: boolean; path?: string }>('mail.saveEml', {
      thread_id: message.thread_id,
      message_ids: [message.id],
      folder: message.folder_id,
      subject: message.subject,
    })
    if (res?.saved) showToast(t('chat.messageSaved'))
  } catch {
    showToast(t('chat.couldNotSaveMessage'))
  }
}

// Copy a keyed image onto the system clipboard. The webview's native "Copy
// Image" is inert in the Wails webview, so the bridge shells out to the same
// clipboard helpers used for pasting.
export async function copyAttachmentImage(att: { key: string | null }) {
  if (!att.key) return
  try {
    await invoke('mail.copyImage', { key: att.key })
    showToast(t('chat.imageCopied'))
  } catch {
    showToast(t('chat.couldNotCopyImage'))
  }
}

// The visible thread list after applying the active filter (all / unread / starred).
export function getFilteredThreads() {
  const threads = mail$.threads.get()
  // The starred folder already lists starred threads only; a leftover filter
  // mode from the previous mailbox must not hide rows here.
  if (isUnifiedStarred(ui$.selectedAccount.get(), ui$.selectedFolder.get())) return threads
  const filterMode = ui$.filterMode.get()
  const selected = ui$.selectedThread.get()
  return filterThreads(threads, filterMode, selected, mail$.readThreads.get())
}

// Move the selection up (delta -1) or down (delta +1) through the visible
// thread list, clamping at the ends (no wrap, so a held key doesn't loop back).
// Backs the j/k keyboard navigation.
export function selectAdjacentThread(delta: number) {
  const list = getFilteredThreads()
  if (list.length === 0) return
  const selected = ui$.selectedThread.get()
  const current = list.findIndex((thread) => thread.thread_id === selected)
  const next = current === -1 ? 0 : Math.min(list.length - 1, Math.max(0, current + delta))
  const target = list[next]
  if (target) ui$.selectedThread.set(target.thread_id)
}

export function getActiveThread() {
  const filtered = getFilteredThreads()
  const threads = mail$.threads.get()
  const selected = ui$.selectedThread.get()
  // Nothing selected means an empty conversation pane. Falling back to the top
  // of the list here would re-open — and so mark read — a thread the user never
  // picked, which is exactly what `loadThreads` refuses to do.
  if (!selected) return null
  const fromList = filtered.find((thread) => thread.thread_id === selected)
  if (fromList) return fromList
  const fromAllThreads = threads.find((thread) => thread.thread_id === selected)
  if (fromAllThreads) return fromAllThreads
  const kanbanColumns = kanban$.threads.get()
  for (const threads of Object.values(kanbanColumns)) {
    const match = threads.find((thread) => thread.thread_id === selected)
    if (match) return match
  }
  // The selected thread's row hasn't landed yet (a notification or kanban jump
  // that outran the list load). Wait for it rather than showing an unrelated
  // conversation from the list we happen to have.
  return null
}

export async function loadFolders(accountId: string, refresh = true) {
  if (accountId === 'unified') {
    const accounts = unifiedAccounts()
    // Publish the synthetic list before the per-account counts land: it never
    // depends on them, and the folder switcher reads it from the per-account
    // cache — leaving that empty until the fan-out resolves shows the picker's
    // "no folders" state on every cold open.
    mail$.folders.set(unifiedFolders(t))
    mail$.foldersByAccount['unified'].set(unifiedFolders(t))
    let totalUnread = 0
    try {
      const foldersList = await Promise.all(
        accounts.map(async (acc) => {
          try {
            const res = await invoke<{ folders: Folder[] }>('mail.folderList', {
              account_id: acc.id,
              // Propagate the caller's refresh so sub-accounts get a real folder
              // LIST sync in the unified view. Without it a freshly added account
              // only ever has its synthetic INBOX row, so the folder picker and
              // "Move to" lists show just Inbox. The sync is async + deduped; the
              // mail.synced({folders:true}) it emits triggers a refresh:false reload.
              refresh,
            })
            const folders = res.folders || []
            mail$.foldersByAccount[acc.id].set(folders)
            return folders
          } catch {
            return []
          }
        }),
      )
      for (const folders of foldersList) {
        const inboxFolder = folders.find((f) => f.role === 'inbox' || f.id.toLowerCase() === 'inbox')
        if (inboxFolder) {
          totalUnread += inboxFolder.unread || 0
        }
      }
    } catch (err) {
      console.error('Failed to load folders list for unified count:', err)
    }

    const folders = unifiedFolders(t, totalUnread)
    mail$.folders.set(folders)
    mail$.foldersByAccount['unified'].set(folders)
    return
  }

  const result = await invoke<{ folders: Folder[] }>('mail.folderList', { account_id: accountId, refresh })
  mail$.folders.set(result.folders)
  mail$.foldersByAccount[accountId].set(result.folders)
}

export async function refreshAccountFoldersCache(accountId: string, refresh = false): Promise<Folder[]> {
  if (!accountId || accountId === 'unified') return []
  try {
    const result = await invoke<{ folders: Folder[] }>('mail.folderList', { account_id: accountId, refresh })
    const folders = result.folders || []
    mail$.foldersByAccount[accountId].set(folders)
    return folders
  } catch (error) {
    console.error('refreshAccountFoldersCache failed:', error)
    return []
  }
}

/** Unread count of the INBOX folder in a folder list, or 0 if absent. */
export function inboxUnread(folders: Folder[] | undefined): number {
  return folderUnread(folders, 'inbox')
}

/** Unread count of a folder in a folder list, treating INBOX case-insensitively. */
export function folderUnread(folders: Folder[] | undefined, folderId: string): number {
  if (!folders) return 0
  const wanted = folderId.toLowerCase()
  const folder = folders.find((f) => {
    if (wanted === 'inbox') return f.role === 'inbox' || f.id.toLowerCase() === 'inbox'
    return f.id === folderId
  })
  return folder?.unread ?? 0
}

// Apply an unread total returned with a thread page to the same per-account
// folder cache used by side-navigation badges. This keeps a freshly loaded
// mailbox/Kanban column and the navigation chrome on one core-owned value.
export function updateCachedFolderUnread(accountId: string, folderId: string, unread: number) {
  if (!accountId || accountId === 'unified' || !folderId || !Number.isFinite(unread)) return
  const count = Math.max(0, Math.floor(unread))
  const patch = (folders: Folder[] | undefined): Folder[] => {
    const current = folders ?? []
    let matched = false
    const next = current.map((folder) => {
      if (!folderMatches(folder, accountId, folderId)) return folder
      matched = true
      return folder.unread === count ? folder : { ...folder, unread: count }
    })
    if (matched || folderId.toLowerCase() !== 'inbox') return next
    return [...next, { id: folderId, account_id: accountId, name: 'Inbox', role: 'inbox', unread: count }]
  }

  const byAccount = mail$.foldersByAccount.get()
  const nextAccountFolders = patch(byAccount[accountId])
  const nextByAccount = { ...byAccount, [accountId]: nextAccountFolders }
  mail$.foldersByAccount.set(nextByAccount)

  const selected = ui$.selectedAccount.get()
  if (selected === accountId) {
    mail$.folders.set(patch(mail$.folders.get()))
  } else if (selected === 'unified') {
    const total = unifiedAccounts().reduce((sum, account) => sum + inboxUnread(nextByAccount[account.id]), 0)
    mail$.folders.set(unifiedFolders(t, total))
  }
}

type MutationResult = { folder_unreads?: Record<string, Record<string, number>> }

function applyMutationFolderUnreads(result: MutationResult | undefined) {
  for (const [accountId, folders] of Object.entries(result?.folder_unreads ?? {})) {
    for (const [folderId, unread] of Object.entries(folders)) {
      updateCachedFolderUnread(accountId, folderId, unread)
    }
  }
}

function hasOnlyBootstrapInbox(folders: Folder[]) {
  if (folders.length !== 1) return false
  const folder = folders[0]
  return folder.role === 'inbox' || folder.id.toLowerCase() === 'inbox' || folder.name.toLowerCase() === 'inbox'
}

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}

// Folders for a specific account, fetching+caching them if not already loaded.
// Used by per-thread actions (e.g. the context menu's "Move to") that may run in
// the unified view, where `mail$.folders` only holds the synthetic unified inbox.
export async function ensureAccountFolders(
  accountId: string,
  options: { refreshIfBootstrapOnly?: boolean; waitForRefresh?: boolean; forceRefresh?: boolean } = {},
): Promise<Folder[]> {
  if (!accountId || accountId === 'unified') return []
  const cached = mail$.foldersByAccount[accountId].get()
  if (cached && cached.length > 0) {
    // A non-empty cache is not proof it is current: folders created on the server
    // (webmail, another client) only reach us through a real LIST sync. Callers
    // that show a folder picker pass forceRefresh so the list self-heals; the
    // result lands asynchronously via mail.synced({folders:true}).
    if (options.forceRefresh || (options.refreshIfBootstrapOnly && hasOnlyBootstrapInbox(cached))) {
      void refreshAccountFoldersCache(accountId, true)
    }
    return cached
  }
  try {
    const result = await invoke<{ folders: Folder[] }>('mail.folderList', {
      account_id: accountId,
      refresh: options.refreshIfBootstrapOnly || options.forceRefresh,
    })
    const folders = result.folders || []
    mail$.foldersByAccount[accountId].set(folders)
    if (options.waitForRefresh && (options.refreshIfBootstrapOnly || options.forceRefresh) && folders.length === 0) {
      for (let attempt = 0; attempt < 10; attempt += 1) {
        await sleep(500)
        const refreshed = await refreshAccountFoldersCache(accountId, false)
        if (refreshed.length > 0) return refreshed
      }
    }
    return folders
  } catch {
    return []
  }
}

// Guards against a slow load repainting the list after a newer one already did.
// A search is a live IMAP round trip — seconds, not milliseconds — so typing
// another character, clearing the box, or switching account/folder while one is
// in flight routinely finishes out of order. Every write below belongs to the
// newest call only; the losers drop their results (same idea as the kanban
// column loader's `columnLoadVersions` and mobile's `activeMailboxLoadToken`).
let threadLoadVersion = 0

// Opening a conversation is never passive: the message pane marks every visible
// unread message read as soon as it renders. So the thread list must not open a
// conversation on its own — switching account/folder (the unified inbox above
// all, since it lands on the newest mail across every account) would silently
// clear the unread flag on a thread the user never looked at. Selection follows
// a click, a j/k move, or the flows below that delete what was open and hand the
// slot to the replacement thread; every other load leaves it alone.
let reselectAfterThreadLoad = false

// View key of the refresh:true load currently out to the server, '' when none,
// tagged with the `threadLoadVersion` of the load holding it so only that load
// releases it. Read by the background-refresh guard below.
let pendingRefreshKey = ''
let pendingRefreshVersion = 0
// View key of a background refresh that stepped aside for the load above, ''
// when none. The load it yielded to read the cache before whatever prompted
// the refresh changed it — a post-send draft discard, say — so its rows are
// not the answer the refresh was after; it runs again once that load lands.
let deferredRefreshKey = ''

// Called by flows that clear `selectedThread` because the open conversation is
// leaving the list (delete, move, discard draft) and want the next load to open
// whatever takes its place. One-shot: consumed by the next `loadThreads`.
export function requestThreadReselect() {
  reselectAfterThreadLoad = true
}

type ThreadSearchStage = 'auto' | 'cache' | 'live'

// Identity of a thread-list view: the four inputs `loadThreads` reads and
// `superseded` watches, newline-joined like `kanbanColumnKey` (neither a folder
// id nor a single-line search box carries one). Both the loader and the list
// build the key from the same fields, so the list can tell "these rows are for
// what I'm showing" from "these rows are the previous view's".
export function threadListViewKey(account: string, folder: string, query: string, filter: string) {
  return [account, folder, query, filter].join('\n')
}

export async function loadThreads(refresh = true, searchStage: ThreadSearchStage = 'auto') {
  // A Kanban card temporarily points selectedFolder at the card's real mailbox
  // so thread actions have the right context. The normal mail list is hidden,
  // and treating that account-specific id as a unified role falls back to Inbox,
  // replacing the rows that should still be waiting for the mail view.
  if (kanban$.activeBoardId.peek()) {
    // A reselect request belongs to the load that was asked for, which is this
    // one; dropping it here keeps it from arming a later load in the mail view,
    // where it would open (and mark read) an unrelated conversation.
    reselectAfterThreadLoad = false
    return
  }

  const initialAccount = ui$.selectedAccount.get()
  const initialFolder = ui$.selectedFolder.get()
  const initialQuery = ui$.query.get()
  const initialFilter = ui$.filterMode.get()
  const activeAccount = accounts$.get().find((account) => account.id === initialAccount)
  // Starred is answered from the local cache, so there is no live stage to run.
  const canSearchLive =
    !isUnifiedStarred(initialAccount, initialFolder) &&
    (initialAccount === 'unified' || !isRssAccount(activeAccount, initialAccount))

  // Paint results from the local FTS index before starting the live IMAP
  // request. RSS search is already local, and background refreshes deliberately
  // remain cache-only.
  if (refresh && searchStage === 'auto' && initialQuery.trim() && canSearchLive) {
    await loadThreads(false, 'cache')
    if (
      ui$.selectedAccount.get() !== initialAccount ||
      ui$.selectedFolder.get() !== initialFolder ||
      ui$.query.get() !== initialQuery ||
      ui$.filterMode.get() !== initialFilter
    ) {
      return
    }
    await loadThreads(true, 'live')
    return
  }

  const selectedAcc = ui$.selectedAccount.get()
  const selectedFol = ui$.selectedFolder.get()
  const q = ui$.query.get()
  const filter = ui$.filterMode.get()
  const viewKey = threadListViewKey(selectedAcc, selectedFol, q, filter)

  // A background refresh steps aside for a server-bound load already running for
  // the same view. Taking the version from it would throw away the fresher rows
  // it is about to return, and — since a background load never settles a view —
  // would strand the list on the spinner: the foreground load loses `superseded`
  // when it lands, and nothing else is scheduled to try again. Nothing is lost by
  // skipping; the load in flight is asking the server the same question.
  // Only while the claiming load is still the newest one. A load another has
  // overtaken will drop its results, and it releases the claim on its own only
  // when its request finally lands — a slow or wedged search would keep every
  // background refresh of the view out until then.
  if (
    !refresh &&
    searchStage !== 'cache' &&
    pendingRefreshKey === viewKey &&
    pendingRefreshVersion === threadLoadVersion
  ) {
    deferredRefreshKey = viewKey
    return
  }

  const version = (threadLoadVersion += 1)
  if (refresh) {
    pendingRefreshKey = viewKey
    pendingRefreshVersion = version
    // This load reads the cache after anything a skipped refresh was reacting to.
    deferredRefreshKey = ''
  }
  // Hand the claim above back the moment this load stops being the one that will
  // write. Every background refresh of this view steps aside while it stands, so
  // a claim left behind by a load that gave up silences them all for as long as
  // the view is on screen — that is how a reply sent from a search left the
  // thread's card showing a Draft badge and a message count that still counted
  // the draft: the refresh the post-send discard runs was skipped for a search
  // load the next keystroke had already superseded.
  const releasePendingRefresh = () => {
    if (pendingRefreshVersion !== version) return
    pendingRefreshKey = ''
    pendingRefreshVersion = 0
  }
  // Asked at every point this load would drop its results, so it is also where
  // the claim is released.
  const superseded = () => {
    const stale =
      threadLoadVersion !== version ||
      ui$.selectedAccount.get() !== selectedAcc ||
      ui$.selectedFolder.get() !== selectedFol ||
      ui$.query.get() !== q ||
      ui$.filterMode.get() !== filter
    if (stale) releasePendingRefresh()
    return stale
  }
  const previousThreads = mail$.threads.get()
  const currentSelected = ui$.selectedThread.get()
  const previousThreadsCursor = mail$.threadsCursor.get()
  const previousAccountCursors = snapshotRecord(mail$.threadAccountCursors.get())
  const userInitiated = refresh || searchStage === 'cache'

  // A background refresh (a sync, not a user-initiated account/folder/query/filter
  // change) only re-fetches the first page of the thread list. If the user has
  // scrolled the list and loaded extra pages, replacing the whole array with just
  // the first page collapses it and resets the scroll position. In that case we
  // merge the fresh page into the list we already have instead.
  const mergeBackground = !refresh && searchStage !== 'cache' && previousThreads.length > 0

  let allThreads: Message[] = []

  // Starred spans every account and every folder, so it is answered by a
  // cross-account cache query rather than the per-account folder fan-out. Its
  // rows are ordinary thread cards, so only the fetch is special-cased.
  if (isUnifiedStarred(selectedAcc, selectedFol)) {
    try {
      const res = await invoke<{ items: Message[]; next_cursor?: string }>('mail.starredItems', {
        query: q,
        filter,
        limit: 50,
      })
      if (superseded()) return
      allThreads = res.items ?? []
      mail$.threadsCursor.set(res.next_cursor ?? '')
      mail$.threadAccountCursors.set({})
    } catch (err) {
      if (superseded()) return
      console.error('Failed to load starred items:', err)
      mail$.threadsCursor.set('')
      mail$.threadAccountCursors.set({})
    }
  } else if (selectedAcc === 'unified') {
    const role = unifiedFolderRole(selectedFol)
    try {
      const result = await invoke<{
        threads: Message[]
        next_cursor?: string
        folder_unreads?: Record<string, number>
        failures?: Array<{ account_id: string; message: string }>
      }>('mail.threadList', {
        account_id: 'unified',
        folder_id: role,
        folder_role: role,
        query: q,
        filter,
        refresh,
      })
      if (superseded()) return
      allThreads = result.threads || []
      // Only the Inbox totals feed the side-nav badges; the other unified
      // folders have no badge to keep in sync.
      if (role === 'inbox') {
        for (const [accountId, unread] of Object.entries(result.folder_unreads ?? {})) {
          updateCachedFolderUnread(accountId, 'inbox', unread)
        }
      }
      for (const failure of result.failures ?? []) {
        console.error(`Failed to load threads for ${failure.account_id}: ${failure.message}`)
      }
      mail$.threadAccountCursors.set({})
      mail$.threadsCursor.set(result.next_cursor ?? '')
    } catch (err) {
      if (superseded()) return
      console.error('Failed to load unified threads:', err)
      mail$.threadAccountCursors.set({})
      mail$.threadsCursor.set('')
    }
  } else {
    try {
      const result = await invoke<{ threads: Message[]; next_cursor?: string; folder_unread?: number }>(
        'mail.threadList',
        {
          account_id: selectedAcc,
          folder_id: selectedFol,
          query: q,
          filter,
          refresh,
        },
      )
      if (superseded()) return
      if (typeof result.folder_unread === 'number') {
        updateCachedFolderUnread(selectedAcc, selectedFol, result.folder_unread)
      }
      allThreads = result.threads || []
      mail$.threadsCursor.set(result.next_cursor ?? '')
      mail$.threadAccountCursors.set({})
    } catch (err) {
      if (superseded()) return
      console.error('Failed to load threads:', err)
      mail$.threadsCursor.set('')
      mail$.threadAccountCursors.set({})
    }
  }

  if (filter !== 'all' && currentSelected && !allThreads.some((thread) => thread.thread_id === currentSelected)) {
    const selectedThread = previousThreads.find((thread) => thread.thread_id === currentSelected)
    if (selectedThread) {
      allThreads = [...allThreads, selectedThread]
      allThreads.sort((a, b) => b.date - a.date)
    }
  }

  if (mergeBackground) {
    // Update the threads we already show with their fresh copies (new unread
    // counts, latest message, etc.), keep the extra pages the user loaded by
    // scrolling, and prepend any threads that are brand-new since the last load.
    // This preserves both the list length and the user's scroll position.
    const fetched = new Map(allThreads.map((thread) => [thread.thread_id, thread]))
    const previousIds = new Set(previousThreads.map((thread) => thread.thread_id))
    const brandNew = allThreads.filter((thread) => !previousIds.has(thread.thread_id))
    const updated = previousThreads.map((thread) => fetched.get(thread.thread_id) ?? thread)
    allThreads = brandNew.length > 0 ? [...brandNew, ...updated] : updated
    // Keep the cursor pointing past the last loaded page rather than resetting it
    // to the first page's cursor.
    mail$.threadsCursor.set(previousThreadsCursor)
    mail$.threadAccountCursors.set(previousAccountCursors)
  }

  mail$.threads.set(allThreads)
  // These rows now stand for this view — but only a load that went to the server
  // for them may say so, which is what `refresh` marks. The two cache-only kinds
  // both answer from the local index, so an empty result from either means "not
  // found *yet*", and settling the view on one puts "No matching mail" on screen
  // while the real answer is still coming:
  //   - the cache stage of a search, whose live IMAP half is the slow one, and
  //     the one that finds mail the index has not got;
  //   - a background refresh (sync event, feed edit), which fires on its own
  //     schedule and so can land in the gap between a keystroke and the
  //     debounced search, or between a folder switch and its own load.
  // Every view reaches the screen through an effect that loads it with
  // refresh:true, including the ones answered locally (Starred, RSS), so none
  // depends on a cache-only load to settle.
  // Only a load that got past `superseded` reaches here, so the captured fields
  // are still the ones on screen.
  if (refresh) {
    mail$.threadsLoadedKey.set(viewKey)
    releasePendingRefresh()
    // A background refresh that stepped aside for this load was asking about a
    // cache this load had already read — the draft a post-send discard removed
    // is still in the rows just written, so its card keeps the Draft badge and
    // counts the draft. Run the refresh now that nothing is in its way.
    if (deferredRefreshKey === viewKey) {
      deferredRefreshKey = ''
      void loadThreads(false).catch(console.error)
    }
  }

  const filtered = getFilteredThreads()
  // Consume the reselect request here rather than at the top of the load: a
  // superseded or replaced load returns above without ever reaching the
  // selection, and the request belongs to whichever load actually lands.
  const reselect = reselectAfterThreadLoad
  reselectAfterThreadLoad = false
  // In kanban view the open conversation is owned by kanban$.paneThreadId, not by
  // mail$.threads/filtered. Clicking a card sets selectedFolder (firing this load)
  // and selectedThread together; auto-selecting or snapping here would yank
  // selectedThread to an unrelated normal-view thread while the pane stays open on
  // the card — rendering the wrong conversation. So leave the selection alone.
  if (kanban$.activeBoardId.get()) {
    return
  }
  if (!currentSelected) {
    // Only a flow that just cleared the selection may fill it (see
    // `requestThreadReselect`); an empty pane otherwise stays empty.
    if (reselect) ui$.selectedThread.set(filtered[0]?.thread_id ?? '')
  } else if (
    // Only drop the selection on a user-initiated load (account/folder/query/
    // filter change, including the cache stage of a search). A background
    // refresh only re-fetches the first page, so an open thread the user
    // scrolled down to and opened from a later page would look "missing" and
    // get closed a second or two later.
    userInitiated &&
    !allThreads.some((thread) => thread.thread_id === currentSelected) &&
    // A selection whose conversation is still being fetched — a notification or
    // starred jump that set account, folder and thread together — isn't missing,
    // it just hasn't landed. Only close one that has settled.
    !mail$.threadLoading.get()
  ) {
    // The thread the user was reading is not in this view: close the pane rather
    // than opening an unrelated one for them.
    ui$.selectedThread.set('')
  }
}

export async function loadMoreThreads() {
  if (mail$.threadsLoadingMore.get()) return
  const selectedAcc = ui$.selectedAccount.get()
  const selectedFol = ui$.selectedFolder.get()
  const q = ui$.query.get()
  const filter = ui$.filterMode.get()
  const version = threadLoadVersion
  const stillCurrent = (cursor: string) =>
    threadLoadVersion === version &&
    ui$.selectedAccount.get() === selectedAcc &&
    ui$.selectedFolder.get() === selectedFol &&
    ui$.query.get() === q &&
    ui$.filterMode.get() === filter &&
    mail$.threadsCursor.get() === cursor
  // The starred filter is one unpaginated page; a search over it is paged like
  // any other, and every other view stops on an empty cursor below.
  if (!q.trim() && filter === 'starred') return

  mail$.threadsLoadingMore.set(true)
  try {
    let moreThreads: Message[] = []
    if (isUnifiedStarred(selectedAcc, selectedFol)) {
      const cursor = mail$.threadsCursor.get()
      if (!cursor) return
      const res = await invoke<{ items: Message[]; next_cursor?: string }>('mail.starredItems', {
        // The cursor walks the *filtered* set, so later pages must repeat the
        // query or they'd page through items the first page never showed.
        query: q,
        filter,
        limit: 50,
        before_cursor: cursor,
      })
      if (!stillCurrent(cursor)) return
      moreThreads = res.items || []
      mail$.threadsCursor.set(res.next_cursor ?? '')
    } else if (selectedAcc === 'unified') {
      const cursor = mail$.threadsCursor.get()
      if (!cursor) return
      const role = unifiedFolderRole(selectedFol)
      const res = await invoke<{
        threads: Message[]
        next_cursor?: string
        folder_unreads?: Record<string, number>
      }>('mail.threadList', {
        account_id: 'unified',
        folder_id: role,
        folder_role: role,
        query: q,
        filter,
        before_cursor: cursor,
        refresh: false,
      })
      if (!stillCurrent(cursor)) return
      if (role === 'inbox') {
        for (const [accountId, unread] of Object.entries(res.folder_unreads ?? {})) {
          updateCachedFolderUnread(accountId, 'inbox', unread)
        }
      }
      moreThreads = res.threads || []
      mail$.threadAccountCursors.set({})
      mail$.threadsCursor.set(res.next_cursor ?? '')
    } else {
      const cursor = mail$.threadsCursor.get()
      if (!cursor) return
      const res = await invoke<{ threads: Message[]; next_cursor?: string; folder_unread?: number }>(
        'mail.threadList',
        {
          account_id: selectedAcc,
          folder_id: selectedFol,
          query: q,
          filter,
          before_cursor: cursor,
          refresh: false,
        },
      )
      if (!stillCurrent(cursor)) return
      if (typeof res.folder_unread === 'number') updateCachedFolderUnread(selectedAcc, selectedFol, res.folder_unread)
      moreThreads = res.threads || []
      mail$.threadsCursor.set(res.next_cursor ?? '')
    }

    if (moreThreads.length > 0) {
      const existing = mail$.threads.get()
      const seen = new Set(existing.map((thread) => thread.thread_id))
      const merged = [...existing, ...moreThreads.filter((thread) => !seen.has(thread.thread_id))]
      if (selectedAcc === 'unified') {
        merged.sort((a, b) => b.date - a.date)
      }
      mail$.threads.set(merged)
    }
  } finally {
    mail$.threadsLoadingMore.set(false)
  }
}

export async function loadThread(threadId: string) {
  mail$.threadLoading.set(true)
  if (mail$.threadErrorId.get() === threadId) mail$.threadErrorId.set('')
  try {
    const result = await invoke<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
      thread_id: threadId,
      limit: CONVERSATION_PAGE_SIZE,
    })
    // Guard against a stale response: the user may have switched threads while
    // this was in flight (e.g. during the ancestor fetch). Don't overwrite the
    // newer thread's messages — and let that newer load own the loading flag.
    if (ui$.selectedThread.get() !== threadId) return
    // Bodies still filling in the background arrive via the `mail.synced`
    // re-read; until then hide their placeholders rather than render empty
    // bubbles.
    const refreshed = result.messages.filter((message) => !message.body_missing)
    const messages = mergeRefreshedThreadMessages(mail$.messages.get(), refreshed, threadId)
    mail$.messages.set(messages)
    mail$.messagesCursor.set(result.next_cursor ?? '')
    mail$.messagesLoadingMore.set(false)
    // Reconcile against the unfiltered page: a hidden placeholder can still be
    // unread, and dropping it must not mark the thread read early.
    reconcileThreadUnreadFromLoadedMessages(threadId, result.messages, result.next_cursor)
  } catch (err) {
    // Without this the failure is silent: the finally below clears the
    // spinner and the reader sits blank. Flag the thread so the pane can
    // offer a retry; keep it only if the user is still looking at it.
    console.error('threadRead failed', err)
    if (ui$.selectedThread.get() === threadId) mail$.threadErrorId.set(threadId)
  } finally {
    if (ui$.selectedThread.get() === threadId) {
      mail$.threadLoading.set(false)
    }
  }
}

/**
 * Keep optimistic sends visible while the server's Sent copy catches up. Some
 * providers expose it over IMAP only after SMTP has returned (Proton Bridge can
 * take several seconds), so replacing the thread page wholesale creates a gap
 * where the reply disappears. Once the canonical row arrives, it replaces the
 * local bubble instead of rendering twice.
 */
export function mergeRefreshedThreadMessages(current: Message[], refreshed: Message[], threadId: string): Message[] {
  const canonicalMessageIds = new Set(
    refreshed.map((message) => normalizeMessageId(message.message_id)).filter(Boolean),
  )
  const unresolved = current.filter((message) => {
    if (message.thread_id !== threadId || !isLocalSendId(message.id)) return false
    const messageId = normalizeMessageId(message.message_id)
    return !messageId || !canonicalMessageIds.has(messageId)
  })
  // Fallback candidates: outgoing, non-draft rows this refresh newly revealed.
  // A message we were already showing before the send cannot be its server
  // copy, and a draft — even one holding this very reply — is not a sent copy.
  const known = new Set(current.map((message) => message.id))
  const candidates = refreshed.filter(
    (message) =>
      !isLocalSendId(message.id) &&
      !known.has(message.id) &&
      message.outgoing &&
      !isDraftFolder(message.folder_id, message.account_id),
  )
  const paired = pairLocalSendsWithServerCopies(unresolved, candidates)
  const optimistic = unresolved.filter((message) => !paired.has(message.id))
  if (optimistic.length === 0) return refreshed
  return [...refreshed, ...optimistic].sort((a, b) => a.date - b.date)
}

/** How far the server's Date header may sit from the moment we rendered the
 * bubble and still be the same message — enough for a slow submission plus
 * modest clock skew, short enough not to swallow a genuinely later reply. */
const SENT_COPY_MATCH_WINDOW_SECONDS = 600

/**
 * Match optimistic bubbles to the server's copies of them when the Message-ID
 * we generated didn't come back. Proton Bridge replaces that id with one of its
 * own (`@protonmail.internalid`), so identity has to come from the envelope:
 * same account and sender, same subject, same recipients, and a send time close
 * to when we rendered the bubble.
 *
 * Two replies into one thread share every one of those fields, so pairing is
 * decided globally rather than by first match: every plausible pair is ranked
 * by whether the content matches and then by how far apart the two times are,
 * and pairs are taken best-first. That keeps a copy arriving out of order from
 * claiming the wrong bubble — which would hide one reply and show the other
 * twice — while still settling on time alone when a server reflows the body it
 * stored and no content match exists.
 *
 * Returns the ids of the bubbles that found a copy.
 */
function pairLocalSendsWithServerCopies(locals: Message[], candidates: Message[]): Set<string> {
  const pairs: { localId: string; candidateId: string; contentMismatch: number; skew: number }[] = []
  for (const local of locals) {
    for (const candidate of candidates) {
      if (!isPlausibleSentCopy(local, candidate)) continue
      pairs.push({
        localId: local.id,
        candidateId: candidate.id,
        contentMismatch: contentSignature(local) === contentSignature(candidate) ? 0 : 1,
        skew: Math.abs(candidate.date - local.date),
      })
    }
  }
  pairs.sort((a, b) => a.contentMismatch - b.contentMismatch || a.skew - b.skew)

  const pairedLocals = new Set<string>()
  const claimed = new Set<string>()
  for (const pair of pairs) {
    if (pairedLocals.has(pair.localId) || claimed.has(pair.candidateId)) continue
    pairedLocals.add(pair.localId)
    claimed.add(pair.candidateId)
  }
  return pairedLocals
}

/** The envelope test every pair must clear before ranking. */
function isPlausibleSentCopy(local: Message, candidate: Message): boolean {
  if (candidate.account_id !== local.account_id) return false
  if (bareAddr(candidate.from_addr ?? '') !== bareAddr(local.from_addr ?? '')) return false
  if ((candidate.subject ?? '').trim() !== (local.subject ?? '').trim()) return false
  if (recipientKey(candidate) !== recipientKey(local)) return false
  return Math.abs(candidate.date - local.date) <= SENT_COPY_MATCH_WINDOW_SECONDS
}

/** What distinguishes two replies that share an envelope: what they say and
 * what they carry. Whitespace-insensitive, since a server may rewrap the body
 * it stored — a mismatch demotes a pair rather than rejecting it. */
function contentSignature(message: Message): string {
  const body = (message.body ?? '').replace(/\s+/g, ' ').trim().toLowerCase()
  const files = (message.attachments ?? [])
    .map((attachment) => attachment.filename.trim().toLowerCase())
    .sort()
    .join('|')
  return `${files}\u0000${body}`
}

/** Order-independent set of the bare To/Cc addresses, for envelope comparison. */
function recipientKey(message: Message): string {
  return [...splitAddressList(message.to), ...splitAddressList(message.cc)]
    .map(bareAddr)
    .filter(Boolean)
    .sort()
    .join(',')
}

// The sidecar accepts RFC Message-IDs both with and without their header angle
// brackets. Cached messages retain the spelling returned by the mail server,
// so every frontend identity comparison uses this same equivalence.
export function normalizeMessageId(value: string | undefined): string {
  return (value ?? '').trim().replace(/^<|>$/g, '').toLowerCase()
}

export async function loadMoreMessages(threadId: string) {
  const cursor = mail$.messagesCursor.get()
  if (!cursor || mail$.messagesLoadingMore.get()) return
  // Guard against a stale click after the thread switched out from under us.
  if (ui$.selectedThread.get() !== threadId) return
  mail$.messagesLoadingMore.set(true)
  try {
    const result = await invoke<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
      thread_id: threadId,
      limit: CONVERSATION_PAGE_SIZE,
      before_cursor: cursor,
    })
    if (ui$.selectedThread.get() !== threadId) return
    // Prepend the older page; engine returns ascending order within the page.
    const existing = mail$.messages.get()
    const seen = new Set(existing.map((m) => m.id))
    const merged = [...result.messages.filter((m) => !seen.has(m.id) && !m.body_missing), ...existing]
    mail$.messages.set(merged)
    mail$.messagesCursor.set(result.next_cursor ?? '')
  } finally {
    mail$.messagesLoadingMore.set(false)
  }
}

export async function markThreadRead(threadId: string) {
  if (!threadId) return
  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousFolders = mail$.folders.get()
  const hasUnread =
    previousThreads.some((thread) => thread.thread_id === threadId && thread.unread) ||
    previousMessages.some((message) => message.thread_id === threadId && message.unread) ||
    Object.values(kanban$.threads.get()).some((threads) =>
      threads.some((thread) => thread.thread_id === threadId && thread.unread),
    )
  if (!hasUnread) return

  const localThread = previousThreads.find((thread) => thread.thread_id === threadId)
  const localMessages = previousMessages.filter((message) => message.thread_id === threadId)
  const localMessageUnread = localMessages.filter((message) => message.unread).length
  const unreadCount = Math.max(1, localThread?.unread_count ?? localMessageUnread)
  const accountId = localThread?.account_id || localMessages[0]?.account_id
  const folderId = localThread?.folder_id || localMessages[0]?.folder_id
  const kanbanKeys = kanbanKeysWithThread(threadId)
  const previousAccountFolders = captureKeys(mail$.foldersByAccount.get(), accountId ? [accountId] : [])
  const previousKanbanThreads = captureKeys(kanban$.threads.get(), kanbanKeys)
  const previousKanbanUnreadCounts = captureKeys(kanban$.unreadCounts.get(), kanbanKeys)

  mail$.readThreads[threadId].set(true)
  mail$.threads.set(
    previousThreads.map((thread) =>
      thread.thread_id === threadId ? { ...thread, unread: false, unread_count: 0 } : thread,
    ),
  )
  mail$.messages.set(
    previousMessages.map((message) => (message.thread_id === threadId ? { ...message, unread: false } : message)),
  )
  decrementFolderUnread(accountId, folderId, unreadCount)
  updateKanbanThread(threadId, (thread) => ({ ...thread, unread: false, unread_count: 0 }))

  try {
    applyMutationFolderUnreads(await invoke<MutationResult>('mail.markRead', { thread_id: threadId }))
  } catch (error) {
    mail$.readThreads[threadId].delete()
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    mail$.folders.set(previousFolders)
    restoreAccountFolders(previousAccountFolders)
    restoreKanbanColumns(previousKanbanThreads, previousKanbanUnreadCounts)
    throw error
  } finally {
    refreshFoldersAfterFlagChange(findLocalThread(threadId)?.account_id)
  }
}

// Flag a thread as unread. The gesture means "bring this back to me", so it
// marks the newest message only — matching the core, which does the same
// server-side. Marking every message unread would reopen the thread at its
// oldest message and shed the count again as the reader scrolls down.
export async function markThreadUnread(threadId: string) {
  if (!threadId) return
  const threadMessages = mail$.messages.get().filter((message) => message.thread_id === threadId)
  const newestMessage = threadMessages.reduce<Message | null>(
    (newest, message) => (!newest || message.date >= newest.date ? message : newest),
    null,
  )
  const alreadyUnread =
    !mail$.threads.get().some((thread) => thread.thread_id === threadId && !thread.unread) &&
    (!newestMessage || newestMessage.unread) &&
    !Object.values(kanban$.threads.get()).some((threads) =>
      threads.some((thread) => thread.thread_id === threadId && !thread.unread),
    )
  if (alreadyUnread) return

  mail$.readThreads[threadId].delete()

  mail$.threads.set(
    mail$.threads
      .get()
      .map((thread) => (thread.thread_id === threadId ? { ...thread, unread: true, unread_count: 1 } : thread)),
  )
  if (newestMessage) {
    mail$.messages.set(
      mail$.messages.get().map((message) => (message.id === newestMessage.id ? { ...message, unread: true } : message)),
    )
  }
  updateKanbanThread(threadId, (thread) => ({
    ...thread,
    unread: true,
    unread_count: 1,
  }))

  applyMutationFolderUnreads(await invoke<MutationResult>('mail.markRead', { thread_id: threadId, seen: false }))

  refreshFoldersAfterFlagChange(findLocalThread(threadId)?.account_id)
}

export async function starThread(threadId: string, starred: boolean) {
  if (!threadId) return

  // Optimistic update
  mail$.threads.set(
    mail$.threads.get().map((thread) => (thread.thread_id === threadId ? { ...thread, starred } : thread)),
  )
  mail$.messages.set(
    mail$.messages.get().map((message) => (message.thread_id === threadId ? { ...message, starred } : message)),
  )
  updateKanbanThread(threadId, (thread) => ({ ...thread, starred }))

  applyMutationFolderUnreads(await invoke<MutationResult>('mail.markStarred', { thread_id: threadId, starred }))
}

// Flip a thread's star and show an undo toast — used by the keyboard shortcut,
// where an accidental press should be trivially recoverable.
export function toggleStarWithUndo(threadId: string) {
  if (!threadId) return
  const thread = findLocalThread(threadId)
  if (!thread) return
  const next = !thread.starred
  void starThread(threadId, next)
  showUndoToast(next ? 'Starred' : 'Unstarred', () => void starThread(threadId, !next))
}

// Mark a thread unread and show an undo toast (revert = mark read again).
export function markUnreadWithUndo(threadId: string) {
  if (!threadId) return
  void markThreadUnread(threadId)
  showUndoToast('Marked unread', () => void markThreadRead(threadId))
}

// Neighbour of a thread inside the kanban column that holds it (next, or
// previous if it was last). The chat-view getFilteredThreads list doesn't apply
// in kanban, where cards live in per-column lists.
function kanbanNeighbourThreadId(threadId: string): string {
  for (const threads of Object.values(kanban$.threads.get())) {
    const index = threads.findIndex((thread) => thread.thread_id === threadId)
    if (index === -1) continue
    const neighbour = threads[index + 1] ?? threads[index - 1]
    return neighbour?.thread_id ?? ''
  }
  return ''
}

function removeThreadLocally(threadId: string) {
  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousKanbanThreads = captureKeys(kanban$.threads.get(), kanbanKeysWithThread(threadId))
  const previousSelected = ui$.selectedThread.get()
  const previousPaneThreadId = kanban$.paneThreadId.get()
  // When the selected thread is the one leaving, advance to its neighbour in the
  // visible list (next, or previous if it was last) instead of snapping back to
  // the top — this keeps keyboard triage (e/# on the j/k selection) in place.
  // Computed before mutating mail$.threads, since getFilteredThreads reads it.
  let nextSelected = previousSelected
  if (previousSelected === threadId) {
    if (kanban$.activeBoardId.get()) {
      nextSelected = kanbanNeighbourThreadId(threadId)
    } else {
      const visible = getFilteredThreads()
      const index = visible.findIndex((thread) => thread.thread_id === threadId)
      // Skip over rows of the deleted thread itself — the unified starred folder
      // can list the same thread once per folder it is starred in.
      const neighbour =
        index === -1
          ? undefined
          : (visible.slice(index + 1).find((item) => item.thread_id !== threadId) ??
            visible
              .slice(0, index)
              .reverse()
              .find((item) => item.thread_id !== threadId))
      nextSelected = neighbour?.thread_id ?? ''
    }
  }
  const nextThreads = previousThreads.filter((thread) => thread.thread_id !== threadId)

  mail$.threads.set(nextThreads)
  mail$.messages.set(previousMessages.filter((message) => message.thread_id !== threadId))
  removeKanbanThread(threadId)
  if (previousSelected === threadId) {
    ui$.selectedThread.set(nextSelected)
    // No neighbour in the list we have: let the fresh list pick the replacement.
    if (!nextSelected) requestThreadReselect()
  }
  // If the kanban conversation pane was open on the deleted card, follow the
  // selection so it doesn't keep rendering the removed thread.
  if (previousPaneThreadId === threadId) {
    kanban$.paneThreadId.set(nextSelected)
  }

  return {
    rollback: () => {
      mail$.threads.set(previousThreads)
      mail$.messages.set(previousMessages)
      restoreKanbanColumns(previousKanbanThreads)
      ui$.selectedThread.set(previousSelected)
      if (previousPaneThreadId === threadId) {
        kanban$.paneThreadId.set(previousPaneThreadId)
      }
    },
  }
}

async function refreshThreadLocation(accountId?: string, refresh = false) {
  await loadThreads(refresh)
  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) {
    void loadFolders(selectedAcc, false)
  }
  if (accountId && accountId !== selectedAcc) {
    void loadFolders(accountId, false)
  }
}

export async function moveThreadToFolder(threadId: string, targetFolderId: string, options: { undo?: boolean } = {}) {
  if (!threadId || !targetFolderId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  if (sourceFolder === targetFolderId) return
  const targetThreadId = threadIdInFolder(threadId, sourceThread?.account_id, targetFolderId)

  const { rollback } = removeThreadLocally(threadId)
  try {
    const res = await invoke('mail.move', { thread_id: threadId, target_folder_id: targetFolderId })
    assertMoveAffected(res)
    applyMutationFolderUnreads(res as MutationResult)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.moveFailedInSameFolder'), 'error')
    } else if (options.undo !== false && sourceFolder) {
      showUndoToast(
        t('mail.toast.threadMoved'),
        () => void moveThreadToFolder(targetThreadId, sourceFolder, { undo: false }),
      )
    } else {
      showToast(t('mail.toast.threadMoved'))
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.moveFailed'), 'error')
  }
}

export async function copyThreadToFolder(threadId: string, targetAccountId: string, targetFolderId: string) {
  if (!threadId || !targetAccountId || !targetFolderId) return
  const sourceThread = findLocalThread(threadId)
  try {
    const res = await invoke('mail.copy', {
      thread_id: threadId,
      target_account_id: targetAccountId,
      target_folder_id: targetFolderId,
    })
    assertCopyAffected(res)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (targetAccountId !== sourceThread?.account_id) {
      void loadFolders(targetAccountId, false)
    }
    showToast(t('mail.toast.threadCopied'))
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('mail.toast.copyFailed'), 'error')
  }
}

function uniqueThreadItems(items: BulkSelectionItem[]) {
  const byThread = new Map<string, BulkSelectionItem>()
  for (const item of items) {
    if (!item.threadId || item.kind !== 'mail') continue
    if (!byThread.has(item.threadId)) byThread.set(item.threadId, item)
  }
  return [...byThread.values()]
}

export async function bulkMarkSelectedRead(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items)
  await Promise.all(targets.map((item) => markThreadRead(item.threadId)))
  clearBulkSelection()
  showToast(t('mail.toast.markedReadCount', { count: targets.length }))
}

export async function bulkMarkSelectedUnread(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items)
  await Promise.all(targets.map((item) => markThreadUnread(item.threadId)))
  clearBulkSelection()
  showToast(t('mail.toast.markedUnreadCount', { count: targets.length }))
}

export async function bulkStarSelected(items: BulkSelectionItem[], starred: boolean) {
  const targets = uniqueThreadItems(items)
  await Promise.all(targets.map((item) => starThread(item.threadId, starred)))
  clearBulkSelection()
  showToast(starred ? t('mail.toast.starredSelected') : t('mail.toast.unstarredSelected'))
}

export async function bulkArchiveSelected(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items).filter((item) => !item.draft && !item.trash)
  if (targets.length === 0) return
  const rollbacks: Array<() => void> = []
  try {
    for (const item of targets) {
      const sourceThread = findLocalThread(item.threadId)
      rollbacks.push(removeThreadLocally(item.threadId).rollback)
      const res = await invoke('mail.archive', { thread_id: item.threadId })
      assertMoveAffected(res, 'Archive')
      applyMutationFolderUnreads(res as MutationResult)
      if (sourceThread?.account_id) void refreshAccountFoldersCache(sourceThread.account_id, false)
    }
    await refreshThreadLocation(undefined, true)
    clearBulkSelection()
    showToast(t('mail.toast.archivedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.archiveFailed'), 'error')
  }
}

export async function bulkMoveSelectedToFolder(items: BulkSelectionItem[], targetFolderId: string) {
  const targets = uniqueThreadItems(items).filter((item) => item.folderId !== targetFolderId)
  if (targets.length === 0) return
  const rollbacks: Array<() => void> = []
  try {
    for (const item of targets) {
      rollbacks.push(removeThreadLocally(item.threadId).rollback)
      const res = await invoke('mail.move', { thread_id: item.threadId, target_folder_id: targetFolderId })
      assertMoveAffected(res)
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(targets[0]?.accountId, true)
    clearBulkSelection()
    showToast(t('mail.toast.movedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.moveFailed'), 'error')
  }
}

export async function bulkCopySelectedToFolder(
  items: BulkSelectionItem[],
  targetAccountId: string,
  targetFolderId: string,
) {
  const targets = uniqueThreadItems(items)
  if (targets.length === 0) return
  try {
    for (const item of targets) {
      const res = await invoke('mail.copy', {
        thread_id: item.threadId,
        target_account_id: targetAccountId,
        target_folder_id: targetFolderId,
      })
      assertCopyAffected(res)
    }
    await refreshThreadLocation(targets[0]?.accountId, true)
    if (targetAccountId !== targets[0]?.accountId) void loadFolders(targetAccountId, false)
    clearBulkSelection()
    showToast(t('mail.toast.copiedCount', { count: targets.length }))
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('mail.toast.copyFailed'), 'error')
  }
}

export async function bulkDeleteSelected(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items)
  if (targets.length === 0) return
  const permanentTargets = targets.filter(
    (item) => item.draft || item.trash || isTrashFolderId(item.accountId, item.folderId),
  )
  if (permanentTargets.length > 0) {
    const confirmed = await confirmAction({
      title: permanentTargets.length === targets.length ? 'Delete selected forever?' : 'Delete selected threads?',
      message:
        permanentTargets.length === targets.length
          ? `${permanentTargets.length} selected thread(s) will be permanently deleted. This can't be undone.`
          : `${permanentTargets.length} selected thread(s) will be permanently deleted; the rest will move to Trash.`,
      confirmLabel: 'Delete selected',
      tone: 'danger',
    })
    if (!confirmed) return
  }

  const rollbacks: Array<() => void> = []
  try {
    for (const item of targets) {
      rollbacks.push(removeThreadLocally(item.threadId).rollback)
      const res = await invoke('mail.delete', {
        thread_id: item.threadId,
        ...(item.folderId ? { folder: item.folderId } : {}),
      })
      assertDeleteAffected(res)
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(undefined, true)
    clearBulkSelection()
    showToast(t('mail.toast.deletedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
  }
}

export async function archiveThread(threadId: string) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  const { rollback } = removeThreadLocally(threadId)
  try {
    const res = await invoke<{ folder?: string; thread_id?: string } & MutationResult>('mail.archive', {
      thread_id: threadId,
    })
    assertMoveAffected(res, 'Archive')
    applyMutationFolderUnreads(res)
    const archivedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.folder)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.archiveFailedInSameFolder'), 'error')
    } else if (sourceFolder) {
      // Offer to move it back where it came from. Falls back to a plain toast
      // when the origin folder is unknown (nothing reliable to restore to).
      showUndoToast(
        t('mail.toast.archivedCount', { count: 1 }),
        () => void moveThreadToFolder(archivedThreadId, sourceFolder, { undo: false }),
      )
    } else {
      showToast(t('mail.toast.archivedCount', { count: 1 }))
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.archiveFailed'), 'error')
  }
}

export async function deleteThread(threadId: string, options: { permanent?: boolean } = {}) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  // Drafts are expunged in place by the engine (never moved to Trash), so the
  // delete is permanent there too — but worded as a discard.
  const isDraft =
    sourceThread?.folder_role === 'drafts' ||
    (!sourceThread?.folder_role && isDraftFolder(sourceFolder, sourceThread?.account_id))
  const isTrash =
    sourceThread?.folder_role === 'trash' ||
    (!sourceThread?.folder_role && isTrashFolderId(sourceThread?.account_id ?? '', sourceFolder))
  const permanent = options.permanent ?? (isDraft || isTrash)
  if (isDraft || permanent) {
    if (
      !(await confirmAction({
        title: isDraft ? 'Discard draft?' : 'Delete thread forever?',
        message: isDraft
          ? "This draft will be permanently deleted. This can't be undone."
          : "This thread will be permanently deleted. This can't be undone.",
        confirmLabel: isDraft ? 'Discard' : 'Delete forever',
        tone: 'danger',
      }))
    ) {
      return
    }
  }

  const { rollback } = removeThreadLocally(threadId)

  try {
    const res = await invoke<
      { deleted?: number; permanent?: boolean; trash?: string; thread_id?: string } & MutationResult
    >('mail.delete', {
      thread_id: threadId,
      ...(sourceFolder ? { folder: sourceFolder } : {}),
    })
    assertDeleteAffected(res)
    applyMutationFolderUnreads(res)
    const trashedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.trash)
    const canUndoTrashMove = !!(res.thread_id || res.trash)
    await refreshThreadLocation(undefined, true)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.deleteFailedInSameFolder'), 'error')
    } else if (!isDraft && !permanent && !res.permanent && sourceFolder && canUndoTrashMove) {
      showUndoToast(
        t('mail.toast.threadMovedToTrash'),
        () => void moveThreadToFolder(trashedThreadId, sourceFolder, { undo: false }),
      )
    } else {
      showToast(
        isDraft
          ? t('mail.toast.draftDiscarded')
          : permanent
            ? t('mail.toast.deletedCount', { count: 1 })
            : t('mail.toast.threadMovedToTrash'),
      )
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
  }
}

// Prefer the core-provided folder role. The name fallback is only for call
// sites that have a bare folder id before folder metadata is loaded.
export function isDraftFolder(folderId: string, accountId?: string): boolean {
  const candidates = [...(accountId ? (mail$.foldersByAccount[accountId].get() ?? []) : []), ...mail$.folders.get()]
  const folder = candidates.find((item) => item.id === folderId || item.name === folderId)
  if (folder?.role) return folder.role === 'drafts'
  return ['drafts', 'draft', 'inbox.drafts', 'inbox.draft', '[gmail]/drafts', '[gmail]/draft'].includes(
    folderId.toLowerCase(),
  )
}

/** Whether a message lives in the account's inbox — i.e. it was delivered to us.
 * Same role-first, name-fallback shape as {@link isDraftFolder}. */
export function isInboxFolder(folderId: string, accountId?: string): boolean {
  const candidates = [...(accountId ? (mail$.foldersByAccount[accountId].get() ?? []) : []), ...mail$.folders.get()]
  const folder = candidates.find((item) => item.id === folderId || item.name === folderId)
  if (folder?.role) return folder.role === 'inbox'
  return folderId.toLowerCase() === 'inbox'
}

export async function discardSavedDraftCopy(
  draft: {
    threadId: string
    messageId: string
    folderId: string
    accountId?: string
    draftMessageId?: string
  },
  options: { throwOnError?: boolean; failureMessage?: string } = {},
): Promise<boolean> {
  if (!draft.accountId && !draft.threadId) return true

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const discardedDraftMessageId = normalizeMessageId(draft.draftMessageId)
  const withoutDiscardedDraft = (messages: Message[]) =>
    messages.filter((message) => {
      if (draft.messageId && message.id === draft.messageId) return false
      if (
        discardedDraftMessageId &&
        normalizeMessageId(message.message_id) === discardedDraftMessageId &&
        isDraftFolder(message.folder_id, message.account_id)
      ) {
        return false
      }
      return true
    })
  const nextMessages = withoutDiscardedDraft(previousMessages)
  const selectedThread = ui$.selectedThread.get()
  const removeThread = !!draft.threadId && !nextMessages.some((message) => message.thread_id === draft.threadId)
  const kanbanKeys = kanbanKeysWithThread(draft.threadId)

  mail$.messages.set(nextMessages)
  if (removeThread) {
    mail$.threads.set(previousThreads.filter((thread) => thread.thread_id !== draft.threadId))
    removeKanbanThread(draft.threadId)
    if (selectedThread === draft.threadId) {
      ui$.selectedThread.set('')
      requestThreadReselect()
    }
  } else {
    reconcileThreadDraftFromLoadedMessages(draft.threadId, nextMessages)
  }

  try {
    if (draft.accountId && draft.draftMessageId) {
      await invoke('mail.discardDraft', {
        account_id: draft.accountId,
        draft_id: draft.draftMessageId,
        thread_id: draft.threadId,
      })
    } else {
      const res = await invoke('mail.delete', {
        thread_id: draft.threadId,
        message_ids: [draft.messageId],
        folder: draft.folderId,
      })
      if (!isDraftFolder(draft.folderId, draft.accountId)) assertDeleteAffected(res)
      applyMutationFolderUnreads(res as MutationResult)
    }
    await reloadThreadCards(kanbanKeys)
    // A mail.synced thread refresh can land while the server discard is on the
    // wire and reinsert the stale draft row after the optimistic removal above.
    // The discard has now succeeded, so remove that copy again before deriving
    // the conversation and thread-card draft state.
    const reconciledMessages = withoutDiscardedDraft(mail$.messages.get())
    if (reconciledMessages.length !== mail$.messages.get().length) mail$.messages.set(reconciledMessages)
    if (!removeThread) reconcileThreadDraftFromLoadedMessages(draft.threadId, reconciledMessages)
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) void loadFolders(selectedAcc, false)
    if (draft.accountId && draft.accountId !== selectedAcc) void loadFolders(draft.accountId, false)
    return true
  } catch (error) {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    if (options.throwOnError) throw error
    const message = options.failureMessage ?? "Sent, but couldn't discard draft"
    showToast(error instanceof Error ? `${message}: ${error.message}` : message, 'error')
    return false
  }
}

// Delete a single message (e.g. one draft) out of a thread, leaving the rest of
// the conversation intact. The message keeps its own folder_id so the delete
// targets the mailbox the message actually lives in, not the thread's folder.
// Drafts are discarded permanently (engine expunges them); other messages go to
// Trash. The confirm/toast wording reflects which.
export async function deleteMessage(message: Message) {
  if (!message?.id) return

  // Local-only optimistic send (still sending, or failed): it has no
  // server-side copy under this id, so just drop it from the pane and forget
  // any pending retry payload — no backend round-trip, no confirm.
  if (isLocalSendId(message.id)) {
    discardPendingSend(message.id)
    mail$.messages.set(mail$.messages.get().filter((item) => item.id !== message.id))
    return
  }

  const isDraft = isDraftFolder(message.folder_id, message.account_id)
  const confirmMessage = isDraft ? "Discard this draft? This can't be undone." : 'Move this message to Trash?'
  if (
    !(await confirmAction({
      title: isDraft ? 'Discard draft?' : 'Move message to Trash?',
      message: confirmMessage,
      confirmLabel: isDraft ? 'Discard' : 'Move to Trash',
      tone: 'danger',
    }))
  ) {
    return
  }

  const threadId = message.thread_id
  const previousMessages = mail$.messages.get()
  const nextMessages = previousMessages.filter((item) => item.id !== message.id)
  const kanbanKeys = kanbanKeysWithThread(threadId)

  mail$.messages.set(nextMessages)

  try {
    const res = await invoke('mail.delete', {
      thread_id: threadId,
      message_ids: [message.id],
      folder: message.folder_id,
    })
    if (!isDraft) assertDeleteAffected(res)
    applyMutationFolderUnreads(res as MutationResult)
    showToast(isDraft ? t('mail.toast.draftDiscarded') : t('mail.toast.messageMovedToTrash'))
    // No messages left from this thread: drop the now-empty conversation.
    if (!nextMessages.some((item) => item.thread_id === threadId)) {
      if (ui$.selectedThread.get() === threadId) {
        ui$.selectedThread.set('')
        requestThreadReselect()
      }
      removeKanbanThread(threadId)
    }
    await reloadThreadCards(kanbanKeys)
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) void loadFolders(selectedAcc, false)
    if (message.account_id && message.account_id !== selectedAcc) {
      void loadFolders(message.account_id, false)
    }
  } catch (error) {
    mail$.messages.set(previousMessages)
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
  }
}

// Mark the current folder/view as read. Mail accounts are marked folder-wide, so
// unread messages outside the loaded page are cleared too; RSS feeds are marked
// per visible thread because they do not have an IMAP-style folder flag.
export async function markAllRead() {
  const threads = mail$.threads.get()
  const unread = threads.filter((thread) => thread.unread)

  const accounts = accounts$.get()
  const selectedAcc = ui$.selectedAccount.get()
  const folder = selectedAcc === 'unified' ? unifiedFolderRole(ui$.selectedFolder.get()) : ui$.selectedFolder.get()
  const activeAccount = accounts.find((account) => account.id === selectedAcc)
  const mailAccountIds =
    selectedAcc === 'unified'
      ? unifiedAccounts()
          .filter((account) => !isRssAccount(account, account.id))
          .map((account) => account.id)
      : selectedAcc && !isRssAccount(activeAccount, selectedAcc)
        ? [selectedAcc]
        : []

  if (mailAccountIds.length === 0 && unread.length === 0) return

  // Optimistic clear for currently loaded rows. Folder cache refresh below brings
  // aggregate unread badges in line after the folder-wide backend update.
  mail$.threads.set(threads.map((thread) => (thread.unread ? { ...thread, unread: false, unread_count: 0 } : thread)))
  mail$.messages.set(mail$.messages.get().map((message) => (message.unread ? { ...message, unread: false } : message)))

  await Promise.all([
    ...(selectedAcc === 'unified' && mailAccountIds.length > 0 ? ['unified'] : mailAccountIds).map((accountId) =>
      invoke<MutationResult>('mail.markAllRead', { account_id: accountId, folder_id: folder })
        .then(applyMutationFolderUnreads)
        .catch((err) => console.error('markAllRead failed:', err)),
    ),
    ...unread
      .filter((thread) =>
        isRssAccount(
          accounts.find((account) => account.id === thread.account_id),
          thread.account_id,
        ),
      )
      .map((thread) =>
        invoke('mail.markRead', { thread_id: thread.thread_id }).catch((err) =>
          console.error('markAllRead (rss) failed:', err),
        ),
      ),
  ])

  if (selectedAcc) void loadFolders(selectedAcc, false)
  // The visible list can span multiple accounts (unified inbox, Starred, a
  // Kanban board), so refresh each affected account's folder cache — not just
  // the selected view — to keep every side navigation badge in sync.
  for (const accountId of new Set([...mailAccountIds, ...unread.map((thread) => thread.account_id)])) {
    if (accountId && accountId !== selectedAcc) void refreshAccountFoldersCache(accountId, false)
  }
}

// Mark one account's Inbox (or its synthetic RSS Inbox) read from account-level
// chrome such as the side navigation menu. This deliberately does not depend on
// the currently selected account/folder.
export async function markAccountInboxRead(accountId: string) {
  if (!accountId || accountId === 'unified') return

  const accountFolders = mail$.foldersByAccount[accountId].get() ?? []
  const inbox = accountFolders.find((folder) => folderMatches(folder, accountId, 'inbox'))
  const folderId = inbox?.id || 'inbox'
  const accountUnread = inbox?.unread ?? 0
  const includedInUnified = accounts$.get().find((account) => account.id === accountId)?.included_in_unified !== false
  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousFolders = mail$.folders.get()
  const previousAccountFolders = captureKeys(mail$.foldersByAccount.get(), [accountId])
  const inAccountInbox = (item: Pick<Message, 'account_id' | 'folder_id'>) =>
    item.account_id === accountId &&
    folderMatches({ id: item.folder_id, account_id: accountId, name: '', role: '', unread: 0 }, accountId, folderId)
  const affectedKanbanThreadIds = new Set(
    Object.values(kanban$.threads.get())
      .flat()
      .filter((thread) => thread.unread && inAccountInbox(thread))
      .map((thread) => thread.thread_id),
  )
  const directKanbanKeys = Array.from(
    new Set([...Object.keys(kanban$.threads.get()), ...Object.keys(kanban$.unreadCounts.get())]),
  ).filter((key) => {
    const [columnAccountId, columnFolderId] = key.split('\n')
    if (!columnAccountId || !columnFolderId) return false
    if (columnAccountId === 'unified') return includedInUnified && unifiedFolderRole(columnFolderId) === 'inbox'
    return (
      columnAccountId === accountId &&
      folderMatches({ id: columnFolderId, account_id: accountId, name: '', role: '', unread: 0 }, accountId, folderId)
    )
  })
  const affectedKanbanKeys = Array.from(
    new Set([...directKanbanKeys, ...Array.from(affectedKanbanThreadIds).flatMap(kanbanKeysWithThread)]),
  )
  const previousKanbanThreads = captureKeys(kanban$.threads.get(), affectedKanbanKeys)
  const previousKanbanUnreadCounts = captureKeys(kanban$.unreadCounts.get(), affectedKanbanKeys)

  mail$.threads.set(
    previousThreads.map((thread) =>
      thread.unread && inAccountInbox(thread) ? { ...thread, unread: false, unread_count: 0 } : thread,
    ),
  )
  mail$.messages.set(
    previousMessages.map((message) =>
      message.unread && inAccountInbox(message) ? { ...message, unread: false } : message,
    ),
  )
  for (const threadId of affectedKanbanThreadIds) {
    updateKanbanThread(threadId, (thread) => ({ ...thread, unread: false, unread_count: 0 }))
  }
  // These badges are backend folder totals, not counts of the loaded cards.
  // A single-account Inbox is fully cleared; a unified Inbox loses this
  // account's whole cached contribution.
  for (const key of directKanbanKeys) {
    const [columnAccountId] = key.split('\n')
    const previous = previousKanbanUnreadCounts.find(([candidate]) => candidate === key)?.[1] ?? 0
    kanban$.unreadCounts[key].set(columnAccountId === 'unified' ? Math.max(0, previous - accountUnread) : 0)
  }
  updateCachedFolderUnread(accountId, folderId, 0)

  try {
    const result = await invoke<MutationResult>('mail.markAllRead', { account_id: accountId, folder_id: folderId })
    applyMutationFolderUnreads(result)
    void refreshAccountFoldersCache(accountId, false)
  } catch (error) {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    mail$.folders.set(previousFolders)
    restoreAccountFolders(previousAccountFolders)
    restoreKanbanColumns(previousKanbanThreads, previousKanbanUnreadCounts)
    showToast(error instanceof Error ? error.message : t('notification.markReadFailed'), 'error')
  }
}

export async function markMessagesRead(threadId: string, messageIds: string[]) {
  const uniqueIds = Array.from(new Set(messageIds.filter(Boolean)))
  if (!threadId || uniqueIds.length === 0) return

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousFolders = mail$.folders.get()
  const previousReadThread = mail$.readThreads[threadId].get()
  const unreadIds = new Set(
    previousMessages
      .filter((message) => message.thread_id === threadId && message.unread && uniqueIds.includes(message.id))
      .map((message) => message.id),
  )
  if (unreadIds.size === 0) return

  const localThread = findLocalThread(threadId)
  const localMessage = previousMessages.find((message) => unreadIds.has(message.id))
  const unreadAccountId = localThread?.account_id || localMessage?.account_id
  const kanbanKeys = kanbanKeysWithThread(threadId)
  const previousAccountFolders = captureKeys(mail$.foldersByAccount.get(), unreadAccountId ? [unreadAccountId] : [])
  const previousKanbanThreads = captureKeys(kanban$.threads.get(), kanbanKeys)
  const previousKanbanUnreadCounts = captureKeys(kanban$.unreadCounts.get(), kanbanKeys)

  // Re-runs the optimistic update from the pre-change snapshots for an arbitrary
  // subset of ids, so a partial failure can keep the folders that succeeded.
  const applyOptimisticRead = (readIds: Set<string>) => {
    // A thread can span folders (an INBOX message and its reply in Sent), and
    // both a card's unread_count and a folder badge count only their own
    // mailbox, so every decrement below is tallied per folder.
    const fallbackFolder = localThread?.folder_id || localMessage?.folder_id
    const readByFolder = new Map<string, number>()
    for (const message of previousMessages) {
      if (!readIds.has(message.id)) continue
      const folderId = message.folder_id || fallbackFolder
      if (!folderId) continue
      readByFolder.set(folderId, (readByFolder.get(folderId) ?? 0) + 1)
    }
    // A card with no folder of its own falls back to the whole batch.
    const readInFolder = (folderId: string | undefined) => (folderId ? (readByFolder.get(folderId) ?? 0) : readIds.size)
    const cardAfterRead = (thread: Message) => {
      const read = readInFolder(thread.folder_id)
      if (read <= 0) return thread
      const unreadCount = Math.max(0, (thread.unread_count ?? (thread.unread ? 1 : 0)) - read)
      return { ...thread, unread: unreadCount > 0, unread_count: unreadCount }
    }

    mail$.messages.set(
      previousMessages.map((message) => (readIds.has(message.id) ? { ...message, unread: false } : message)),
    )
    mail$.threads.set(previousThreads.map((thread) => (thread.thread_id === threadId ? cardAfterRead(thread) : thread)))
    updateKanbanThread(threadId, cardAfterRead)
    const stillUnread =
      mail$.threads.get().some((thread) => thread.thread_id === threadId && thread.unread) ||
      mail$.messages.get().some((message) => message.thread_id === threadId && message.unread) ||
      Object.values(kanban$.threads.get()).some((threads) =>
        threads.some((thread) => thread.thread_id === threadId && thread.unread),
      )
    if (!stillUnread) mail$.readThreads[threadId].set(true)
    // The account stays the thread's: `previousAccountFolders` snapshots only
    // that one, so a rollback can still undo every decrement made here.
    for (const [folderId, count] of readByFolder) decrementFolderUnread(unreadAccountId, folderId, count)
  }
  // Restores every snapshot taken above, undoing an applyOptimisticRead call.
  const restoreSnapshots = () => {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    mail$.folders.set(previousFolders)
    restoreAccountFolders(previousAccountFolders)
    restoreKanbanColumns(previousKanbanThreads, previousKanbanUnreadCounts)
    if (previousReadThread === undefined) {
      mail$.readThreads[threadId].delete()
    } else {
      mail$.readThreads[threadId].set(previousReadThread)
    }
  }

  applyOptimisticRead(unreadIds)

  const accountId = findLocalThread(threadId)?.account_id
  try {
    // A thread can span folders, and IMAP UIDs are mailbox-local, so each folder
    // gets its own call.
    const messagesByFolder = new Map<string, string[]>()
    for (const message of previousMessages.filter((item) => unreadIds.has(item.id))) {
      const ids = messagesByFolder.get(message.folder_id) ?? []
      ids.push(message.id)
      messagesByFolder.set(message.folder_id, ids)
    }
    const folders = Array.from(messagesByFolder)
    const results = await Promise.allSettled(
      folders.map(([folder, message_ids]) => invoke('mail.markRead', { thread_id: threadId, folder, message_ids })),
    )
    const failed = results.flatMap((result, index) => (result.status === 'rejected' ? [index] : []))
    if (failed.length > 0) {
      const failedIds = new Set(failed.flatMap((index) => folders[index][1]))
      restoreSnapshots()
      // Only the folders that failed go back to unread.
      const readIds = new Set(Array.from(unreadIds).filter((id) => !failedIds.has(id)))
      if (readIds.size > 0) applyOptimisticRead(readIds)
      throw (results[failed[0]] as PromiseRejectedResult).reason
    }
  } finally {
    refreshFoldersAfterFlagChange(accountId)
  }
}

export async function markMessageReadState(message: Message, seen: boolean) {
  if (!message?.id || !message.thread_id) return
  if (message.unread === !seen) return

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const kanbanKeys = kanbanKeysWithThread(message.thread_id)
  const previousKanbanThreads = captureKeys(kanban$.threads.get(), kanbanKeys)
  const previousKanbanUnreadCounts = captureKeys(kanban$.unreadCounts.get(), kanbanKeys)
  const delta = seen ? -1 : 1

  mail$.messages.set(previousMessages.map((item) => (item.id === message.id ? { ...item, unread: !seen } : item)))
  mail$.threads.set(
    previousThreads.map((thread) => {
      if (thread.thread_id !== message.thread_id) return thread
      const unreadCount = Math.max(0, (thread.unread_count ?? (thread.unread ? 1 : 0)) + delta)
      return { ...thread, unread: unreadCount > 0, unread_count: unreadCount }
    }),
  )
  updateKanbanThread(message.thread_id, (thread) => {
    const unreadCount = Math.max(0, (thread.unread_count ?? (thread.unread ? 1 : 0)) + delta)
    return { ...thread, unread: unreadCount > 0, unread_count: unreadCount }
  })

  try {
    await invoke('mail.markRead', {
      thread_id: message.thread_id,
      folder: message.folder_id,
      message_ids: [message.id],
      seen,
    })
  } catch (error) {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    restoreKanbanColumns(previousKanbanThreads, previousKanbanUnreadCounts)
    throw error
  } finally {
    refreshFoldersAfterFlagChange(message.account_id)
  }
}

export async function starMessage(message: Message, starred: boolean) {
  if (!message?.id || !message.thread_id) return

  const nextMessages = mail$.messages.get().map((item) => (item.id === message.id ? { ...item, starred } : item))
  const threadStarred = nextMessages.some((item) => item.thread_id === message.thread_id && item.starred)

  mail$.messages.set(nextMessages)
  // The starred folder lists a thread only while some message in it is starred,
  // so a row leaves as soon as the last star is cleared and appears the moment
  // the first one is set — including when starring from the open conversation,
  // where the thread isn't listed yet.
  if (isUnifiedStarred(ui$.selectedAccount.peek(), ui$.selectedFolder.peek())) {
    const rows = mail$.threads.get()
    const listed = rows.some((item) => item.thread_id === message.thread_id)
    if (!threadStarred) {
      mail$.threads.set(rows.filter((item) => item.thread_id !== message.thread_id))
    } else if (listed) {
      mail$.threads.set(
        rows.map((item) => (item.thread_id === message.thread_id ? { ...item, starred: threadStarred } : item)),
      )
    } else {
      mail$.threads.set([...rows, { ...message, starred: threadStarred }].sort((a, b) => b.date - a.date))
    }
  } else {
    mail$.threads.set(
      mail$.threads
        .get()
        .map((thread) => (thread.thread_id === message.thread_id ? { ...thread, starred: threadStarred } : thread)),
    )
  }
  updateKanbanThread(message.thread_id, (thread) => ({ ...thread, starred: threadStarred }))

  await invoke('mail.markStarred', {
    thread_id: message.thread_id,
    folder: message.folder_id,
    message_ids: [message.id],
    starred,
  })
}

export async function syncMail() {
  mail$.readThreads.set({})
  const selectedAcc = ui$.selectedAccount.get()
  if (!selectedAcc) return

  ui$.busy.set(true)
  try {
    if (selectedAcc === 'unified') {
      const accounts = unifiedAccounts()
      await Promise.all(
        accounts.map((acc) =>
          invoke('mail.sync', { account_id: acc.id }).catch((err) =>
            console.error(`Sync failed for ${acc.email}:`, err),
          ),
        ),
      )
    } else {
      await invoke('mail.sync', { account_id: selectedAcc })
    }
    await loadThreads()
    showToast(t('mail.toast.synced'))
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('mail.toast.syncFailed'), 'error')
  } finally {
    ui$.busy.set(false)
  }
}
