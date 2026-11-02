import { folderMatches } from '../states/mailFolders'
import type { KanbanBoard, KanbanBoardColumn } from '../states/settings'
import type { Account, Folder } from '../types'
import { isRSSAccount } from './kanbanData'
import { UNIFIED_ACCOUNT, accountFolderForRole, isUnifiedStarred, unifiedFolderRole } from './unifiedFolders'

type BadgeBoard = Pick<KanbanBoard, 'columns' | 'showUnreadBadge'>
type BadgeAccount = Pick<Account, 'id' | 'provider' | 'auth_type' | 'paused' | 'included_in_unified'>

// Saved search columns and the unified Starred column have no folder count that
// says how many of their cards are unread.
function hasFolderCount(column: KanbanBoardColumn): boolean {
  return !column.query && !isUnifiedStarred(column.accountId, column.folderId.toLowerCase())
}

/**
 * The folders behind a board's counted columns, each once however many columns
 * show it. A unified column stands for that role's folder in every account of
 * the unified view.
 */
function boardColumnFolders(
  columns: KanbanBoardColumn[],
  foldersByAccount: Record<string, Folder[] | undefined>,
  unifiedAccountIds: string[],
): { accountId: string; folder: Folder }[] {
  const found = new Map<string, { accountId: string; folder: Folder }>()
  const add = (accountId: string, folderId: string | undefined) => {
    const folder = foldersByAccount[accountId]?.find((item) => folderMatches(item, accountId, folderId))
    if (folder) found.set(`${accountId}\n${folder.id}`, { accountId, folder })
  }
  for (const column of columns.filter(hasFolderCount)) {
    if (column.accountId === UNIFIED_ACCOUNT) {
      const role = unifiedFolderRole(column.folderId.toLowerCase())
      for (const accountId of unifiedAccountIds) add(accountId, accountFolderForRole(foldersByAccount[accountId], role))
    } else {
      add(column.accountId, column.folderId)
    }
  }
  return [...found.values()]
}

/**
 * A board's rail badge: the total of its folder columns' unread counts, read
 * from the cached folder lists so a board that isn't open gets a number too. The
 * counts are per folder, as in the column headers, so a Gmail message under two
 * labels the board shows counts in both.
 */
export function boardUnreadCount(
  columns: KanbanBoardColumn[],
  foldersByAccount: Record<string, Folder[] | undefined>,
  unifiedAccountIds: string[],
): number {
  return boardColumnFolders(columns, foldersByAccount, unifiedAccountIds).reduce(
    (total, { folder }) => total + folder.unread,
    0,
  )
}

/** The accounts whose folder counts the badges of `boards` read. */
export function boardBadgeAccountIds(boards: BadgeBoard[], unifiedAccountIds: string[]): string[] {
  const columns = boards
    .filter((board) => board.showUnreadBadge)
    .flatMap((board) => board.columns.filter(hasFolderCount))
  return [
    ...new Set(
      columns.flatMap((column) => (column.accountId === UNIFIED_ACCOUNT ? unifiedAccountIds : [column.accountId])),
    ),
  ]
}

/**
 * The folders a badge needs synced on a timer. Nothing else keeps a folder's
 * count current while its board is closed, except for inboxes and the folders
 * opted in to notifications, which the core already watches. Paused and RSS
 * accounts have no mailbox to sync.
 */
export function boardBadgeSyncTargets(
  boards: BadgeBoard[],
  foldersByAccount: Record<string, Folder[] | undefined>,
  accounts: BadgeAccount[],
): { account_id: string; folder: string }[] {
  const unifiedAccountIds = accounts.filter((account) => account.included_in_unified !== false).map(({ id }) => id)
  const syncable = new Set(
    accounts.filter((account) => !account.paused && !isRSSAccount(account.id, accounts)).map(({ id }) => id),
  )
  const columns = boards.filter((board) => board.showUnreadBadge).flatMap((board) => board.columns)
  return boardColumnFolders(columns, foldersByAccount, unifiedAccountIds)
    .filter(({ accountId, folder }) => {
      const isInbox = folderMatches(folder, accountId, 'inbox')
      return syncable.has(accountId) && !isInbox && !folder.notify
    })
    .map(({ accountId, folder }) => ({ account_id: accountId, folder: folder.id }))
}
