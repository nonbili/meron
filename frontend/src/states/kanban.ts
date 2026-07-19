import { observable } from '@legendapp/state'
import type { ChatWallpaper, Message } from '../types'
import { isFilterMode, ui$, type FilterMode } from './ui'
import { mail$, refreshAccountFoldersCache } from './mail'
import { accounts$ } from './accounts'
import { filterThreads, isRssAccount } from '../lib/threadActions'
import { persistedField } from '../lib/sessionPref'
import { settings$, type KanbanBoard } from './settings'
import { invoke } from '../lib/bridge'

export type KanbanColumn = {
  accountId: string
  folderId: string
}

const DEFAULT_BOARD_NAME = 'Kanban board'

// Ephemeral kanban runtime state. Board definitions and pane/column sizes live
// in `settings$`; thread caches and open conversation state are per session.
export const kanban$ = observable({
  activeBoardId: '',
  threads: {} as Record<string, Message[]>,
  // Authoritative folder unread totals returned with each column's thread page.
  unreadCounts: {} as Record<string, number>,
  loading: {} as Record<string, boolean>,
  // Pagination cursors per column key. `cursors` holds a single-account folder's
  // opaque next-page cursor; `accountCursors` holds the per-account cursors a
  // unified column needs (one inbox per account). "" / empty = no more pages.
  cursors: {} as Record<string, string>,
  accountCursors: {} as Record<string, Record<string, string>>,
  loadingMore: {} as Record<string, boolean>,
  // Per-column thread filter (keyed by kanbanColumnKey). When set, overrides the
  // board-wide globalFilter for that column; unset columns follow globalFilter.
  filters: {} as Record<string, FilterMode>,
  // Board-wide filter from the header switch; the default for columns that don't
  // have their own filter set.
  globalFilter: 'all' as FilterMode,
  searchQuery: '',
  searchScope: 'all',
  movingThread: '',
  paneThreadId: '',
  paneColumnKey: '',
})

function newBoardId(): string {
  const fallback = `${Date.now()}-${Math.random().toString(16).slice(2)}`
  return `kb-${globalThis.crypto?.randomUUID?.() ?? fallback}`
}

export function defaultKanbanBoard(): KanbanBoard {
  return {
    id: newBoardId(),
    name: DEFAULT_BOARD_NAME,
    columns: [{ accountId: 'unified', folderId: 'inbox' }],
  }
}

export function ensureDefaultKanbanBoard() {
  const boards = settings$.kanbanBoards.get()
  if (boards.length > 0) return
  const board = defaultKanbanBoard()
  settings$.kanbanBoards.set([board])
}

export function activeKanbanBoard(): KanbanBoard | null {
  const boardId = kanban$.activeBoardId.get()
  return settings$.kanbanBoards.get().find((board) => board.id === boardId) ?? null
}

export function selectKanbanBoard(boardId: string) {
  if (!settings$.kanbanBoards.get().some((board) => board.id === boardId)) return
  ui$.selectedThread.set('')
  kanban$.paneThreadId.set('')
  kanban$.paneColumnKey.set('')
  kanban$.activeBoardId.set(boardId)
}

export function closeKanbanPane() {
  ui$.selectedThread.set('')
  kanban$.paneThreadId.set('')
  kanban$.paneColumnKey.set('')
  ui$.mobilePane.set('threads')
}

export function closeKanbanBoard() {
  kanban$.activeBoardId.set('')
  kanban$.paneThreadId.set('')
  kanban$.paneColumnKey.set('')
}

export function createKanbanBoard(name = DEFAULT_BOARD_NAME): string {
  const board = { ...defaultKanbanBoard(), name: name.trim() || DEFAULT_BOARD_NAME }
  settings$.kanbanBoards.set([...settings$.kanbanBoards.get(), board])
  selectKanbanBoard(board.id)
  return board.id
}

export function renameKanbanBoard(boardId: string, name: string) {
  const nextName = name.trim()
  if (!nextName) return
  settings$.kanbanBoards.set(
    settings$.kanbanBoards.get().map((board) => (board.id === boardId ? { ...board, name: nextName } : board)),
  )
}

/** Set or clear ("" clears) a board's custom rail/header image. */
export function setKanbanBoardAvatar(boardId: string, avatarUrl: string) {
  const url = avatarUrl.trim()
  settings$.kanbanBoards.set(
    settings$.kanbanBoards.get().map((board) => {
      if (board.id !== boardId) return board
      const { avatarUrl: _, ...rest } = board
      return url ? { ...rest, avatarUrl: url } : rest
    }),
  )
}

/** Set or clear (null clears) the background shown behind a board's columns. */
export function setKanbanBoardWallpaper(boardId: string, wallpaper: ChatWallpaper | null) {
  settings$.kanbanBoards.set(
    settings$.kanbanBoards.get().map((board) => {
      if (board.id !== boardId) return board
      const { wallpaper: _, ...rest } = board
      return wallpaper ? { ...rest, wallpaper } : rest
    }),
  )
}

export function removeKanbanBoard(boardId: string) {
  const boards = settings$.kanbanBoards.get()
  if (boards.length <= 1) {
    const board = defaultKanbanBoard()
    settings$.kanbanBoards.set([board])
    selectKanbanBoard(board.id)
    return
  }
  const index = boards.findIndex((board) => board.id === boardId)
  if (index === -1) return
  const next = boards.filter((board) => board.id !== boardId)
  settings$.kanbanBoards.set(next)
  if (kanban$.activeBoardId.peek() === boardId) {
    selectKanbanBoard(next[Math.max(0, index - 1)]?.id ?? next[0].id)
  }
}

export function reorderKanbanBoards(oldIndex: number, newIndex: number) {
  const boards = [...settings$.kanbanBoards.get()]
  if (oldIndex < 0 || newIndex < 0 || oldIndex >= boards.length || newIndex >= boards.length) return
  const [removed] = boards.splice(oldIndex, 1)
  boards.splice(newIndex, 0, removed)
  settings$.kanbanBoards.set(boards)
}

// The board-wide filter from the header switch. It's the default for columns
// without their own filter. Both this and the per-column overrides persist
// across restarts (see globalFilterSession / columnFiltersSession below).
// Touching the switch clears all per-column overrides — the most recent choice
// wins, so a stale override can never silently mask the global filter.
export function setGlobalKanbanFilter(mode: FilterMode) {
  kanban$.filters.set({})
  kanban$.globalFilter.set(mode)
}

const globalFilterSession = persistedField(kanban$.globalFilter, 'session_kanban_filter', (raw) =>
  isFilterMode(raw) ? raw : undefined,
)
// Per-column filter overrides, keyed by kanbanBoardColumnKey. Persisted so a
// column set to "all" while global is "unread" keeps that choice on restart.
const columnFiltersSession = persistedField(kanban$.filters, 'session_kanban_column_filters', (raw) => {
  if (!raw || typeof raw !== 'object') return undefined
  const out: Record<string, FilterMode> = {}
  for (const [colKey, mode] of Object.entries(raw as Record<string, unknown>)) {
    if (isFilterMode(mode)) out[colKey] = mode
  }
  return out
})
const activeBoardSession = persistedField(kanban$.activeBoardId, 'session_kanban_board', (raw) => {
  if (typeof raw !== 'string') return undefined
  if (raw === '') return ''
  return settings$.kanbanBoards.get().some((board) => board.id === raw) ? raw : ''
})

/** Prefs keys this module owns; boot requests them in its single prefsGet. */
export const KANBAN_SESSION_KEYS = [globalFilterSession.key, columnFiltersSession.key, activeBoardSession.key]

/** Seed kanban session state from persisted last session. Call once at boot. */
export function restoreKanbanSession(prefs: Record<string, unknown>) {
  globalFilterSession.restore(prefs)
  columnFiltersSession.restore(prefs)
  activeBoardSession.restore(prefs)
}

export function kanbanColumnKey(column: KanbanColumn): string {
  return `${column.accountId}\n${column.folderId}`
}

export function kanbanBoardColumnKey(boardId: string, column: KanbanColumn): string {
  return `${boardId}\n${kanbanColumnKey(column)}`
}

function updateBoard(boardId: string, update: (board: KanbanBoard) => KanbanBoard) {
  settings$.kanbanBoards.set(
    settings$.kanbanBoards.get().map((board) => (board.id === boardId ? update(board) : board)),
  )
}

export function getKanbanColumns(boardId: string): KanbanColumn[] {
  return settings$.kanbanBoards.get().find((board) => board.id === boardId)?.columns ?? []
}

export function getAllKanbanColumns(): KanbanColumn[] {
  return settings$.kanbanBoards.get().flatMap((board) => board.columns)
}

export function addKanbanColumn(boardId: string, column: KanbanColumn) {
  if (!boardId || !column.accountId || !column.folderId) return
  const entry = kanbanColumnKey(column)
  updateBoard(boardId, (board) => {
    if (board.columns.some((item) => kanbanColumnKey(item) === entry)) return board
    return { ...board, columns: [...board.columns, column] }
  })
}

export function reorderKanbanColumn(boardId: string, from: KanbanColumn, to: KanbanColumn) {
  if (!boardId) return
  const fromEntry = kanbanColumnKey(from)
  const toEntry = kanbanColumnKey(to)
  if (fromEntry === toEntry) return
  updateBoard(boardId, (board) => {
    const fromIdx = board.columns.findIndex((column) => kanbanColumnKey(column) === fromEntry)
    const toIdx = board.columns.findIndex((column) => kanbanColumnKey(column) === toEntry)
    if (fromIdx === -1 || toIdx === -1) return board
    const columns = [...board.columns]
    const [removed] = columns.splice(fromIdx, 1)
    columns.splice(toIdx, 0, removed)
    return { ...board, columns }
  })
}

export function removeKanbanColumn(boardId: string, column: KanbanColumn) {
  if (!boardId) return
  const entry = kanbanColumnKey(column)
  updateBoard(boardId, (board) => ({
    ...board,
    columns: board.columns.filter((item) => kanbanColumnKey(item) !== entry),
  }))

  const key = kanbanColumnKey(column)
  const nextThreads = { ...kanban$.threads.get() }
  delete nextThreads[key]
  kanban$.threads.set(nextThreads)
  const nextCursors = { ...kanban$.cursors.get() }
  delete nextCursors[key]
  kanban$.cursors.set(nextCursors)
  const nextAccountCursors = { ...kanban$.accountCursors.get() }
  delete nextAccountCursors[key]
  kanban$.accountCursors.set(nextAccountCursors)
}

// Mark a column as read. Mail columns are marked folder-wide, so unread messages
// outside the loaded page are cleared too; RSS/starred aggregates fall back to
// per loaded item/thread operations because they have no folder-wide unread flag.
export async function markColumnAllRead(column: KanbanColumn) {
  const key = kanbanColumnKey(column)
  const threads = kanban$.threads[key].get() ?? []
  const unread = threads.filter((thread) => thread.unread)

  if (column.accountId === 'unified' && column.folderId.toLowerCase() === 'starred') {
    if (unread.length === 0) return
    kanban$.threads[key].set(
      threads.map((thread) => (thread.unread ? { ...thread, unread: false, unread_count: 0 } : thread)),
    )
    await Promise.all(
      unread.map((thread) =>
        invoke('mail.markRead', { thread_id: thread.thread_id, message_ids: [thread.id] }).catch((err) =>
          console.error('markAllRead (starred) failed:', err),
        ),
      ),
    )
    return
  }

  const accounts = accounts$.get()
  const columnAccount = accounts.find((account) => account.id === column.accountId)
  const mailAccountIds =
    column.accountId === 'unified'
      ? accounts
          .filter((account) => account.included_in_unified !== false && !isRssAccount(account, account.id))
          .map((account) => account.id)
      : !isRssAccount(columnAccount, column.accountId)
        ? [column.accountId]
        : []
  const rssUnread = unread.filter((thread) =>
    isRssAccount(
      accounts.find((account) => account.id === thread.account_id),
      thread.account_id,
    ),
  )
  if (mailAccountIds.length === 0 && rssUnread.length === 0) return

  kanban$.threads[key].set(
    threads.map((thread) => (thread.unread ? { ...thread, unread: false, unread_count: 0 } : thread)),
  )
  kanban$.unreadCounts[key].set(0)

  await Promise.all([
    ...mailAccountIds.map((accountId) =>
      invoke('mail.markAllRead', { account_id: accountId, folder_id: column.folderId }).catch((err) =>
        console.error('markAllRead failed:', err),
      ),
    ),
    ...rssUnread.map((thread) =>
      invoke('mail.markRead', { thread_id: thread.thread_id }).catch((err) =>
        console.error('markAllRead (rss) failed:', err),
      ),
    ),
  ])
  await Promise.all(
    Array.from(new Set([...mailAccountIds, ...rssUnread.map((thread) => thread.account_id)]))
      .filter(Boolean)
      .map((accountId) => refreshAccountFoldersCache(accountId, false)),
  )
}

// Move the open kanban conversation through the visible cards in its current
// column. This mirrors chat-list adjacent navigation but uses the active board.
export function selectAdjacentKanbanThread(delta: number) {
  const selected = kanban$.paneThreadId.get() || ui$.selectedThread.get()
  const boardId = kanban$.activeBoardId.get()
  const columns = getKanbanColumns(boardId)
  const allThreads = kanban$.threads.get()
  const filters = kanban$.filters.get()
  const globalFilter = kanban$.globalFilter.get()
  const preferredKey = kanban$.paneColumnKey.get()
  const orderedColumns = preferredKey
    ? [
        ...columns.filter((column) => kanbanBoardColumnKey(boardId, column) === preferredKey),
        ...columns.filter((column) => kanbanBoardColumnKey(boardId, column) !== preferredKey),
      ]
    : columns

  let fallback: { key: string; thread: Message } | undefined
  for (const column of orderedColumns) {
    const sourceKey = kanbanColumnKey(column)
    const boardKey = kanbanBoardColumnKey(boardId, column)
    const rawThreads = allThreads[sourceKey] ?? []
    const filterMode = filters[sourceKey] ?? globalFilter
    const list = filterThreads(rawThreads, filterMode, selected, mail$.readThreads.get())
    if (!fallback && list.length > 0) {
      fallback = { key: boardKey, thread: delta >= 0 ? list[0] : list[list.length - 1] }
    }
    if (!selected || !rawThreads.some((thread) => thread.thread_id === selected)) continue

    const current = list.findIndex((thread) => thread.thread_id === selected)
    if (current === -1) continue

    const next = Math.min(list.length - 1, Math.max(0, current + delta))
    const target = list[next]
    if (!target || target.thread_id === selected) return

    ui$.selectedFolder.set(target.folder_id)
    ui$.selectedThread.set(target.thread_id)
    kanban$.paneThreadId.set(target.thread_id)
    kanban$.paneColumnKey.set(boardKey)
    ui$.mobilePane.set('conversation')
    return
  }

  if (fallback) {
    ui$.selectedFolder.set(fallback.thread.folder_id)
    ui$.selectedThread.set(fallback.thread.thread_id)
    kanban$.paneThreadId.set(fallback.thread.thread_id)
    kanban$.paneColumnKey.set(fallback.key)
    ui$.mobilePane.set('conversation')
  }
}
