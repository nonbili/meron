import { observable } from '@legendapp/state'
import { isNarrowWindow } from '../lib/useNarrowWindow'
import type { ChatWallpaper, Folder, Message } from '../types'
import { isFilterMode, pauseMailFolderPersist, persistMailFolder, ui$, showToast, type FilterMode } from './ui'
import { mail$ } from './mail'
import {
  captureFolderUnreadVersion,
  holdFolderUnread,
  folderMatches,
  refreshAccountFoldersCache,
  updateCachedFolderUnread,
} from './mailFolders'
import { accounts$ } from './accounts'
import { filterThreads, isRssAccount } from '../lib/threadActions'
import { isUnifiedStarredColumn, loadKanbanColumn } from '../lib/kanbanData'
import { accountFolderForRole, unifiedFolderRole } from '../lib/unifiedFolders'
import { openMessageTab } from './compose'
import { compose$ } from './composeState'
import { persistedField } from '../lib/sessionPref'
import { hasStoredKanbanBoards, normalizeKanbanColumnQuery, settings$, type KanbanBoard } from './settings'
import { invoke } from '../lib/bridge'
import { thread$ } from './thread'
import { t } from '../lib/i18n'
import { boardWallpaper } from '../lib/wallpapers'

export type KanbanColumn = {
  accountId: string
  folderId: string
  /** Set on a saved search column: it always lists this folder's matches. */
  query?: string
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
  // Per-column attachments toggle (keyed by kanbanColumnKey), overriding
  // globalAttachmentsOnly for that column the same way `filters` does.
  attachmentsOnly: {} as Record<string, boolean>,
  // Board-wide attachments toggle, independent of the read-state filters above
  // and the default for columns without their own toggle set.
  globalAttachmentsOnly: false,
  searchQuery: '',
  searchScope: 'all',
  movingThread: '',
  // The card conversation in the pane, behind any tabs (see kanbanPaneThreadId).
  paneThreadId: '',
  // The column of the card last opened, as a conversation or in a tab: j/k
  // steps through it first.
  paneColumnKey: '',
  // The card selected from the keyboard while no conversation is open: the one
  // Esc closed the conversation of, then wherever the arrow keys move. It has
  // keyboard focus and reads as selected until a conversation opens or focus
  // moves on.
  keyboardCard: null as KanbanCardRef | null,
})

// The highlight belongs to the open conversation's card: the keyboard's card
// gives it up the moment a conversation opens or the board changes.
kanban$.paneThreadId.onChange(({ value }) => {
  if (value) kanban$.keyboardCard.set(null)
})
kanban$.activeBoardId.onChange(() => kanban$.keyboardCard.set(null))

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

/** Seed the starter board on a fresh profile only. An empty stored list is a
 * user who deleted every board, so it must survive the restart. */
export function ensureDefaultKanbanBoard() {
  if (hasStoredKanbanBoards() || settings$.kanbanBoards.get().length > 0) return
  settings$.kanbanBoards.set([defaultKanbanBoard()])
}

export function activeKanbanBoard(): KanbanBoard | null {
  const boardId = kanban$.activeBoardId.get()
  return settings$.kanbanBoards.get().find((board) => board.id === boardId) ?? null
}

export function selectKanbanBoard(boardId: string) {
  if (!settings$.kanbanBoards.get().some((board) => board.id === boardId)) return
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  // An open tab carries over to the board's pane, and an active thread tab owns
  // selectedThread (it renders only while the two match), so clearing it would
  // blank the tab's conversation. Only the card conversation behind it resets.
  const activeTab = compose$.activeTab.peek()
  if (!compose$.tabs.peek().some((tab) => tab.id === activeTab)) ui$.selectedThread.set('')
  kanban$.paneThreadId.set('')
  kanban$.paneColumnKey.set('')
  ui$.calendarOpen.set(false)
  kanban$.activeBoardId.set(boardId)
}

// The thread the board's conversation pane is showing: an active thread or
// reader tab's own thread, else the card conversation behind the tabs. Tabs
// opened from a card leave paneThreadId alone, so it alone would point
// shortcuts at the wrong thread, or at none when the pane was closed.
export function kanbanPaneThreadId(): string {
  const activeTab = compose$.activeTab.peek()
  const tab = compose$.tabs.peek().find((item) => item.id === activeTab)
  if (tab && tab.kind !== 'compose') return tab.threadId
  return kanban$.paneThreadId.peek()
}

// Close the conversation behind the tabs (the Current tab): a card opened in
// the kanban pane, or the selected thread in the mail view. The kanban pane
// itself only goes away when nothing else is left in it: tabs opened over the
// conversation are rendered by this same pane, so hiding it with them still
// open would strand them out of sight (and still in the strip when a card
// reopens the pane).
export function closeCurrentConversation() {
  kanban$.paneThreadId.set('')
  kanban$.paneColumnKey.set('')
  // Nothing to return to once the card's conversation is closed, so the Current
  // tab stops offering one (see ConversationTabs).
  compose$.conversationThread.set('')
  const tabs = compose$.tabs.peek()
  const activeTab = compose$.activeTab.peek()
  // A tab is on screen rather than the card's conversation: it owns the pane
  // and selectedThread, which points at the thread it renders. Leave both be.
  if (tabs.some((tab) => tab.id === activeTab)) return
  ui$.selectedThread.set('')
  const last = tabs[tabs.length - 1]
  if (last) {
    compose$.activeTab.set(last.id)
    if (last.kind === 'thread') ui$.selectedThread.set(last.threadId)
    return
  }
  ui$.mobilePane.set('threads')
}

// Opening a card retargets ui$.selectedFolder to that thread's own folder so
// thread actions resolve against it. Stash the folder the mail view was on the
// first time that happens per board visit, so closing the board goes back there
// instead of stranding the mail view on the last card's folder.
let mailViewFolder: string | null = null

/** Point the selection at a card's folder, remembering the mail view's own. */
export function focusKanbanThreadFolder(folderId: string) {
  if (mailViewFolder === null) mailViewFolder = ui$.selectedFolder.peek()
  // The stash is in-memory only, so the session pref keeps describing the mail
  // view; a restart with the board still open resumes there, not on the card.
  pauseMailFolderPersist(true)
  ui$.selectedFolder.set(folderId)
}

/**
 * Drop the stashed folder if deleting `folderIds` removed it, so closing the
 * board can't restore the mail view onto a folder that no longer exists.
 */
export function forgetDeletedMailViewFolder(folderIds: string[]) {
  if (mailViewFolder === null || !folderIds.includes(mailViewFolder)) return
  mailViewFolder = 'inbox'
  persistMailFolder(mailViewFolder)
}

/** Follow a folder rename in the stashed mail view folder. */
export function renameMailViewFolder(renames: Map<string, string>) {
  const renamed = mailViewFolder === null ? undefined : renames.get(mailViewFolder)
  if (renamed === undefined) return
  mailViewFolder = renamed
  persistMailFolder(mailViewFolder)
}

/**
 * Leave any open board and show `accountId`'s mail. Coming back to the account
 * the mail view is already on is a view switch, not a navigation: it keeps that
 * view's folder (e.g. Unified starred) instead of snapping back to `folderId`.
 */
export function openMailAccount(accountId: string, folderId = 'inbox', resumeCurrentFolder = true) {
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
  const resumesMailView =
    resumeCurrentFolder && !!kanban$.activeBoardId.peek() && ui$.selectedAccount.peek() === accountId
  if (!resumesMailView) {
    // Going somewhere else: drop the stash unused, so closing the board doesn't
    // also write the old folder — two prefs writes for one key can land out of
    // order, and only the destination folder below should survive.
    mailViewFolder = null
    pauseMailFolderPersist(false)
  }
  closeKanbanBoard()
  ui$.calendarOpen.set(false)
  ui$.selectedAccount.set(accountId)
  if (resumesMailView) return
  ui$.selectedFolder.set(folderId)
  // The card may already have had the selection on `folderId`, in which case the
  // set above changes nothing and the persist listener never fires — while the
  // pref still holds the folder the paused mail view was on. Write it directly.
  persistMailFolder(folderId)
}

/** Leave the conversation/board and show mail involving one correspondent.
 * Mail search already merges the selected folder with Sent, so a bare address
 * finds both incoming and outgoing messages without provider-specific syntax. */
export function openCorrespondentMail(accountId: string, folderId: string, email: string) {
  const query = email.trim()
  if (!query) return
  // This action targets the card/conversation folder, so it is a navigation,
  // not a view resume. Let openMailAccount discard the stash before closing the
  // board and persist the destination through its single-write path.
  openMailAccount(accountId, folderId, false)
  ui$.filterMode.set('all')
  ui$.query.set(query)
  ui$.selectedThread.set('')
  thread$.mediaOpen.set(false)
  ui$.mobilePane.set('threads')
}

export function closeKanbanBoard() {
  pauseMailFolderPersist(false)
  if (mailViewFolder !== null) {
    ui$.selectedFolder.set(mailViewFolder)
    mailViewFolder = null
  }
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

/** Set or clear (null or Plain clears) the background shown behind a board's columns. */
export function setKanbanBoardWallpaper(boardId: string, wallpaper: ChatWallpaper | null) {
  const next = boardWallpaper(wallpaper)
  settings$.kanbanBoards.set(
    settings$.kanbanBoards.get().map((board) => {
      if (board.id !== boardId) return board
      const { wallpaper: _, ...rest } = board
      return next ? { ...rest, wallpaper: next } : rest
    }),
  )
}

/** Turn the unread count on a board's rail tile on or off. */
export function setKanbanBoardUnreadBadge(boardId: string, show: boolean) {
  updateBoard(boardId, ({ showUnreadBadge: _, ...rest }) => (show ? { ...rest, showUnreadBadge: true } : rest))
}

export function removeKanbanBoard(boardId: string) {
  const boards = settings$.kanbanBoards.get()
  const index = boards.findIndex((board) => board.id === boardId)
  if (index === -1) return
  const next = boards.filter((board) => board.id !== boardId)
  settings$.kanbanBoards.set(next)
  if (kanban$.activeBoardId.peek() !== boardId) return
  // Deleting the board on screen: fall back to its neighbour, or leave the board
  // view for the mail view when that was the last board.
  const fallback = next[Math.max(0, index - 1)]?.id
  if (fallback) selectKanbanBoard(fallback)
  else closeKanbanBoard()
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

// The board-wide attachments toggle; like setGlobalKanbanFilter, it clears the
// per-column overrides so the latest choice applies to every column.
export function setGlobalKanbanAttachmentsOnly(on: boolean) {
  kanban$.attachmentsOnly.set({})
  kanban$.globalAttachmentsOnly.set(on)
}

const globalFilterSession = persistedField(kanban$.globalFilter, 'session_kanban_filter', (raw) =>
  isFilterMode(raw) ? raw : undefined,
)
const attachmentsSession = persistedField(kanban$.globalAttachmentsOnly, 'session_kanban_attachments_only', (raw) =>
  typeof raw === 'boolean' ? raw : undefined,
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
const columnAttachmentsSession = persistedField(
  kanban$.attachmentsOnly,
  'session_kanban_column_attachments_only',
  (raw) => {
    if (!raw || typeof raw !== 'object') return undefined
    const out: Record<string, boolean> = {}
    for (const [colKey, on] of Object.entries(raw as Record<string, unknown>)) {
      if (typeof on === 'boolean') out[colKey] = on
    }
    return out
  },
)
const activeBoardSession = persistedField(kanban$.activeBoardId, 'session_kanban_board', (raw) => {
  if (typeof raw !== 'string') return undefined
  if (raw === '') return ''
  return settings$.kanbanBoards.get().some((board) => board.id === raw) ? raw : ''
})

const parsePaneString = (raw: unknown) => (typeof raw === 'string' ? raw : undefined)
const paneThreadSession = persistedField(kanban$.paneThreadId, 'session_kanban_pane_thread', parsePaneString)
const paneColumnSession = persistedField(kanban$.paneColumnKey, 'session_kanban_pane_column', parsePaneString)

/** Prefs keys this module owns; boot requests them in its single prefsGet. */
export const KANBAN_SESSION_KEYS = [
  globalFilterSession.key,
  attachmentsSession.key,
  columnFiltersSession.key,
  columnAttachmentsSession.key,
  activeBoardSession.key,
  paneThreadSession.key,
  paneColumnSession.key,
]

/** Seed kanban session state from persisted last session. Call once at boot. */
export function restoreKanbanSession(prefs: Record<string, unknown>) {
  globalFilterSession.restore(prefs)
  attachmentsSession.restore(prefs)
  columnFiltersSession.restore(prefs)
  columnAttachmentsSession.restore(prefs)
  activeBoardSession.restore(prefs)
  paneThreadSession.restore(prefs)
  paneColumnSession.restore(prefs)
}

// A saved search is its own column beside the folder's plain one, so its query is
// part of the key. Splitting a key on "\n" still yields account and folder first.
export function kanbanColumnKey(column: KanbanColumn): string {
  const base = `${column.accountId}\n${column.folderId}`
  return column.query ? `${base}\n${column.query}` : base
}

// The board search inside a saved search column narrows it. The core reads each
// side of an `AND` as its own query that must also match, so free text, dates
// and read state narrow the saved query rather than merging into or replacing
// its own. The search starts a line of its own, which no quote or group left
// open in the saved query reaches into. A leading `OR` would be a stray word
// there, so it is dropped.
export function narrowKanbanQuery(saved: string | undefined, query: string): string {
  if (!saved) return query.trim()
  const extra = query.trim().replace(/^(?:OR(?:\s+|$))+/, '')
  return extra ? `${saved}\nAND ${extra}` : saved
}

export function kanbanBoardColumnKey(boardId: string, column: KanbanColumn): string {
  return `${boardId}\n${kanbanColumnKey(column)}`
}

function updateBoard(boardId: string, update: (board: KanbanBoard) => KanbanBoard) {
  settings$.kanbanBoards.set(
    settings$.kanbanBoards.get().map((board) => (board.id === boardId ? update(board) : board)),
  )
}

/** Every saved search on every board, for the core to read. */
export function kanbanColumnQueries(): string[] {
  return [
    ...new Set(settings$.kanbanBoards.get().flatMap((board) => board.columns.flatMap((column) => column.query ?? []))),
  ]
}

/** Saved searches take the given new spellings, on every board, each column
 *  keeping its slot and state. One that comes to mean the search of another
 *  column on its board joins it: whatever pointed at it moves there. */
export function renameKanbanColumnQueries(renamed: Map<string, string>) {
  for (const board of settings$.kanbanBoards.get()) {
    for (const column of board.columns) {
      const query = column.query && renamed.get(column.query)
      if (!query || query === column.query) continue
      const target = { ...column, query }
      if (replaceKanbanColumn(board.id, column, target)) continue
      if (kanban$.searchScope.peek() === kanbanColumnKey(column)) kanban$.searchScope.set(kanbanColumnKey(target))
      if (kanban$.paneColumnKey.peek() === kanbanBoardColumnKey(board.id, column)) {
        kanban$.paneColumnKey.set(kanbanBoardColumnKey(board.id, target))
      }
      removeKanbanColumn(board.id, column)
    }
  }
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

// Turn the board search on one column into a column of its own, placed right
// after it. Returns the new column, or null when there is nothing to save or the
// board already has it. A saved search narrowed by the board search isn't saved
// again: its two queries would have to share one line.
export function saveKanbanSearchColumn(boardId: string, column: KanbanColumn, query: string): KanbanColumn | null {
  const saved = normalizeKanbanColumnQuery(query)
  if (!boardId || !saved || column.query) return null
  const target: KanbanColumn = { accountId: column.accountId, folderId: column.folderId, query: saved }
  const entry = kanbanColumnKey(target)
  const columns = getKanbanColumns(boardId)
  if (columns.some((item) => kanbanColumnKey(item) === entry)) return null
  const sourceEntry = kanbanColumnKey(column)
  updateBoard(boardId, (board) => {
    const next = [...board.columns]
    const at = next.findIndex((item) => kanbanColumnKey(item) === sourceEntry)
    next.splice(at === -1 ? next.length : at + 1, 0, target)
    return { ...board, columns: next }
  })
  return target
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

// Drop every column showing a folder, saved searches in it included, on all
// boards. Used once the folder is gone from the server: a column left behind
// would only fail to load.
export function removeKanbanColumnsForFolder(accountId: string, folderId: string) {
  if (!accountId || !folderId) return
  for (const board of settings$.kanbanBoards.get()) {
    for (const column of board.columns) {
      if (column.accountId === accountId && column.folderId === folderId) removeKanbanColumn(board.id, column)
    }
  }
}

// Follow a folder rename: columns on the old names show the new ones, in the
// same slots, and load them afresh.
export function renameKanbanColumnsForFolder(accountId: string, renames: Map<string, string>) {
  if (!accountId) return
  for (const board of settings$.kanbanBoards.get()) {
    for (const column of board.columns) {
      const folderId = column.accountId === accountId ? renames.get(column.folderId) : undefined
      if (folderId === undefined) continue
      // A board already showing the new name keeps that column alone.
      if (!replaceKanbanColumn(board.id, column, { ...column, folderId })) removeKanbanColumn(board.id, column)
      else void loadKanbanColumn({ ...column, folderId }, true)
    }
  }
}

// Point an existing column at another folder of the same account, keeping its
// slot on the board. Returns false when the switch can't happen (no such board
// column, or the target folder already has a column here).
export function switchKanbanColumnFolder(boardId: string, column: KanbanColumn, folderId: string): boolean {
  if (!boardId || !folderId || folderId === column.folderId) return false
  return replaceKanbanColumn(boardId, column, { ...column, folderId })
}

// Give a saved search column a new query, keeping its slot on the board. Returns
// the edited column, or null when the query is empty, unchanged, or already has
// a column of its own here.
export function editKanbanSearchColumn(boardId: string, column: KanbanColumn, query: string): KanbanColumn | null {
  const next = normalizeKanbanColumnQuery(query)
  if (!boardId || !column.query || !next || next === column.query) return null
  const target: KanbanColumn = { ...column, query: next }
  return replaceKanbanColumn(boardId, column, target) ? target : null
}

// Swap `column` for `target` in place, carrying the column's state over so the
// change feels like the same column. False when the board has no such column or
// already shows the target.
function replaceKanbanColumn(boardId: string, column: KanbanColumn, target: KanbanColumn): boolean {
  const fromEntry = kanbanColumnKey(column)
  const toEntry = kanbanColumnKey(target)
  const columns = getKanbanColumns(boardId)
  const index = columns.findIndex((item) => kanbanColumnKey(item) === fromEntry)
  if (index === -1) return false
  if (columns.some((item) => kanbanColumnKey(item) === toEntry)) return false

  updateBoard(boardId, (board) => {
    const next = [...board.columns]
    const at = next.findIndex((item) => kanbanColumnKey(item) === fromEntry)
    if (at === -1) return board
    next[at] = target
    return { ...board, columns: next }
  })

  // Carry the column's per-board and per-key state over to the new key so the
  // switch feels like the same column, then drop the old entries.
  const fromBoardKey = kanbanBoardColumnKey(boardId, column)
  const toBoardKey = kanbanBoardColumnKey(boardId, target)
  const minimized = settings$.kanbanMinimizedColumns[fromBoardKey].get()
  if (minimized !== undefined) {
    settings$.kanbanMinimizedColumns[toBoardKey].set(minimized)
    settings$.kanbanMinimizedColumns[fromBoardKey].delete()
  }
  if (kanban$.paneColumnKey.get() === fromBoardKey) kanban$.paneColumnKey.set(toBoardKey)
  if (kanban$.searchScope.get() === fromEntry) kanban$.searchScope.set(toEntry)

  const filter = kanban$.filters[fromEntry].get()
  if (filter !== undefined) kanban$.filters[toEntry].set(filter)
  const attachmentsOnly = kanban$.attachmentsOnly[fromEntry].get()
  if (attachmentsOnly !== undefined) kanban$.attachmentsOnly[toEntry].set(attachmentsOnly)

  // Only discard the old column's cached page when no other board still shows it.
  if (!getAllKanbanColumns().some((item) => kanbanColumnKey(item) === fromEntry)) {
    const nextThreads = { ...kanban$.threads.get() }
    delete nextThreads[fromEntry]
    kanban$.threads.set(nextThreads)
    const nextCursors = { ...kanban$.cursors.get() }
    delete nextCursors[fromEntry]
    kanban$.cursors.set(nextCursors)
    const nextAccountCursors = { ...kanban$.accountCursors.get() }
    delete nextAccountCursors[fromEntry]
    kanban$.accountCursors.set(nextAccountCursors)
    const nextFilters = { ...kanban$.filters.get() }
    delete nextFilters[fromEntry]
    kanban$.filters.set(nextFilters)
    const nextAttachmentsOnly = { ...kanban$.attachmentsOnly.get() }
    delete nextAttachmentsOnly[fromEntry]
    kanban$.attachmentsOnly.set(nextAttachmentsOnly)
  }
  return true
}

// Mark a column as read. Mail columns are marked folder-wide, so unread messages
// outside the loaded page are cleared too; RSS/starred aggregates fall back to
// per loaded item/thread operations because they have no folder-wide unread flag.
type ReadRequest = (command: string, payload: Record<string, string>) => Promise<void>

const requestRead: ReadRequest = async (command, payload) => {
  const result = await invoke<{ ok?: boolean; failures?: Array<{ message: string }> }>(command, payload)
  if (result?.ok === false || result?.failures?.length) {
    throw new Error(result.failures?.map((failure) => failure.message).join('; ') || t('notification.markReadFailed'))
  }
}

async function markColumnRead(column: KanbanColumn, request: ReadRequest, baseline?: Record<string, Folder[]>) {
  const key = kanbanColumnKey(column)
  const threads = kanban$.threads[key].get() ?? []
  const unread = threads.filter((thread) => thread.unread)
  const accounts = accounts$.get()
  const starred = isUnifiedStarredColumn(column)
  // A saved search marks only its matches read, never the whole folder, so it
  // goes thread by thread like the starred aggregate.
  const perItem = starred || !!column.query
  const columnAccount = accounts.find((account) => account.id === column.accountId)
  const mailAccountIds = perItem
    ? []
    : column.accountId === 'unified'
      ? accounts
          .filter((account) => account.included_in_unified !== false && !isRssAccount(account, account.id))
          .map((account) => account.id)
      : !isRssAccount(columnAccount, column.accountId)
        ? [column.accountId]
        : []
  const itemTargets = perItem
    ? unread
    : unread.filter((thread) =>
        isRssAccount(
          accounts.find((account) => account.id === thread.account_id),
          thread.account_id,
        ),
      )
  const writes = [
    ...(mailAccountIds.length
      ? [() => request('mail.markAllRead', { account_id: column.accountId, folder_id: column.folderId })]
      : []),
    ...itemTargets.map((thread) => () => request('mail.markRead', { thread_id: thread.thread_id })),
  ]
  // The side navigation reads its badges from the folder cache, not from the
  // column, so clearing only `unreadCounts` left the nav counts stale until the
  // folder refresh below came back from the server. Work out the same folders
  // the writes cover and their pre-write totals, to clear now and put back if a
  // write fails: a mail account's folder goes to zero (the write is
  // folder-wide), an RSS folder loses only the items marked here.
  const folderTargets = new Map<string, { accountId: string; folderId: string; previous: number; next: number }>()
  const role = column.accountId === 'unified' ? unifiedFolderRole(column.folderId) : ''
  const targetFolder = (accountId: string, folderId: string | undefined) => {
    if (!accountId || accountId === 'unified' || !folderId) return undefined
    const folders = mail$.foldersByAccount[accountId].get() ?? []
    const resolved = folders.find((folder) => folderMatches(folder, accountId, folderId))
    if (!resolved) return undefined
    const key = `${accountId}\n${resolved.id}`
    let target = folderTargets.get(key)
    if (!target) {
      // The columns of one board clear in turn without awaiting, so by the time
      // a later column reads the cache an earlier one sharing this folder has
      // already zeroed it. `baseline` is the board's pre-clear snapshot, so both
      // columns roll back to the count the folder really had; the optimistic
      // number still counts down from what is on screen now.
      const before = baseline?.[accountId]?.find((folder) => folderMatches(folder, accountId, folderId))
      target = { accountId, folderId: resolved.id, previous: before?.unread ?? resolved.unread, next: resolved.unread }
      folderTargets.set(key, target)
    }
    return target
  }
  for (const accountId of mailAccountIds) {
    const folderId = role ? accountFolderForRole(mail$.foldersByAccount[accountId].get(), role) : column.folderId
    const target = targetFolder(accountId, folderId)
    if (target) target.next = 0
  }
  for (const thread of itemTargets) {
    const target = targetFolder(thread.account_id, thread.folder_id)
    if (target) target.next = Math.max(0, target.next - 1)
  }
  // Optimistic: clear the cards before any await, so a board-wide mark flips
  // every column at once. A failure puts back only the rows cleared here (by id,
  // so a page load that landed meanwhile survives) and this column's badge.
  const previousById = new Map(unread.map((thread) => [thread.thread_id, thread]))
  const previousUnreadCount = kanban$.unreadCounts[key].get()
  const clearsBadge = !starred && writes.length > 0
  kanban$.threads[key].set(
    threads.map((thread) =>
      previousById.has(thread.thread_id) ? { ...thread, unread: false, unread_count: 0 } : thread,
    ),
  )
  if (clearsBadge) kanban$.unreadCounts[key].set(0)
  for (const target of folderTargets.values())
    updateCachedFolderUnread(target.accountId, target.folderId, target.next, 'local')
  const settleCounts = new Map(
    Array.from(folderTargets.values(), (target) => [target, holdFolderUnread(target.accountId, target.folderId)]),
  )
  const results = await Promise.allSettled(writes.map((write) => write()))
  const refreshed = new Map<string, Folder[]>()
  const refreshVersions = new Map<string, number>()
  await Promise.all(
    Array.from(new Set([...mailAccountIds, ...itemTargets.map((thread) => thread.account_id)]))
      .filter(Boolean)
      .map(async (accountId) => {
        refreshVersions.set(accountId, captureFolderUnreadVersion())
        refreshed.set(accountId, await refreshAccountFoldersCache(accountId, false))
      }),
  )
  const failure = results.find((result) => result.status === 'rejected')
  if (failure?.status === 'rejected') {
    kanban$.threads[key].set(
      (kanban$.threads[key].get() ?? []).map((thread) => {
        const previous = previousById.get(thread.thread_id)
        return previous ? { ...thread, unread: previous.unread, unread_count: previous.unread_count } : thread
      }),
    )
    if (clearsBadge) kanban$.unreadCounts[key].set(previousUnreadCount)
    for (const target of folderTargets.values()) {
      // One failed write does not mean every folder is still unread: the
      // refresh above answered with the server's own counts for the accounts it
      // reached — including those a sibling write did mark read — so put back
      // the pre-write count only where it did not answer.
      const server = refreshed
        .get(target.accountId)
        ?.find((folder) => folderMatches(folder, target.accountId, target.folderId))
      settleCounts.get(target)!(
        server?.unread ?? target.previous,
        !!server,
        server ? refreshVersions.get(target.accountId) : undefined,
      )
    }
    throw failure.reason
  }
  for (const target of folderTargets.values()) {
    const server = refreshed
      .get(target.accountId)
      ?.find((folder) => folderMatches(folder, target.accountId, target.folderId))
    settleCounts.get(target)!(
      server?.unread ?? target.next,
      true,
      server ? refreshVersions.get(target.accountId) : undefined,
    )
  }
  // Reapply on success: a column reload that raced the write may have put the
  // pre-write rows and badge back over the optimistic clear.
  kanban$.threads[key].set(
    (kanban$.threads[key].get() ?? []).map((thread) =>
      previousById.has(thread.thread_id) ? { ...thread, unread: false, unread_count: 0 } : thread,
    ),
  )
  if (clearsBadge) kanban$.unreadCounts[key].set(0)
}

export async function markColumnAllRead(column: KanbanColumn) {
  try {
    await markColumnRead(column, requestRead)
  } catch {
    showToast(t('notification.markReadFailed'), 'error')
  }
}

export async function markBoardAllRead(boardId: string) {
  const columns = Array.from(
    new Map(getKanbanColumns(boardId).map((column) => [kanbanColumnKey(column), column])).values(),
  )
  const requests = new Map<string, Promise<void>>()
  const request: ReadRequest = (command, payload) => {
    // Reuse a unified folder write for a concrete column covering the same folder.
    // A saved unified search marks only its matches, so it never stands in.
    if (command === 'mail.markAllRead' && payload.account_id !== 'unified') {
      const account = accounts$.get().find((account) => account.id === payload.account_id)
      const folder = mail$.foldersByAccount[payload.account_id].get()?.find((folder) => folder.id === payload.folder_id)
      const role = folder?.role || (payload.folder_id.toLowerCase() === 'inbox' ? 'inbox' : '')
      const unified = columns.find(
        (column) =>
          column.accountId === 'unified' &&
          !column.query &&
          column.folderId.toLowerCase() === role &&
          !isUnifiedStarredColumn(column),
      )
      if (account && account.included_in_unified !== false && unified) {
        payload = { account_id: 'unified', folder_id: unified.folderId }
      }
    }
    const key = JSON.stringify([command, payload])
    let pending = requests.get(key)
    if (!pending) {
      pending = requestRead(command, payload)
      requests.set(key, pending)
    }
    return pending
  }
  const baseline = mail$.foldersByAccount.get()
  const results = await Promise.allSettled(columns.map((column) => markColumnRead(column, request, baseline)))
  if (results.some((result) => result.status === 'rejected')) {
    showToast(t('notification.markReadFailed'), 'error')
  }
}

// A feed item in the unified starred column: items of one feed share a
// thread_id, so it is told apart by its own id.
function isStarredFeedItem(thread: Message, column: KanbanColumn): boolean {
  return (
    isUnifiedStarredColumn(column) &&
    isRssAccount(
      accounts$.get().find((item) => item.id === thread.account_id),
      thread.account_id,
    )
  )
}

// Open a card the keyboard moved onto, the same way clicking it would. Feed
// rows in the starred column carry their full body and live in a reader tab,
// leaving the card conversation behind the tabs alone; everything else opens in
// the conversation pane, which means dropping any tab on screen — MessagePane
// renders the active tab over the conversation, so the old item would otherwise
// stay on screen while only the card highlight moved.
function openAdjacentKanbanCard(thread: Message, column: KanbanColumn, boardKey: string) {
  if (isStarredFeedItem(thread, column)) {
    openMessageTab(thread)
  } else {
    focusKanbanThreadFolder(thread.folder_id)
    compose$.activeTab.set('')
    ui$.selectedThread.set(thread.thread_id)
    kanban$.paneThreadId.set(thread.thread_id)
  }
  kanban$.paneColumnKey.set(boardKey)
  ui$.mobilePane.set('conversation')
}

/** The thread of the card selected from the keyboard, while it shows as
 * selected (no conversation or tab in the pane): what a bare action applies to. */
export function kanbanKeyboardThreadId(): string {
  if (kanban$.paneThreadId.peek() || compose$.activeTab.peek()) return ''
  return kanban$.keyboardCard.peek()?.threadId ?? ''
}

// The cards a column shows: its threads under the column's filters (or the
// board's), with what was read this session kept, as KanbanBoardColumn renders
// them. `keepId` stays in whatever the filters say.
function visibleKanbanColumnThreads(column: KanbanColumn, keepId?: string): Message[] {
  const sourceKey = kanbanColumnKey(column)
  const filterMode = kanban$.filters[sourceKey].get() ?? kanban$.globalFilter.get()
  const attachmentsOnly = kanban$.attachmentsOnly[sourceKey].get() ?? kanban$.globalAttachmentsOnly.get()
  const threads = kanban$.threads[sourceKey].get() ?? []
  return filterThreads(threads, filterMode, keepId, mail$.readThreads.get(), attachmentsOnly)
}

/** Where the keyboard selection goes when its card's thread is removed from
 * the board (archived, deleted): the next card in its column, or the previous
 * if it was last, as the open conversation does. Null when none is left. */
export function kanbanKeyboardCardAfterRemoving(card: KanbanCardRef): KanbanCardRef | null {
  const boardId = kanban$.activeBoardId.peek()
  const column = getKanbanColumns(boardId).find((item) => kanbanBoardColumnKey(boardId, item) === card.columnKey)
  if (!column) return null
  // The leaving card is kept in, to find its place among the cards on screen.
  const threads = visibleKanbanColumnThreads(column, card.threadId)
  const index = threads.findIndex((thread) =>
    card.itemId ? thread.id === card.itemId : thread.thread_id === card.threadId,
  )
  if (index === -1) return null
  // A feed's items share its thread, and all of them go with it.
  const others = (list: Message[]) => list.find((thread) => thread.thread_id !== card.threadId)
  const neighbour = others(threads.slice(index + 1)) ?? others(threads.slice(0, index).reverse())
  if (!neighbour) return null
  return {
    threadId: neighbour.thread_id,
    columnKey: card.columnKey,
    ...(isStarredFeedItem(neighbour, column) ? { itemId: neighbour.id } : {}),
  }
}

/** A card on the board: its thread, and the column (board column key) it sits
 * in. A starred feed item also carries its own id, as its feed's items share
 * the thread. */
export type KanbanCardRef = { threadId: string; columnKey: string; itemId?: string }

// Move through the visible cards of the active board, its current column first.
// This mirrors chat-list adjacent navigation. With a conversation or tab in the
// pane it steps from whatever the pane shows and opens the next card; with none,
// it only moves the selection (kanban$.keyboardCard), from `from` (the focused
// card) or the selected card, and Return opens it. With neither, it starts at
// the first column's end.
export function selectAdjacentKanbanThread(delta: number, from?: KanbanCardRef) {
  const paneOpen = !!(kanbanPaneThreadId() || compose$.activeTab.get())
  const open = kanbanPaneThreadId() || ui$.selectedThread.get()
  const origin = open ? undefined : (from ?? kanban$.keyboardCard.get() ?? undefined)
  const step = (thread: Message, column: KanbanColumn, boardKey: string) => {
    if (paneOpen) openAdjacentKanbanCard(thread, column, boardKey)
    else
      kanban$.keyboardCard.set({
        threadId: thread.thread_id,
        columnKey: boardKey,
        ...(isStarredFeedItem(thread, column) ? { itemId: thread.id } : {}),
      })
  }
  const selected = open || origin?.threadId || ''
  // Items of one feed share a thread_id, so a reader tab's card, or a selected
  // feed item, is found by the item's own id first.
  const activeTab = compose$.activeTab.get()
  const selectedId =
    compose$.tabs.get().find((tab) => tab.id === activeTab && tab.kind === 'reader')?.messageId ?? origin?.itemId
  const indexOfSelected = (list: Message[]) => {
    const byId = selectedId ? list.findIndex((thread) => thread.id === selectedId) : -1
    return byId !== -1 ? byId : list.findIndex((thread) => thread.thread_id === selected)
  }
  const boardId = kanban$.activeBoardId.get()
  const columns = getKanbanColumns(boardId)
  const allThreads = kanban$.threads.get()
  const preferredKey = origin?.columnKey || kanban$.paneColumnKey.get()
  const orderedColumns = preferredKey
    ? [
        ...columns.filter((column) => kanbanBoardColumnKey(boardId, column) === preferredKey),
        ...columns.filter((column) => kanbanBoardColumnKey(boardId, column) !== preferredKey),
      ]
    : columns

  // A minimized column doesn't render its cards, so the selection (which only
  // shows on a card) skips it; opening a card shows it in the pane instead.
  const minimized = settings$.kanbanMinimizedColumns.get()

  let fallback: { key: string; thread: Message; column: KanbanColumn } | undefined
  for (const column of orderedColumns) {
    const sourceKey = kanbanColumnKey(column)
    const boardKey = kanbanBoardColumnKey(boardId, column)
    if (!paneOpen && minimized[boardKey]) continue
    const rawThreads = allThreads[sourceKey] ?? []
    const list = visibleKanbanColumnThreads(column, selected)
    if (!fallback && list.length > 0) {
      fallback = { key: boardKey, thread: delta >= 0 ? list[0] : list[list.length - 1], column }
    }
    if (!selected || !rawThreads.some((thread) => thread.thread_id === selected)) continue

    const current = indexOfSelected(list)
    if (current === -1) continue

    const next = Math.min(list.length - 1, Math.max(0, current + delta))
    const target = list[next]
    if (!target || next === current) return

    step(target, column, boardKey)
    return
  }

  if (fallback) {
    step(fallback.thread, fallback.column, fallback.key)
  }
}
