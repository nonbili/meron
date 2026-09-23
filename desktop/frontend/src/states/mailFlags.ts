import type { Message } from '../types'
import { invoke } from '../lib/bridge'
import { t } from '../lib/i18n'
import { clearBulkSelection, ui$, showToast, showUndoToast, type BulkSelectionItem } from './ui'
import { accounts$, unifiedAccounts } from './accounts'
import { kanban$ } from './kanban'
import { isRssAccount } from '../lib/threadActions'
import {
  accountFolderForRole,
  isUnifiedStarred,
  unifiedFolderRole,
  type UnifiedFolderRole,
} from '../lib/unifiedFolders'
import {
  captureKeys,
  findLocalThread,
  kanbanKeysWithThread,
  loadThreads,
  mail$,
  restoreAccountFolders,
  restoreKanbanColumns,
  threadListViewKey,
  uniqueThreadItems,
  updateKanbanThread,
} from './mail'
import {
  applyMutationFolderUnreads,
  decrementFolderUnread,
  folderMatches,
  loadFolders,
  type MutationResult,
  publishCachedFolders,
  refreshAccountFoldersCache,
  updateCachedFolderUnread,
} from './mailFolders'

// Read and starred state changes for threads and messages, with optimistic updates.

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

export async function starThread(threadId: string, starred: boolean, options: { refresh?: boolean } = {}) {
  try {
    await setThreadStar(threadId, starred, options)
    return true
  } catch (error) {
    showStarError(error, starred)
    return false
  }
}

function showStarError(error: unknown, starred: boolean) {
  showToast(error instanceof Error ? error.message : starred ? 'Star failed' : 'Unstar failed', 'error')
}

async function setThreadStar(threadId: string, starred: boolean, options: { refresh?: boolean } = {}) {
  if (!threadId) return

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousKanban = captureKeys(kanban$.threads.get(), kanbanKeysWithThread(threadId))
  const viewKey = () =>
    threadListViewKey(ui$.selectedAccount.peek(), ui$.selectedFolder.peek(), ui$.query.peek(), ui$.filterMode.peek())
  const previousView = viewKey()
  // Restore only this action's stars, preserving concurrent changes to other
  // threads (including successful actions in the same bulk operation).
  const restore = (rows: Message[], before: Message[], missing = false) => {
    const saved = before.filter((row) => row.thread_id === threadId)
    const byId = new Map(saved.map((row) => [row.id, row]))
    const restored = rows.map((row) => (byId.has(row.id) ? { ...row, starred: byId.get(row.id)!.starred } : row))
    if (missing) {
      for (const row of saved) {
        if (!restored.some((item) => item.id === row.id))
          restored.splice(Math.min(before.indexOf(row), restored.length), 0, row)
      }
    }
    return restored
  }

  // Optimistic update
  mail$.threads.set(
    mail$.threads.get().flatMap((thread) => {
      if (thread.thread_id !== threadId) return [thread]
      if (!starred && isUnifiedStarred(ui$.selectedAccount.peek(), ui$.selectedFolder.peek())) return []
      return [{ ...thread, starred }]
    }),
  )
  mail$.messages.set(
    mail$.messages.get().map((message) => (message.thread_id === threadId ? { ...message, starred } : message)),
  )
  updateKanbanThread(threadId, (thread) => ({ ...thread, starred }))

  try {
    applyMutationFolderUnreads(await invoke<MutationResult>('mail.markStarred', { thread_id: threadId, starred }))
  } catch (error) {
    if (viewKey() === previousView) mail$.threads.set(restore(mail$.threads.get(), previousThreads, true))
    mail$.messages.set(restore(mail$.messages.get(), previousMessages))
    for (const [key, rows] of previousKanban) {
      if (rows && kanban$.threads[key].peek()) kanban$.threads[key].set(restore(kanban$.threads[key].get(), rows, true))
    }
    throw error
  }
  if (options.refresh !== false && isUnifiedStarred(ui$.selectedAccount.peek(), ui$.selectedFolder.peek())) {
    await loadThreads(false)
  }
}

// Flip a thread's star and show an undo toast — used by the keyboard shortcut,
// where an accidental press should be trivially recoverable.
export function toggleStarWithUndo(threadId: string) {
  if (!threadId) return
  const thread = findLocalThread(threadId)
  if (!thread) return
  const next = !thread.starred
  void starThread(threadId, next).then((success) => {
    if (success) showUndoToast(next ? 'Starred' : 'Unstarred', () => void starThread(threadId, !next))
  })
}

// Mark a thread unread and show an undo toast (revert = mark read again).
export function markUnreadWithUndo(threadId: string) {
  if (!threadId) return
  void markThreadUnread(threadId)
  showUndoToast('Marked unread', () => void markThreadRead(threadId))
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
  if (targets.length === 0) return
  try {
    const results = await Promise.allSettled(
      targets.map((item) => setThreadStar(item.threadId, starred, { refresh: false })),
    )
    if (isUnifiedStarred(ui$.selectedAccount.peek(), ui$.selectedFolder.peek())) await loadThreads(false)
    const failure = results.find((result) => result.status === 'rejected')
    if (failure?.status === 'rejected') throw failure.reason
    showToast(starred ? t('mail.toast.starredSelected') : t('mail.toast.unstarredSelected'))
  } catch (error) {
    showStarError(error, starred)
  } finally {
    clearBulkSelection()
  }
}

// Mark the current folder/view as read. Mail accounts are marked folder-wide, so
// unread messages outside the loaded page are cleared too; RSS feeds are marked
// per visible thread because they do not have an IMAP-style folder flag, and so
// is unified Starred, which is a flag rather than a mailbox.
export async function markAllRead() {
  const threads = mail$.threads.get()
  const unread = threads.filter((thread) => thread.unread)

  const accounts = accounts$.get()
  const selectedAcc = ui$.selectedAccount.get()
  const starred = isUnifiedStarred(selectedAcc, ui$.selectedFolder.get())
  const folder = selectedAcc === 'unified' ? unifiedFolderRole(ui$.selectedFolder.get()) : ui$.selectedFolder.get()
  const activeAccount = accounts.find((account) => account.id === selectedAcc)
  const mailAccountIds = starred
    ? []
    : selectedAcc === 'unified'
      ? unifiedAccounts()
          .filter((account) => !isRssAccount(account, account.id))
          .map((account) => account.id)
      : selectedAcc && !isRssAccount(activeAccount, selectedAcc)
        ? [selectedAcc]
        : []

  if (mailAccountIds.length === 0 && unread.length === 0) return

  const itemUnread = starred
    ? unread
    : unread.filter((thread) =>
        isRssAccount(
          accounts.find((account) => account.id === thread.account_id),
          thread.account_id,
        ),
      )
  const affectedAccountIds = Array.from(
    new Set([...mailAccountIds, ...unread.map((thread) => thread.account_id)].filter(Boolean)),
  )
  const viewKey = () =>
    threadListViewKey(ui$.selectedAccount.peek(), ui$.selectedFolder.peek(), ui$.query.peek(), ui$.filterMode.peek())
  const previousView = viewKey()
  const previousMessages = mail$.messages.get()
  // What a failure puts back: only the rows and folder counts cleared here, by
  // id, so a view or conversation opened while the write was pending keeps its
  // own state.
  const clearedThreads = new Map(unread.map((thread) => [thread.id, thread]))
  const clearedMessages = new Map(
    previousMessages.filter((message) => message.unread).map((message) => [message.id, message]),
  )
  const clearedFolders = new Map<string, { accountId: string; folderId: string; previous: number }>()
  const noteFolder = (accountId: string, folderId: string | undefined) => {
    if (!folderId) return
    const resolved = mail$.foldersByAccount[accountId]
      .get()
      ?.find((candidate) => folderMatches(candidate, accountId, folderId))
    if (!resolved) return
    const key = `${accountId}\n${resolved.id}`
    if (!clearedFolders.has(key))
      clearedFolders.set(key, { accountId, folderId: resolved.id, previous: resolved.unread })
  }

  // Optimistic clear for currently loaded rows, and for the side navigation
  // badges they sum into — those read the folder cache, so leaving them to the
  // refresh below left the nav counts stale until the server answered. A mail
  // account's folder goes to zero (its write is folder-wide); a folder marked
  // item by item loses only the items marked here.
  mail$.threads.set(threads.map((thread) => (thread.unread ? { ...thread, unread: false, unread_count: 0 } : thread)))
  mail$.messages.set(previousMessages.map((message) => (message.unread ? { ...message, unread: false } : message)))
  for (const accountId of mailAccountIds) {
    const accountFolders = mail$.foldersByAccount[accountId].get()
    const folderId =
      selectedAcc === 'unified' ? accountFolderForRole(accountFolders, folder as UnifiedFolderRole) : folder
    const resolved = accountFolders?.find((candidate) => folderMatches(candidate, accountId, folderId))
    noteFolder(accountId, resolved?.id)
    if (resolved) updateCachedFolderUnread(accountId, resolved.id, 0)
  }
  for (const thread of itemUnread) {
    noteFolder(thread.account_id, thread.folder_id)
    decrementFolderUnread(thread.account_id, thread.folder_id, 1)
  }

  const results = await Promise.allSettled([
    ...(selectedAcc === 'unified' && mailAccountIds.length > 0 ? ['unified'] : mailAccountIds).map(
      async (accountId) => {
        const result = await invoke<MutationResult & { ok?: boolean; failures?: Array<{ message: string }> }>(
          'mail.markAllRead',
          { account_id: accountId, folder_id: folder },
        )
        // A unified write that failed for some accounts still reports the ones
        // it did mark, so take those counts before treating it as a failure.
        applyMutationFolderUnreads(result)
        if (result?.ok === false || result?.failures?.length) {
          throw new Error(
            result.failures?.map((failure) => failure.message).join('; ') || t('notification.markReadFailed'),
          )
        }
      },
    ),
    ...itemUnread.map((thread) => invoke('mail.markRead', { thread_id: thread.thread_id })),
  ])
  const failure = results.find((result) => result.status === 'rejected')
  if (failure?.status === 'rejected') {
    console.error('markAllRead failed:', failure.reason)
    mail$.threads.set(
      mail$.threads.get().map((thread) => {
        const before = clearedThreads.get(thread.id)
        return before ? { ...thread, unread: before.unread, unread_count: before.unread_count } : thread
      }),
    )
    mail$.messages.set(
      mail$.messages.get().map((message) => (clearedMessages.has(message.id) ? { ...message, unread: true } : message)),
    )
    for (const target of clearedFolders.values()) {
      updateCachedFolderUnread(target.accountId, target.folderId, target.previous, 'local')
    }
    showToast(failure.reason instanceof Error ? failure.reason.message : t('notification.markReadFailed'), 'error')
    // Some writes may have landed; reload so the rows show what the server has.
    // The folder refresh below reconciles the badges either way.
    if (viewKey() === previousView) void loadThreads(false)
  }

  // The visible list can span multiple accounts (unified inbox, Starred, a
  // Kanban board), so refresh each affected account's folder cache — not just
  // the selected view — to keep every side navigation badge in sync. These are
  // cache-only: the active folder list is republished from the cache only if
  // the user is still on this account, so a view opened meanwhile keeps its own.
  await Promise.all(affectedAccountIds.map((accountId) => refreshAccountFoldersCache(accountId, false)))
  if (selectedAcc) publishCachedFolders(selectedAcc)
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
