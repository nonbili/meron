import { useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { invoke } from './bridge'
import { accounts$, unifiedAccounts } from '../states/accounts'
import { getAllKanbanColumns, getKanbanColumns, kanbanColumnKey, kanban$, type KanbanColumn } from '../states/kanban'
import { mail$, updateCachedFolderUnread } from '../states/mail'
import { showToast } from '../states/ui'
import type { FilterMode } from '../states/ui'
import { accountFolderForRole, unifiedFolderLabel, unifiedFolderRole, type UnifiedFolderRole } from './unifiedFolders'
import type { Account, Folder, Message } from '../types'
import type { ThreadContextAction, ThreadContextActionDetail } from '../components/threads/ThreadContextMenu'

export const COLUMN_LIMIT = 50
export const KANBAN_COLUMN_MINIMIZED_WIDTH = 48
export const SEARCH_DEBOUNCE_MS = 300

const columnLoadVersions = new Map<string, number>()
const KANBAN_SYNC_TIMEOUT_MS = 130_000
// A first look at a folder that was never synced answers from an empty cache, so
// the column waits this long for the background sync the read kicked off rather
// than claiming the folder is empty.
const KANBAN_FIRST_SYNC_TIMEOUT_MS = 20_000

type MailSyncEvent = {
  account?: string
  folder?: string
  message?: string
}

function waitForKanbanSync(
  accountIds: string[],
  folderId: string,
  timeoutMs = KANBAN_SYNC_TIMEOUT_MS,
  // A unified column asks each account for its *own* mailbox, so one folder id
  // cannot match every event it is waiting on. When given, any of these counts.
  folderIds?: Set<string>,
) {
  const eventsOn = (window as any).runtime?.EventsOn
  if (typeof eventsOn !== 'function') {
    return {
      promise: Promise.resolve(),
      cancel: () => {},
    }
  }

  const pending = new Set(accountIds)
  let settled = false
  let resolvePromise: () => void
  let rejectPromise: (error: Error) => void
  const promise = new Promise<void>((resolve, reject) => {
    resolvePromise = resolve
    rejectPromise = reject
  })
  const subscriptions: unknown[] = []
  let timeout: number | undefined

  const cleanup = () => {
    if (timeout !== undefined) window.clearTimeout(timeout)
    for (const unsubscribe of subscriptions) {
      if (typeof unsubscribe === 'function') unsubscribe()
    }
  }
  const finish = (error?: Error) => {
    if (settled) return
    settled = true
    cleanup()
    if (error) rejectPromise(error)
    else resolvePromise()
  }
  const folderAccepted = (synced: string) =>
    folderIds ? [...folderIds].some((wanted) => folderMatches(wanted, synced)) : folderMatches(folderId, synced)
  const matches = (detail: MailSyncEvent) =>
    !!detail?.account && pending.has(detail.account) && (!detail.folder || folderAccepted(detail.folder))
  const completeAccount = (detail: MailSyncEvent) => {
    if (!matches(detail)) return
    pending.delete(detail.account!)
    if (pending.size === 0) finish()
  }
  const failAccount = (detail: MailSyncEvent) => {
    if (!detail?.account || !pending.has(detail.account)) return
    // Reject even on a bare budget expiry: the core has already dropped this
    // folder's sync future by the time it emits, so no mail.synced for this
    // mailbox can follow; waiting would only stall until our own
    // timeoutMs. Whether the banner shows the error is a separate question.
    finish(new Error(detail.message || 'Sync failed'))
  }

  subscriptions.push(eventsOn('mail.synced', completeAccount))
  subscriptions.push(eventsOn('mail.newMessages', completeAccount))
  subscriptions.push(eventsOn('mail.syncError', failAccount))
  timeout = window.setTimeout(() => finish(new Error('Sync timed out')), timeoutMs)

  return {
    promise,
    cancel: () => {
      if (settled) return
      settled = true
      cleanup()
    },
  }
}

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

// Read the per-account folder cache in a way memoization can trust. Legend-State
// mutates the raw object in place when a child node is set, so
// `useValue(mail$.foldersByAccount)` keeps handing back the *same* object
// reference after one account's list is replaced. The change does bubble up, so
// the component re-renders — but anything memoized on that reference (the column
// picker's folder tree, the label lookups) keeps its stale value, which is why a
// folder created in the app only showed up after a restart. Every write replaces
// an account's array wholesale, so comparing array identities is enough to know
// when to mint a fresh object.
export function useFoldersByAccount(): Record<string, Folder[]> {
  const byAccount = useValue(mail$.foldersByAccount)
  const snapshot = useRef<Record<string, Folder[]>>({})
  snapshot.current = nextFoldersSnapshot(snapshot.current, byAccount)
  return snapshot.current
}

// The comparison behind `useFoldersByAccount`, split out so it can be tested
// without a DOM renderer: a fresh object when any account's list was replaced (or
// an account came or went), the previous one otherwise so memos still hold.
export function nextFoldersSnapshot(
  previous: Record<string, Folder[]>,
  current: Record<string, Folder[]>,
): Record<string, Folder[]> {
  const keys = Object.keys(current)
  const changed = keys.length !== Object.keys(previous).length || keys.some((key) => previous[key] !== current[key])
  return changed ? { ...current } : previous
}

export function accountLabel(accountId: string, accounts: { id: string; email: string; display_name: string }[]) {
  if (accountId === 'unified') return 'Unified'
  const account = accounts.find((item) => item.id === accountId)
  return account?.display_name || account?.email || accountId
}

export function isUnifiedStarredColumn(column: KanbanColumn): boolean {
  return column.accountId === 'unified' && column.folderId.toLowerCase() === 'starred'
}

export function folderLabel(
  column: KanbanColumn,
  folders: Folder[],
  accounts: Account[],
  t: (key: string) => string = (key) => key,
) {
  const isInbox = column.folderId.toLowerCase() === 'inbox'
  // The unified view's synthetic folders are named after their role alone
  // ("Sent", "Archive") for the side nav, where the selected account already
  // says whose mail it is. Anywhere a column or header stands beside a single
  // account's, the qualified name is the one that reads unambiguously.
  if (column.accountId === 'unified') return unifiedFolderLabel(column.folderId, t)
  if (isInbox) return isRSSAccount(column.accountId, accounts) ? 'Feed' : 'Inbox'
  return (
    folders.find((folder) => folder.account_id === column.accountId && folder.id === column.folderId)?.name ||
    column.folderId
  )
}

export function searchColumnLabel(
  column: KanbanColumn,
  folders: Folder[],
  accounts: Account[],
  t: (key: string) => string = (key) => key,
) {
  if (column.accountId === 'unified') return unifiedFolderLabel(column.folderId, t)
  return `${accountLabel(column.accountId, accounts)} / ${folderLabel(column, folders, accounts)}`
}

export function kanbanColumnMatchesMailEvent(
  column: KanbanColumn,
  accountId: string,
  folderId: string | undefined,
  foldersByAccount: Record<string, Folder[]>,
): boolean {
  if (column.accountId !== 'unified') {
    return column.accountId === accountId && folderMatches(column.folderId, folderId)
  }
  if (isUnifiedStarredColumn(column)) return true
  const role = unifiedFolderRole(column.folderId)
  if (!folderId) return role === 'inbox'
  const accountFolder = accountFolderForRole(foldersByAccount[accountId], role) ?? (role === 'inbox' ? 'inbox' : '')
  return !!accountFolder && folderMatches(accountFolder, folderId)
}

type EventsOn = (name: string, callback: (detail: { account?: string; folder?: string }) => void) => unknown

export function subscribeKanbanMailReloads(eventsOn: EventsOn): () => void {
  // Coalesce reloads. At startup each account is synced separately, so a board
  // showing N accounts gets N mail.synced events back-to-back — and each event
  // would otherwise re-fetch every matching column once. Collect the columns to
  // reload (keyed by column key, latest query wins) and flush them once after a
  // short quiet window so a burst of syncs costs one threadList per column.
  const pending = new Map<string, { column: KanbanColumn; query: string }>()
  let flushTimer: number | undefined
  const flush = () => {
    flushTimer = undefined
    const jobs = [...pending.values()]
    pending.clear()
    for (const job of jobs) {
      void loadKanbanColumn(job.column, false, job.query)
    }
  }
  const reload = (detail: { account?: string; folder?: string }) => {
    const account = detail?.account
    if (!account) return
    const query = kanban$.searchQuery.peek().trim()
    const scope = kanban$.searchScope.peek()
    const folders = mail$.foldersByAccount.peek()
    // Only the open board renders, and its columns are the only ones whose
    // cache anything reads; a board opened later reloads its columns on mount
    // (with a real refresh). Read the active board here rather than closing
    // over the prop so the effect can stay mounted for the session.
    for (const column of getKanbanColumns(kanban$.activeBoardId.peek())) {
      const key = kanbanColumnKey(column)
      const columnQuery = query && (scope === 'all' || scope === key) ? query : ''
      if (kanbanColumnMatchesMailEvent(column, account, detail?.folder, folders)) {
        pending.set(key, { column, query: columnQuery })
      }
    }
    // Leading-window collect: the first event arms the flush; later events in
    // the window just add to the pending set, bounding latency at ~250ms.
    if (pending.size > 0 && flushTimer === undefined) {
      flushTimer = window.setTimeout(flush, 250)
    }
  }
  const offSynced = eventsOn('mail.synced', reload)
  const offNew = eventsOn('mail.newMessages', reload)
  return () => {
    if (flushTimer !== undefined) window.clearTimeout(flushTimer)
    if (typeof offSynced === 'function') offSynced()
    if (typeof offNew === 'function') offNew()
  }
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

// A column a drag can't land in is drawn as refused rather than inviting, so the
// user sees the "no" before letting go; the reason itself is shown on hover and
// repeated as a toast if they drop anyway.
export function columnDropTargetClass(isOver: boolean, blocked = false): string {
  if (blocked) {
    return isOver
      ? 'border-red-400 bg-red-500/10 ring-2 ring-red-400/40 dark:border-red-900/70'
      : 'border-dashed opacity-60'
  }
  return isOver ? 'border-accent bg-accent/10 ring-2 ring-accent/35 dark:bg-accent/15' : ''
}

/**
 * Why a drop was refused, as locale keys — the reason is shown on the column
 * while dragging and toasted if the card is dropped anyway, so it is translated
 * where it is rendered rather than here (this stays pure and testable).
 */
export const KANBAN_DROP_REASONS = {
  unifiedTarget: 'kanban.drop.unifiedTarget',
  unknownOrigin: 'kanban.drop.unknownOrigin',
  feedToMail: 'kanban.drop.feedToMail',
  mailToFeed: 'kanban.drop.mailToFeed',
} as const

/**
 * The locale keys the kanban move path toasts once a drop is under way. Kept
 * beside the refusal keys so both sets are covered by the same catalogue test.
 */
export const KANBAN_MOVE_MESSAGES = {
  moved: 'kanban.move.moved',
  failed: 'kanban.move.failed',
  copyFailed: 'kanban.move.copyFailed',
  noMatchingMessages: 'kanban.move.noMatchingMessages',
  copiedNotRemoved: 'kanban.move.copiedNotRemoved',
  copiedNotRemovedReason: 'kanban.move.copiedNotRemovedReason',
} as const

/**
 * What dropping `thread` from `source` onto `target` would do: the move and the
 * mailbox it leaves from, nothing, or a refusal to explain. A unified column
 * names a role across accounts rather than one mailbox, so a thread dragged out
 * of one takes its origin from the card itself; every other column is its own
 * origin. Pure, so the drop handler and the drag-time column styling agree.
 */
export type KanbanMoveResolution =
  | { kind: 'move'; origin: KanbanColumn }
  | { kind: 'noop' }
  | { kind: 'blocked'; reasonKey: string }

export function resolveKanbanMove(
  source: KanbanColumn,
  target: KanbanColumn,
  thread: Pick<Message, 'account_id' | 'folder_id'> | undefined,
  accounts: { id: string; provider: string; auth_type: string }[],
): KanbanMoveResolution {
  const sameColumn = (a: KanbanColumn, b: KanbanColumn) =>
    a.accountId === b.accountId && folderMatches(a.folderId, b.folderId)
  if (target.accountId === 'unified') return { kind: 'blocked', reasonKey: KANBAN_DROP_REASONS.unifiedTarget }
  if (sameColumn(source, target)) return { kind: 'noop' }
  if (source.accountId === 'unified' && !thread) {
    return { kind: 'blocked', reasonKey: KANBAN_DROP_REASONS.unknownOrigin }
  }
  const origin: KanbanColumn =
    source.accountId === 'unified' && thread ? { accountId: thread.account_id, folderId: thread.folder_id } : source
  // The card's own mailbox can be the column it was dropped on — dragging out of
  // Unified Inbox onto that same account's Inbox column.
  if (sameColumn(origin, target)) return { kind: 'noop' }
  const originRSS = isRSSAccount(origin.accountId, accounts)
  const targetRSS = isRSSAccount(target.accountId, accounts)
  if (originRSS && !targetRSS) return { kind: 'blocked', reasonKey: KANBAN_DROP_REASONS.feedToMail }
  if (!originRSS && targetRSS) return { kind: 'blocked', reasonKey: KANBAN_DROP_REASONS.mailToFeed }
  return { kind: 'move', origin }
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

// Whether an empty first page for this column should wait for the background
// sync the read spawned. Only a single-account IMAP folder gets one: unified and
// starred columns aggregate folders, RSS syncs feeds, and a paused account never
// reports back.
function awaitsFirstSync(column: KanbanColumn): boolean {
  if (column.accountId === 'unified' || isUnifiedStarredColumn(column)) return false
  const account = accounts$.get().find((item) => item.id === column.accountId)
  if (!account || account.paused) return false
  return !isRSSAccount(column.accountId, accounts$.get())
}

type ColumnPage = {
  threads: Message[]
  folderUnread?: number
  folderUnreadByAccount?: Record<string, number>
  /** False when the core has no completed header sync recorded for this folder. */
  folderSynced?: boolean
  // Opaque next-page cursor for either a single-account or unified column.
  nextSingle: string
  // Legacy per-account cursors retained in state while persisted boards migrate.
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
    const result = await invoke<{ items: Message[]; next_cursor?: string }>('mail.starredItems', {
      query: trimmedQuery,
      filter,
      limit: COLUMN_LIMIT,
      before_cursor: before?.single,
    })
    return {
      threads: result.items ?? [],
      folderUnread: undefined,
      folderUnreadByAccount: undefined,
      folderSynced: undefined,
      nextSingle: result.next_cursor ?? '',
      nextUnified: {},
    }
  }
  if (column.accountId === 'unified') {
    // The column names a role, not a folder: the core resolves each account's
    // own Sent/Archive/… and leaves out the ones whose server has none.
    const role = unifiedFolderRole(column.folderId)
    const result = await invoke<{
      threads: Message[]
      next_cursor?: string
      folder_unread?: number
      folder_unreads?: Record<string, number>
    }>('mail.threadList', {
      account_id: 'unified',
      folder_id: role,
      folder_role: role,
      query: trimmedQuery,
      filter,
      refresh,
      limit: COLUMN_LIMIT,
      before_cursor: before?.single,
    })
    return {
      threads: result.threads || [],
      folderUnread: result.folder_unread,
      folderUnreadByAccount: result.folder_unreads,
      folderSynced: undefined,
      nextSingle: result.next_cursor ?? '',
      nextUnified: {},
    }
  }

  const result = await invoke<{
    threads: Message[]
    next_cursor?: string
    folder_unread?: number
    folder_synced?: boolean
  }>('mail.threadList', {
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
    folderSynced: result.folder_synced,
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
    const { threads, folderUnread, folderUnreadByAccount, folderSynced, nextSingle, nextUnified } =
      await fetchColumnThreads(column, refresh, trimmedQuery)
    if (columnLoadVersions.get(key) !== version) return
    // Only Inbox totals back the side-nav badges. A unified column on another
    // role reports that role's per-account unreads, which must not be written
    // to the cache under 'inbox' — that would overwrite the badge with, say,
    // the unread count of Trash.
    const unreadCacheFolder =
      column.accountId === 'unified' ? (unifiedFolderRole(column.folderId) === 'inbox' ? 'inbox' : '') : column.folderId
    if (unreadCacheFolder) {
      for (const [accountId, unread] of Object.entries(folderUnreadByAccount ?? {})) {
        updateCachedFolderUnread(accountId, unreadCacheFolder, unread)
      }
    }
    kanban$.threads[key].set(keepReadThreads(column, key, threads))
    if (folderUnread !== undefined) kanban$.unreadCounts[key].set(folderUnread)
    kanban$.cursors[key].set(trimmedQuery ? '' : nextSingle)
    kanban$.accountCursors[key].set(trimmedQuery ? {} : nextUnified)

    // A folder nobody has opened yet has nothing cached, so this read served an
    // empty page and only kicked off the background sync. Stay in the loading
    // state until that sync lands instead of flashing "no threads", then take
    // the freshly stored page.
    if (folderSynced === false && threads.length === 0 && refresh && !trimmedQuery && awaitsFirstSync(column)) {
      const completion = waitForKanbanSync([column.accountId], column.folderId, KANBAN_FIRST_SYNC_TIMEOUT_MS)
      try {
        await completion.promise
      } catch {
        return
      } finally {
        completion.cancel()
      }
      if (columnLoadVersions.get(key) !== version) return
      const synced = await fetchColumnThreads(column, false, trimmedQuery)
      if (columnLoadVersions.get(key) !== version) return
      kanban$.threads[key].set(keepReadThreads(column, key, synced.threads))
      if (synced.folderUnread !== undefined) kanban$.unreadCounts[key].set(synced.folderUnread)
      kanban$.cursors[key].set(synced.nextSingle)
      kanban$.accountCursors[key].set(synced.nextUnified)
    }
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
  if (isUnifiedStarredColumn(column)) {
    await loadKanbanColumn(column, false)
    return
  }

  // A sync names a real mailbox, so a unified column resolves its role against
  // each account's own folder list first. Accounts whose server has no such
  // folder are simply not synced — the same accounts the listing leaves out.
  const role = unifiedFolderRole(column.folderId)
  const targets =
    column.accountId === 'unified'
      ? unifiedAccounts().flatMap((account) => {
          // Inbox needs no lookup — every server has one and the core
          // canonicalises the name — so a cold folder cache still syncs it.
          // The other roles have no safe guess and sit the round out.
          const folder =
            accountFolderForRole(mail$.foldersByAccount[account.id].peek(), role) ??
            (role === 'inbox' ? 'inbox' : undefined)
          return folder ? [{ id: account.id, folder }] : []
        })
      : [{ id: column.accountId, folder: column.folderId }]
  if (targets.length === 0) {
    await loadKanbanColumn(column, false)
    return
  }
  // The sync events name each account's own folder, so a unified column waits
  // on whichever folder it asked that account for.
  const completion = waitForKanbanSync(
    targets.map((target) => target.id),
    targets[0].folder,
    KANBAN_SYNC_TIMEOUT_MS,
    new Set(targets.map((target) => target.folder)),
  )
  try {
    const requests = targets.map((target) =>
      invoke<{ online?: boolean }>('mail.sync', { account_id: target.id, folder: target.folder }).then((result) => {
        if (result?.online === false) throw new Error('Mail engine unavailable')
      }),
    )
    await Promise.all([Promise.all(requests), completion.promise])
    await loadKanbanColumn(column, false)
  } catch (error) {
    console.error('Kanban column sync failed:', error)
  } finally {
    completion.cancel()
  }
}

function columnHasMore(key: string, _unified: boolean): boolean {
  return !!(kanban$.cursors[key].get() ?? '')
}

// Append the next page of older threads to a column, de-duping by thread id. The
// scroll handler drives this; it no-ops once the cursors are exhausted.
export async function loadMoreKanbanColumn(column: KanbanColumn) {
  const key = kanbanColumnKey(column)
  const unified = column.accountId === 'unified'
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
    // As in loadKanbanColumn: only Inbox totals belong in the badge cache.
    const unreadCacheFolder = unified
      ? unifiedFolderRole(column.folderId) === 'inbox'
        ? 'inbox'
        : ''
      : column.folderId
    if (unreadCacheFolder) {
      for (const [accountId, unread] of Object.entries(folderUnreadByAccount ?? {})) {
        updateCachedFolderUnread(accountId, unreadCacheFolder, unread)
      }
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
