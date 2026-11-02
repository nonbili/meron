import type { KanbanBoardColumn } from '../states/settings'
import type { Folder } from '../types'
import { UNIFIED_ACCOUNT, accountFolderForRole, isUnifiedStarred, unifiedFolderRole } from './unifiedFolders'

/**
 * Unread messages on a board, for its rail badge: the sum over the folders its
 * columns show, read from the cached folder counts so a board that isn't open
 * gets a number too. A unified column stands for that role's folder in every
 * account of the unified view. Each folder counts once, however many columns
 * show it. Saved search columns and the unified Starred column are left out:
 * no folder count says how many of their cards are unread.
 */
export function boardUnreadCount(
  columns: KanbanBoardColumn[],
  foldersByAccount: Record<string, Folder[] | undefined>,
  unifiedAccountIds: string[],
): number {
  const counted = new Set<string>()
  let total = 0
  const add = (accountId: string, folderId: string | undefined) => {
    if (!folderId) return
    const folders = foldersByAccount[accountId]
    const wanted = folderId.toLowerCase() === 'inbox' ? accountFolderForRole(folders, 'inbox') : folderId
    const folder = folders?.find((item) => item.id === wanted)
    if (!folder) return
    const key = `${accountId}\n${folder.id}`
    if (counted.has(key)) return
    counted.add(key)
    total += folder.unread
  }
  for (const column of columns) {
    if (column.query || isUnifiedStarred(column.accountId, column.folderId.toLowerCase())) continue
    if (column.accountId === UNIFIED_ACCOUNT) {
      const role = unifiedFolderRole(column.folderId.toLowerCase())
      for (const accountId of unifiedAccountIds) add(accountId, accountFolderForRole(foldersByAccount[accountId], role))
    } else {
      add(column.accountId, column.folderId)
    }
  }
  return total
}
