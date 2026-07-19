import { invoke } from './bridge'
import { unifiedAccounts } from '../states/accounts'
import { getAllKanbanColumns, kanbanColumnKey, kanban$, type KanbanColumn } from '../states/kanban'
import { mail$, updateCachedFolderUnread } from '../states/mail'
import { showToast } from '../states/ui'
import { mergeStarredItems } from './starredItems'
import type { FilterMode } from '../states/ui'
import type { Account, Folder, Message } from '../types'
import type { ThreadContextAction, ThreadContextActionDetail } from '../components/threads/ThreadContextMenu'

export const COLUMN_LIMIT = 50
export const KANBAN_COLUMN_MINIMIZED_WIDTH = 48
export const SEARCH_DEBOUNCE_MS = 300

const columnLoadVersions = new Map<string, number>()

export function searchScopeColumn(scope: string): KanbanColumn | null {
  if (!scope || scope === 'all') return null
  const [accountId, folderId] = scope.split('\n')
  if (!accountId || !folderId) return null
  return { accountId, folderId }
}

export function searchTargets(columns: KanbanColumn[], scope: string): KanbanColumn[] {
  const scopedColumn = searchScopeColumn(scope)
  if (!scopedColumn) return columns
  return columns.filter((column) => kanbanColumnKey(column) === kanbanColumnKey(scopedColumn))
}

// The full folder list used for column/scope labels: an account's own folders
// plus every account's folders (so a column can name a folder for an account
// other than the currently selected one).
export function mergeLabelFolders(folders: Folder[], foldersByAccount: Record<string, Folder[]>): Folder[] {
  return [...folders, ...Object.values(foldersByAccount).flat()]
}

export function accountLabel(accountId: string, accounts: { id: string; email: string; display_name: string }[]) {
  if (accountId === 'unified') return 'Unified'
  const account = accounts.find((item) => item.id === accountId)
  return account?.display_name || account?.email || accountId
}

export function isUnifiedStarredColumn(column: KanbanColumn): boolean {
  return column.accountId === 'unified' && column.folderId.toLowerCase() === 'starred'
}

export function folderLabel(column: KanbanColumn, folders: Folder[], accounts: Account[]) {
  const isInbox = column.folderId.toLowerCase() === 'inbox'
  if (isUnifiedStarredColumn(column)) return 'Unified Starred'
  if (column.accountId === 'unified') return 'Unified Inbox'
  if (isInbox) return isRSSAccount(column.accountId, accounts) ? 'Feed' : 'Inbox'
  return (
    folders.find((folder) => folder.account_id === column.accountId && folder.id === column.folderId)?.name ||
    column.folderId
  )
}

export function searchColumnLabel(column: KanbanColumn, folders: Folder[], accounts: Account[]) {
  if (isUnifiedStarredColumn(column)) return 'Unified Starred'
  if (column.accountId === 'unified') return 'Unified Inbox'
  return `${accountLabel(column.accountId, accounts)} / ${folderLabel(column, folders, accounts)}`
}

// Whether a column is in the active search's scope ("all" or this column's key).
export function columnSearchActive(key: string, searchQuery: string, searchScope: string): boolean {
  return !!searchQuery.trim() && (searchScope === 'all' || searchScope === key)
}

// Over a board wallpaper the usual translucent dark surface lets the image
// bleed through the column, so it swaps to a more opaque, blurred one.
export function columnSearchHighlightClass(active: boolean, overWallpaper = false): string {
  if (active) return 'border-accent/70 ring-2 ring-accent/20 bg-chats dark:bg-black/35'
  return overWallpaper
    ? 'border-border bg-raised/95 backdrop-blur-sm dark:bg-black/45'
    : 'border-border bg-raised dark:bg-black/20'
}

export function columnDropTargetClass(isOver: boolean): string {
  return isOver ? 'border-accent bg-accent/10 ring-2 ring-accent/35 dark:bg-accent/15' : ''
}

// Placeholder text for an empty column, branching on the active filter and
// whether a search hid otherwise-present threads.
export function columnEmptyText(
  filterMode: string,
  searchActive: boolean,
  hasRawThreads: boolean,
  isRss = false,
): string {
  // RSS columns list feed subscriptions rather than mail threads, so the noun
  // shifts to match the user's mental model of the column.
  const noun = isRss ? 'feeds' : 'threads'
  if (searchActive) {
    if (filterMode === 'unread') return hasRawThreads ? 'Matches hidden by Unread filter' : 'No unread matches'
    if (filterMode === 'starred') return hasRawThreads ? 'Matches hidden by Starred filter' : 'No starred matches'
    return 'No matches'
  }
  if (filterMode === 'unread') return `No unread ${noun}`
  if (filterMode === 'starred') return `No starred ${noun}`
  return `No ${noun}`
}

// The sidecar reports the canonical IMAP folder ("INBOX", actual name); match it
// against a stored column folderId, treating the inbox role case-insensitively.
export function folderMatches(folderId: string, synced: string | undefined): boolean {
  if (!synced) return false
  const norm = (value: string) => (value.toLowerCase() === 'inbox' ? 'inbox' : value)
  return norm(folderId) === norm(synced)
}

export function isRSSAccount(accountId: string, accounts: { id: string; provider: string; auth_type: string }[]) {
  const account = accounts.find((item) => item.id === accountId)
  return accountId.startsWith('rss-') || account?.provider === 'rss' || account?.auth_type === 'rss'
}

export function loadedUnreadCount(threads: Message[]): number {
  return threads.reduce((count, thread) => count + (thread.unread ? (thread.unread_count ?? 1) : 0), 0)
}

export function kanbanColumnUnreadCount(
  column: KanbanColumn,
  folderUnread: number | undefined,
  loadedThreads: Message[] = [],
): number {
  if (isUnifiedStarredColumn(column)) return loadedUnreadCount(loadedThreads)
  return folderUnread ?? loadedUnreadCount(loadedThreads)
}

type ColumnPage = {
  threads: Message[]
  folderUnread?: number
  folderUnreadByAccount?: Record<string, number>
  // Next-page cursor for a single-account column ("" = no more).
  nextSingle: string
  // Next-page cursors per account for a unified column (absent = that account
  // is exhausted).
  nextUnified: Record<string, string>
}

export function activeKanbanColumnFilter(column: KanbanColumn): FilterMode {
  return kanban$.filters[kanbanColumnKey(column)].peek() ?? kanban$.globalFilter.peek()
}

// Fetch one page of a column's threads. `before` carries the cursors from the
// previous page; omit it for the first page. Unified columns page each account's
// inbox independently and only re-request accounts that still have a cursor.
async function fetchColumnThreads(
  column: KanbanColumn,
  refresh = false,
  query = '',
  before?: { single?: string; unified?: Record<string, string> },
): Promise<ColumnPage> {
  const trimmedQuery = query.trim()
  const filter = activeKanbanColumnFilter(column)
  if (isUnifiedStarredColumn(column)) {
    const result = await invoke<{ items: Message[] }>('mail.starredItems', {})
    return {
      threads: mergeStarredItems(result.items ?? [], trimmedQuery).slice(0, COLUMN_LIMIT),
      folderUnread: undefined,
      folderUnreadByAccount: undefined,
      nextSingle: '',
      nextUnified: {},
    }
  }
  if (column.accountId === 'unified') {
    const beforeUnified = before?.unified
    const accounts = beforeUnified
      ? unifiedAccounts().filter((account) => !!beforeUnified[account.id])
      : unifiedAccounts()
    const nextUnified: Record<string, string> = {}
    const results = await Promise.all(
      accounts.map((account) =>
        invoke<{ threads: Message[]; next_cursor?: string; folder_unread?: number }>('mail.threadList', {
          account_id: account.id,
          folder_id: 'inbox',
          query: trimmedQuery,
          filter,
          refresh,
          limit: COLUMN_LIMIT,
          before_cursor: beforeUnified?.[account.id],
        })
          .then((result) => {
            if (result.next_cursor) nextUnified[account.id] = result.next_cursor
            return result
          })
          .catch(() => ({ threads: [] as Message[] })),
      ),
    )
    const threads = results.flatMap((result) => result.threads || []).sort((a, b) => b.date - a.date)
    const folderUnread = results.every((result) => typeof result.folder_unread === 'number')
      ? results.reduce((sum, result) => sum + (result.folder_unread ?? 0), 0)
      : undefined
    const folderUnreadByAccount = Object.fromEntries(
      results.flatMap((result, index) =>
        typeof result.folder_unread === 'number' ? [[accounts[index].id, result.folder_unread] as const] : [],
      ),
    )
    return { threads, folderUnread, folderUnreadByAccount, nextSingle: '', nextUnified }
  }

  const result = await invoke<{ threads: Message[]; next_cursor?: string; folder_unread?: number }>('mail.threadList', {
    account_id: column.accountId,
    folder_id: column.folderId,
    query: trimmedQuery,
    filter,
    refresh,
    limit: COLUMN_LIMIT,
    before_cursor: before?.single,
  })
  return {
    threads: result.threads || [],
    folderUnread: result.folder_unread,
    folderUnreadByAccount:
      typeof result.folder_unread === 'number' ? { [column.accountId]: result.folder_unread } : undefined,
    nextSingle: result.next_cursor ?? '',
    nextUnified: {},
  }
}

// Reading a thread marks it read on the backend, which fires `mail.synced` and
// reloads the column. Under the Unread filter the now-read thread drops out of
// the fresh fetch, so re-append any thread we just marked read (tracked in
// mail$.readThreads) that was showing before but the server no longer returns —
// the render-time filter (filterThreads) then keeps it pinned in place. Without
// this the just-read card vanishes the instant it's read.
function keepReadThreads(column: KanbanColumn, key: string, fetched: Message[]): Message[] {
  const filter = activeKanbanColumnFilter(column)
  if (filter === 'all') return fetched
  const readThreads = mail$.readThreads.peek()
  if (Object.keys(readThreads).length === 0) return fetched
  const fetchedIds = new Set(fetched.map((thread) => thread.thread_id))
  const kept = (kanban$.threads[key].peek() ?? []).filter(
    (thread) => readThreads[thread.thread_id] && !fetchedIds.has(thread.thread_id),
  )
  if (kept.length === 0) return fetched
  return [...fetched, ...kept].sort((a, b) => b.date - a.date)
}

export async function loadKanbanColumn(column: KanbanColumn, refresh = false, query = '') {
  const key = kanbanColumnKey(column)
  const trimmedQuery = query.trim()
  const version = (columnLoadVersions.get(key) ?? 0) + 1
  columnLoadVersions.set(key, version)
  kanban$.loading[key].set(true)
  try {
    const { threads, folderUnread, folderUnreadByAccount, nextSingle, nextUnified } = await fetchColumnThreads(
      column,
      refresh,
      trimmedQuery,
    )
    if (columnLoadVersions.get(key) !== version) return
    for (const [accountId, unread] of Object.entries(folderUnreadByAccount ?? {})) {
      updateCachedFolderUnread(accountId, column.accountId === 'unified' ? 'inbox' : column.folderId, unread)
    }
    kanban$.threads[key].set(keepReadThreads(column, key, threads))
    if (folderUnread !== undefined) kanban$.unreadCounts[key].set(folderUnread)
    kanban$.cursors[key].set(trimmedQuery ? '' : nextSingle)
    kanban$.accountCursors[key].set(trimmedQuery ? {} : nextUnified)
  } finally {
    if (columnLoadVersions.get(key) === version) {
      kanban$.loading[key].set(false)
    }
  }
}

// Manually pull a single column's folder from the server, then reload it. Unlike
// the chat-view sync (which always targets the selected account's inbox), this is
// column-scoped: it syncs the exact account + folder the column shows. Unified
// columns sync each member account's inbox; the unified starred column has no
// remote folder to pull, so it just reloads.
export async function syncKanbanColumn(column: KanbanColumn) {
  mail$.readThreads.set({})
  try {
    if (isUnifiedStarredColumn(column)) {
      // Aggregated view — nothing to fetch, just refresh from local state.
    } else if (column.accountId === 'unified') {
      await Promise.all(
        unifiedAccounts().map((account) =>
          invoke('mail.sync', { account_id: account.id, folder: 'inbox' }).catch((err) =>
            console.error(`Sync failed for ${account.email}:`, err),
          ),
        ),
      )
    } else {
      await invoke('mail.sync', { account_id: column.accountId, folder: column.folderId })
    }
    await loadKanbanColumn(column, true)
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Sync failed', 'error')
  }
}

function columnHasMore(key: string, unified: boolean): boolean {
  return unified
    ? Object.keys(kanban$.accountCursors[key].get() ?? {}).length > 0
    : !!(kanban$.cursors[key].get() ?? '')
}

// Append the next page of older threads to a column, de-duping by thread id. The
// scroll handler drives this; it no-ops once the cursors are exhausted.
export async function loadMoreKanbanColumn(column: KanbanColumn) {
  const key = kanbanColumnKey(column)
  const unified = column.accountId === 'unified'
  if (isUnifiedStarredColumn(column)) return
  if (kanban$.loadingMore[key].get() || !columnHasMore(key, unified)) return
  kanban$.loadingMore[key].set(true)
  try {
    const { threads, folderUnreadByAccount, nextSingle, nextUnified } = await fetchColumnThreads(column, false, '', {
      single: kanban$.cursors[key].get(),
      unified: kanban$.accountCursors[key].get(),
    })
    const existing = kanban$.threads[key].get() ?? []
    const seen = new Set(existing.map((thread) => thread.thread_id))
    const merged = [...existing, ...threads.filter((thread) => !seen.has(thread.thread_id))]
    for (const [accountId, unread] of Object.entries(folderUnreadByAccount ?? {})) {
      updateCachedFolderUnread(accountId, unified ? 'inbox' : column.folderId, unread)
    }
    if (unified) merged.sort((a, b) => b.date - a.date)
    kanban$.threads[key].set(merged)
    kanban$.cursors[key].set(nextSingle)
    kanban$.accountCursors[key].set(nextUnified)
  } finally {
    kanban$.loadingMore[key].set(false)
  }
}

export async function refreshKanbanContextAction(
  source: KanbanColumn,
  action: ThreadContextAction,
  detail: ThreadContextActionDetail | undefined,
  sourceQuery: string,
) {
  const columns = [source]
  if (action === 'move' && detail?.targetFolderId) {
    const targetAccountId = detail.targetAccountId ?? source.accountId
    for (const column of getAllKanbanColumns()) {
      if (column.accountId !== targetAccountId || column.folderId !== detail.targetFolderId) continue
      if (columns.some((item) => kanbanColumnKey(item) === kanbanColumnKey(column))) continue
      columns.push(column)
    }
  }
  await Promise.all(
    columns.map((column) =>
      loadKanbanColumn(column, false, kanbanColumnKey(column) === kanbanColumnKey(source) ? sourceQuery : ''),
    ),
  )
}
