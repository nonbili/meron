import { observable } from '@legendapp/state'
import type { Folder, Message } from '../types'
import { invoke } from '../lib/bridge'
import { confirmAction, ui$, showToast, showUndoToast } from './ui'
import { accounts$, unifiedAccounts } from './accounts'
import { kanban$ } from './kanban'
import { filterThreads, markThreadsReadRemote } from '../lib/threadActions'
import { mergeStarredItems } from '../lib/starredItems'
import { isLocalSendId, discardPendingSend } from './pendingSends'

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
  readThreads: {} as Record<string, boolean>,
})

const THREAD_PAGE_SIZE = 30

function updateKanbanThread(threadId: string, update: (thread: Message) => Message) {
  const columns = kanban$.threads.get()
  let changed = false
  const nextColumns = Object.fromEntries(
    Object.entries(columns).map(([key, threads]) => {
      let columnChanged = false
      const nextThreads = threads.map((thread) => {
        if (thread.thread_id !== threadId) return thread
        columnChanged = true
        return update(thread)
      })
      if (columnChanged) changed = true
      return [key, columnChanged ? nextThreads : threads]
    }),
  )
  if (changed) kanban$.threads.set(nextColumns)
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

function findLocalThread(threadId: string): Message | undefined {
  const thread = mail$.threads.get().find((item) => item.thread_id === threadId)
  if (thread) return thread
  for (const threads of Object.values(kanban$.threads.get())) {
    const match = threads.find((item) => item.thread_id === threadId)
    if (match) return match
  }
  return mail$.messages.get().find((item) => item.thread_id === threadId)
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
    if (res?.saved) showToast(`Saved ${att.filename}`)
  } catch {
    showToast(`Couldn't save ${att.filename}`)
  }
}

// Copy a keyed image onto the system clipboard. The webview's native "Copy
// Image" is inert in the Wails webview, so the bridge shells out to the same
// clipboard helpers used for pasting.
export async function copyAttachmentImage(att: { key: string | null }) {
  if (!att.key) return
  try {
    await invoke('mail.copyImage', { key: att.key })
    showToast('Image copied')
  } catch {
    showToast("Couldn't copy image")
  }
}

// The visible thread list after applying the active filter (all / unread / starred).
export function getFilteredThreads() {
  const threads = mail$.threads.get()
  // The starred view is already a flat list of starred items only; a leftover
  // filter mode from the previous mailbox must not hide rows here.
  if (ui$.selectedAccount.get() === 'starred') return threads
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
  if (ui$.selectedAccount.get() === 'starred') {
    const selectedItem = ui$.selectedStarredItem.get()
    const current = list.findIndex((thread) => thread.id === selectedItem)
    const next = current === -1 ? 0 : Math.min(list.length - 1, Math.max(0, current + delta))
    const target = list[next]
    if (target) {
      ui$.selectedStarredItem.set(target.id)
      ui$.selectedThread.set(target.thread_id)
    }
    return
  }
  const selected = ui$.selectedThread.get()
  const current = list.findIndex((thread) => thread.thread_id === selected)
  const next = current === -1 ? 0 : Math.min(list.length - 1, Math.max(0, current + delta))
  const target = list[next]
  if (target) ui$.selectedThread.set(target.thread_id)
}

export function getActiveThread() {
  const filtered = getFilteredThreads()
  const threads = mail$.threads.get()
  if (ui$.selectedAccount.get() === 'starred') {
    const selectedItem = ui$.selectedStarredItem.get()
    const fromItem = selectedItem ? filtered.find((thread) => thread.id === selectedItem) : null
    if (fromItem) return fromItem
  }
  const selected = ui$.selectedThread.get()
  if (!selected) {
    return filtered[0] ?? null
  }
  const fromList = filtered.find((thread) => thread.thread_id === selected)
  if (fromList) return fromList
  const fromAllThreads = threads.find((thread) => thread.thread_id === selected)
  if (fromAllThreads) return fromAllThreads
  const kanbanColumns = kanban$.threads.get()
  for (const threads of Object.values(kanbanColumns)) {
    const match = threads.find((thread) => thread.thread_id === selected)
    if (match) return match
  }
  return filtered[0] ?? null
}

export async function loadFolders(accountId: string, refresh = true) {
  if (accountId === 'starred') {
    mail$.folders.set([{ id: 'inbox', account_id: 'starred', name: 'Starred', role: 'inbox', unread: 0 }])
    return
  }
  if (accountId === 'unified') {
    const accounts = unifiedAccounts()
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

    mail$.folders.set([{ id: 'inbox', account_id: 'unified', name: 'Inbox', role: 'inbox', unread: totalUnread }])
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
  } catch {
    return []
  }
}

/** Unread count of the INBOX folder in a folder list, or 0 if absent. */
export function inboxUnread(folders: Folder[] | undefined): number {
  if (!folders) return 0
  const inbox = folders.find((f) => f.role === 'inbox' || f.id.toLowerCase() === 'inbox')
  return inbox?.unread ?? 0
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
  options: { refreshIfBootstrapOnly?: boolean; waitForRefresh?: boolean } = {},
): Promise<Folder[]> {
  if (!accountId || accountId === 'unified') return []
  const cached = mail$.foldersByAccount[accountId].get()
  if (cached && cached.length > 0) {
    if (options.refreshIfBootstrapOnly && hasOnlyBootstrapInbox(cached)) {
      void refreshAccountFoldersCache(accountId, true)
    }
    return cached
  }
  try {
    const result = await invoke<{ folders: Folder[] }>('mail.folderList', {
      account_id: accountId,
      refresh: options.refreshIfBootstrapOnly,
    })
    const folders = result.folders || []
    mail$.foldersByAccount[accountId].set(folders)
    if (options.waitForRefresh && options.refreshIfBootstrapOnly && folders.length === 0) {
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

export async function loadThreads(refresh = true) {
  const selectedAcc = ui$.selectedAccount.get()
  const selectedFol = ui$.selectedFolder.get()
  const q = ui$.query.get()
  const filter = ui$.filterMode.get()
  const previousThreads = mail$.threads.get()
  const currentSelected = ui$.selectedThread.get()
  const currentSelectedItem = ui$.selectedStarredItem.get()
  const previousThreadsCursor = mail$.threadsCursor.get()
  const previousAccountCursors = mail$.threadAccountCursors.get()

  // A background refresh (a sync, not a user-initiated account/folder/query/filter
  // change) only re-fetches the first page of the thread list. If the user has
  // scrolled the list and loaded extra pages, replacing the whole array with just
  // the first page collapses it and resets the scroll position. In that case we
  // merge the fresh page into the list we already have instead.
  const mergeBackground = !refresh && previousThreads.length > 0

  // The starred view is a flat cross-account list of individual starred items
  // (mail messages + feed items), fetched in one un-paginated call.
  if (selectedAcc === 'starred') {
    try {
      const res = await invoke<{ items: Message[] }>('mail.starredItems', {})
      const items = mergeStarredItems(res.items ?? [], q)
      mail$.threads.set(items)
      // Keep the conversation pane coherent: getActiveThread falls back to the
      // first visible row, so the selection must point at a listed item.
      if (!kanban$.activeBoardId.get() && !items.some((item) => item.id === currentSelectedItem)) {
        const nextItem = items.find((item) => item.thread_id === currentSelected) ?? items[0]
        ui$.selectedStarredItem.set(nextItem?.id ?? '')
        ui$.selectedThread.set(nextItem?.thread_id ?? '')
      }
    } catch (err) {
      console.error('Failed to load starred items:', err)
    }
    mail$.threadsCursor.set('')
    mail$.threadAccountCursors.set({})
    return
  }

  let allThreads: Message[] = []

  if (selectedAcc === 'unified') {
    const accounts = unifiedAccounts()
    const nextCursors: Record<string, string> = {}
    const fetchPromises = accounts.map(async (acc) => {
      try {
        const res = await invoke<{ threads: Message[]; next_cursor?: string }>('mail.threadList', {
          account_id: acc.id,
          folder_id: 'inbox',
          query: q,
          filter,
          refresh,
        })
        if (res.next_cursor) nextCursors[acc.id] = res.next_cursor
        return res.threads || []
      } catch (err) {
        console.error(`Failed to load threads for ${acc.email}:`, err)
        return []
      }
    })
    const results = await Promise.all(fetchPromises)
    allThreads = results.flat()
    allThreads.sort((a, b) => b.date - a.date)
    mail$.threadAccountCursors.set(nextCursors)
    mail$.threadsCursor.set(Object.keys(nextCursors).length > 0 ? 'unified' : '')
  } else {
    try {
      const result = await invoke<{ threads: Message[]; next_cursor?: string }>('mail.threadList', {
        account_id: selectedAcc,
        folder_id: selectedFol,
        query: q,
        filter,
        refresh,
      })
      allThreads = result.threads || []
      mail$.threadsCursor.set(result.next_cursor ?? '')
      mail$.threadAccountCursors.set({})
    } catch (err) {
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

  const filtered = getFilteredThreads()
  // In kanban view the open conversation is owned by kanban$.paneThreadId, not by
  // mail$.threads/filtered. Clicking a card sets selectedFolder (firing this load)
  // and selectedThread together; auto-selecting or snapping here would yank
  // selectedThread to an unrelated normal-view thread while the pane stays open on
  // the card — rendering the wrong conversation. So leave the selection alone.
  if (kanban$.activeBoardId.get()) {
    return
  }
  if (!currentSelected) {
    ui$.selectedThread.set(filtered[0]?.thread_id ?? '')
  } else if (
    // Only snap the selection on a user-initiated load (account/folder/query/
    // filter change). A background refresh (refresh === false, e.g. a sync) only
    // re-fetches the first page of the thread list, so an open thread the user
    // scrolled down to and opened from a later page — or whose messages are still
    // in flight — would look "missing" and get yanked back to the top thread a
    // second or two later. Deliberate clear-then-reselect flows (delete/move)
    // reset selectedThread to "" first, so they fall into the branch above.
    refresh &&
    !allThreads.some((thread) => thread.thread_id === currentSelected) &&
    !mail$.messages.get().some((message) => message.thread_id === currentSelected)
  ) {
    ui$.selectedThread.set(filtered[0]?.thread_id ?? '')
  }
}

export async function loadMoreThreads() {
  if (mail$.threadsLoadingMore.get()) return
  const selectedAcc = ui$.selectedAccount.get()
  const selectedFol = ui$.selectedFolder.get()
  const q = ui$.query.get()
  const filter = ui$.filterMode.get()
  if (q.trim() || filter === 'starred') return

  mail$.threadsLoadingMore.set(true)
  try {
    let moreThreads: Message[] = []
    if (selectedAcc === 'unified') {
      const cursors = mail$.threadAccountCursors.get()
      const accounts = unifiedAccounts().filter((account) => !!cursors[account.id])
      if (accounts.length === 0) return
      const nextCursors: Record<string, string> = {}
      const results = await Promise.all(
        accounts.map(async (acc) => {
          try {
            const res = await invoke<{ threads: Message[]; next_cursor?: string }>('mail.threadList', {
              account_id: acc.id,
              folder_id: 'inbox',
              query: q,
              filter,
              before_cursor: cursors[acc.id],
              refresh: false,
            })
            if (res.next_cursor) nextCursors[acc.id] = res.next_cursor
            return res.threads || []
          } catch (err) {
            console.error(`Failed to load more threads for ${acc.email}:`, err)
            return []
          }
        }),
      )
      moreThreads = results.flat()
      mail$.threadAccountCursors.set(nextCursors)
      mail$.threadsCursor.set(Object.keys(nextCursors).length > 0 ? 'unified' : '')
    } else {
      const cursor = mail$.threadsCursor.get()
      if (!cursor) return
      const res = await invoke<{ threads: Message[]; next_cursor?: string }>('mail.threadList', {
        account_id: selectedAcc,
        folder_id: selectedFol,
        query: q,
        filter,
        before_cursor: cursor,
        refresh: false,
      })
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
  try {
    const result = await invoke<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
      thread_id: threadId,
      limit: THREAD_PAGE_SIZE,
    })
    // Guard against a stale response: the user may have switched threads while
    // this was in flight (e.g. during the ancestor fetch). Don't overwrite the
    // newer thread's messages — and let that newer load own the loading flag.
    if (ui$.selectedThread.get() !== threadId) return
    mail$.messages.set(result.messages)
    mail$.messagesCursor.set(result.next_cursor ?? '')
    mail$.messagesLoadingMore.set(false)
  } finally {
    if (ui$.selectedThread.get() === threadId) {
      mail$.threadLoading.set(false)
    }
  }
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
      limit: THREAD_PAGE_SIZE,
      before_cursor: cursor,
    })
    if (ui$.selectedThread.get() !== threadId) return
    // Prepend the older page; engine returns ascending order within the page.
    const existing = mail$.messages.get()
    const seen = new Set(existing.map((m) => m.id))
    const merged = [...result.messages.filter((m) => !seen.has(m.id)), ...existing]
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
  const previousKanbanThreads = kanban$.threads.get()
  const hasUnread =
    previousThreads.some((thread) => thread.thread_id === threadId && thread.unread) ||
    previousMessages.some((message) => message.thread_id === threadId && message.unread) ||
    Object.values(previousKanbanThreads).some((threads) =>
      threads.some((thread) => thread.thread_id === threadId && thread.unread),
    )
  if (!hasUnread) return

  mail$.readThreads[threadId].set(true)
  mail$.threads.set(
    previousThreads.map((thread) =>
      thread.thread_id === threadId ? { ...thread, unread: false, unread_count: 0 } : thread,
    ),
  )
  mail$.messages.set(
    previousMessages.map((message) => (message.thread_id === threadId ? { ...message, unread: false } : message)),
  )
  updateKanbanThread(threadId, (thread) => ({ ...thread, unread: false, unread_count: 0 }))

  try {
    await invoke('mail.markRead', { thread_id: threadId })
  } catch (error) {
    mail$.readThreads[threadId].set(undefined)
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    kanban$.threads.set(previousKanbanThreads)
    throw error
  } finally {
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) {
      void loadFolders(selectedAcc, false)
    }
  }
}

export async function markThreadUnread(threadId: string) {
  if (!threadId) return
  const alreadyAllUnread =
    !mail$.threads.get().some((thread) => thread.thread_id === threadId && !thread.unread) &&
    !mail$.messages.get().some((message) => message.thread_id === threadId && !message.unread) &&
    !Object.values(kanban$.threads.get()).some((threads) =>
      threads.some((thread) => thread.thread_id === threadId && !thread.unread),
    )
  if (alreadyAllUnread) return

  mail$.readThreads[threadId].set(undefined)

  mail$.threads.set(
    mail$.threads
      .get()
      .map((thread) =>
        thread.thread_id === threadId
          ? { ...thread, unread: true, unread_count: Math.max(1, thread.unread_count ?? 0) }
          : thread,
      ),
  )
  mail$.messages.set(
    mail$.messages.get().map((message) => (message.thread_id === threadId ? { ...message, unread: true } : message)),
  )
  updateKanbanThread(threadId, (thread) => ({
    ...thread,
    unread: true,
    unread_count: Math.max(1, thread.unread_count ?? 0),
  }))

  await invoke('mail.markRead', { thread_id: threadId, seen: false })

  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) {
    void loadFolders(selectedAcc, false)
  }
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

  await invoke('mail.markStarred', { thread_id: threadId, starred })
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
  const previousKanbanThreads = kanban$.threads.get()
  const previousSelected = ui$.selectedThread.get()
  const previousPaneThreadId = kanban$.paneThreadId.get()
  const previousStarredItem = ui$.selectedStarredItem.get()
  const inStarredView = ui$.selectedAccount.get() === 'starred'
  // When the selected thread is the one leaving, advance to its neighbour in the
  // visible list (next, or previous if it was last) instead of snapping back to
  // the top — this keeps keyboard triage (e/# on the j/k selection) in place.
  // Computed before mutating mail$.threads, since getFilteredThreads reads it.
  let nextSelected = previousSelected
  let nextStarredItem = previousStarredItem
  if (previousSelected === threadId) {
    if (kanban$.activeBoardId.get()) {
      nextSelected = kanbanNeighbourThreadId(threadId)
    } else {
      const visible = getFilteredThreads()
      const index = visible.findIndex((thread) => thread.thread_id === threadId)
      // Skip over rows of the deleted thread itself — the starred view is a
      // flat item list that can hold several rows with the same thread_id.
      const neighbour =
        index === -1
          ? undefined
          : (visible.slice(index + 1).find((item) => item.thread_id !== threadId) ??
            visible
              .slice(0, index)
              .reverse()
              .find((item) => item.thread_id !== threadId))
      nextSelected = neighbour?.thread_id ?? ''
      if (inStarredView) nextStarredItem = neighbour?.id ?? ''
    }
  }
  const nextThreads = previousThreads.filter((thread) => thread.thread_id !== threadId)

  mail$.threads.set(nextThreads)
  mail$.messages.set(previousMessages.filter((message) => message.thread_id !== threadId))
  removeKanbanThread(threadId)
  if (previousSelected === threadId) {
    ui$.selectedThread.set(nextSelected)
    if (inStarredView) ui$.selectedStarredItem.set(nextStarredItem)
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
      kanban$.threads.set(previousKanbanThreads)
      ui$.selectedThread.set(previousSelected)
      ui$.selectedStarredItem.set(previousStarredItem)
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
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (threadStillListed(threadId)) {
      showToast('Move failed: thread is still in this folder', 'error')
    } else if (options.undo !== false && sourceFolder) {
      showUndoToast('Thread moved', () => void moveThreadToFolder(targetThreadId, sourceFolder, { undo: false }))
    } else {
      showToast('Thread moved')
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : 'Move failed', 'error')
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
    showToast('Thread copied')
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Copy failed', 'error')
  }
}

export async function archiveThread(threadId: string) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  const { rollback } = removeThreadLocally(threadId)
  try {
    const res = await invoke<{ folder?: string; thread_id?: string }>('mail.archive', { thread_id: threadId })
    assertMoveAffected(res, 'Archive')
    const archivedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.folder)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (threadStillListed(threadId)) {
      showToast('Archive failed: thread is still in this folder', 'error')
    } else if (sourceFolder) {
      // Offer to move it back where it came from. Falls back to a plain toast
      // when the origin folder is unknown (nothing reliable to restore to).
      showUndoToast('Thread archived', () => void moveThreadToFolder(archivedThreadId, sourceFolder, { undo: false }))
    } else {
      showToast('Thread archived')
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : 'Archive failed', 'error')
  }
}

export async function deleteThread(threadId: string, options: { permanent?: boolean } = {}) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  // Drafts are expunged in place by the engine (never moved to Trash), so the
  // delete is permanent there too — but worded as a discard.
  const isDraft = isDraftFolder(sourceFolder)
  const permanent = options.permanent ?? (isDraft || isTrashFolderId(sourceThread?.account_id ?? '', sourceFolder))
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
    const res = await invoke<{ deleted?: number; permanent?: boolean; trash?: string; thread_id?: string }>(
      'mail.delete',
      {
        thread_id: threadId,
        ...(sourceFolder ? { folder: sourceFolder } : {}),
      },
    )
    assertDeleteAffected(res)
    const trashedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.trash)
    const canUndoTrashMove = !!(res.thread_id || res.trash)
    await refreshThreadLocation(undefined, true)
    if (threadStillListed(threadId)) {
      showToast('Delete failed: thread is still in this folder', 'error')
    } else if (!isDraft && !permanent && !res.permanent && sourceFolder && canUndoTrashMove) {
      showUndoToast(
        'Thread moved to Trash',
        () => void moveThreadToFolder(trashedThreadId, sourceFolder, { undo: false }),
      )
    } else {
      showToast(isDraft ? 'Draft discarded' : permanent ? 'Thread deleted' : 'Thread moved to Trash')
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : 'Delete failed', 'error')
  }
}

// True when `folderId` names an IMAP Drafts mailbox. Mirrors the core's
// `looks_like_drafts` so the UI can word the delete as a permanent discard —
// the engine expunges drafts in place rather than moving them to Trash.
export function isDraftFolder(folderId: string): boolean {
  return ['drafts', 'draft', 'inbox.drafts', 'inbox.draft', '[gmail]/drafts', '[gmail]/draft'].includes(
    folderId.toLowerCase(),
  )
}

export async function discardSavedDraftCopy(draft: {
  threadId: string
  messageId: string
  folderId: string
  accountId?: string
  draftMessageId?: string
}) {
  if (!draft.accountId && !draft.threadId) return

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const nextMessages = draft.messageId
    ? previousMessages.filter((message) => message.id !== draft.messageId)
    : previousMessages
  const selectedThread = ui$.selectedThread.get()
  const removeThread = !!draft.threadId && !nextMessages.some((message) => message.thread_id === draft.threadId)

  mail$.messages.set(nextMessages)
  if (removeThread) {
    mail$.threads.set(previousThreads.filter((thread) => thread.thread_id !== draft.threadId))
    removeKanbanThread(draft.threadId)
    if (selectedThread === draft.threadId) ui$.selectedThread.set('')
  }

  try {
    if (draft.accountId && draft.draftMessageId) {
      await invoke('mail.discardDraft', {
        account_id: draft.accountId,
        draft_id: draft.draftMessageId,
      })
    } else {
      const res = await invoke('mail.delete', {
        thread_id: draft.threadId,
        message_ids: [draft.messageId],
        folder: draft.folderId,
      })
      assertDeleteAffected(res)
    }
    await loadThreads(false)
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) void loadFolders(selectedAcc, false)
    if (draft.accountId && draft.accountId !== selectedAcc) void loadFolders(draft.accountId, false)
  } catch (error) {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    showToast(
      error instanceof Error
        ? `Sent, but couldn't discard draft: ${error.message}`
        : "Sent, but couldn't discard draft",
      'error',
    )
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

  const isDraft = isDraftFolder(message.folder_id)
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

  mail$.messages.set(nextMessages)

  try {
    const res = await invoke('mail.delete', {
      thread_id: threadId,
      message_ids: [message.id],
      folder: message.folder_id,
    })
    assertDeleteAffected(res)
    showToast(isDraft ? 'Draft discarded' : 'Message moved to Trash')
    // No messages left from this thread: drop the now-empty conversation.
    if (!nextMessages.some((item) => item.thread_id === threadId)) {
      if (ui$.selectedThread.get() === threadId) {
        ui$.selectedThread.set('')
      }
      removeKanbanThread(threadId)
    }
    await loadThreads(false)
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) {
      void loadFolders(selectedAcc, false)
    }
  } catch (error) {
    mail$.messages.set(previousMessages)
    showToast(error instanceof Error ? error.message : 'Delete failed', 'error')
  }
}

// Mark every visible thread as read. Mail accounts are marked folder-wide in one
// call each; RSS feeds (no folder-wide flag) are marked per-thread.
export async function markAllRead() {
  const threads = mail$.threads.get()
  const unread = threads.filter((thread) => thread.unread)
  if (unread.length === 0) return

  const accounts = accounts$.get()
  const selectedAcc = ui$.selectedAccount.get()
  const folder = selectedAcc === 'unified' ? 'inbox' : ui$.selectedFolder.get()

  // Optimistic clear.
  mail$.threads.set(threads.map((thread) => (thread.unread ? { ...thread, unread: false, unread_count: 0 } : thread)))
  mail$.messages.set(mail$.messages.get().map((message) => (message.unread ? { ...message, unread: false } : message)))

  await markThreadsReadRemote(unread, accounts, folder)

  if (selectedAcc) {
    void loadFolders(selectedAcc, false)
  }
}

export async function markMessagesRead(threadId: string, messageIds: string[]) {
  const uniqueIds = Array.from(new Set(messageIds.filter(Boolean)))
  if (!threadId || uniqueIds.length === 0) return

  const unreadIds = new Set(
    mail$.messages
      .get()
      .filter((message) => message.thread_id === threadId && message.unread && uniqueIds.includes(message.id))
      .map((message) => message.id),
  )
  if (unreadIds.size === 0) return

  mail$.messages.set(
    mail$.messages.get().map((message) => (unreadIds.has(message.id) ? { ...message, unread: false } : message)),
  )
  mail$.threads.set(
    mail$.threads.get().map((thread) => {
      if (thread.thread_id !== threadId) return thread
      const unreadCount = Math.max(0, (thread.unread_count ?? (thread.unread ? 1 : 0)) - unreadIds.size)
      return { ...thread, unread: unreadCount > 0, unread_count: unreadCount }
    }),
  )

  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) {
    void loadFolders(selectedAcc, false)
  }
  await invoke('mail.markRead', { thread_id: threadId, message_ids: Array.from(unreadIds) })
}

export async function markMessageReadState(message: Message, seen: boolean) {
  if (!message?.id || !message.thread_id) return
  if (message.unread === !seen) return

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousKanbanThreads = kanban$.threads.get()
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
    await invoke('mail.markRead', { thread_id: message.thread_id, message_ids: [message.id], seen })
  } catch (error) {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    kanban$.threads.set(previousKanbanThreads)
    throw error
  } finally {
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) {
      void loadFolders(selectedAcc, false)
    }
  }
}

export async function starMessage(message: Message, starred: boolean) {
  if (!message?.id || !message.thread_id) return

  const nextMessages = mail$.messages.get().map((item) => (item.id === message.id ? { ...item, starred } : item))
  const threadStarred = nextMessages.some((item) => item.thread_id === message.thread_id && item.starred)

  mail$.messages.set(nextMessages)
  // In the starred view, threads holds per-message rows: insert/remove the one
  // row by id instead of stamping the thread-level star onto every sibling.
  if (ui$.selectedAccount.peek() === 'starred') {
    const rows = mail$.threads.get()
    if (!starred) {
      mail$.threads.set(rows.filter((item) => item.id !== message.id))
    } else if (rows.some((item) => item.id === message.id)) {
      mail$.threads.set(rows.map((item) => (item.id === message.id ? { ...item, starred } : item)))
    } else {
      // Starred from the open conversation: the row isn't listed yet, insert it
      // at its sorted position.
      mail$.threads.set(mergeStarredItems([...rows, { ...message, starred }], ui$.query.peek()))
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
    showToast('Synced')
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Sync failed', 'error')
  } finally {
    ui$.busy.set(false)
  }
}
