import { useCallback, useMemo } from 'react'
import { Rss } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { accounts$ } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import type { KanbanColumn } from '../../states/kanban'
import { accountLabel, folderLabel, mergeLabelFolders, useFoldersByAccount } from '../../lib/kanbanData'
import { isRssAccount } from '../../lib/threadActions'
import { AccountAvatar } from '../avatar/AccountAvatar'

/** A column's folder and account names, as the search scope shows them. A saved
 * search shares its folder's name, so its query tells the two apart. */
export function useColumnLabel() {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const folderList = useValue(mail$.folders)
  const foldersByAccount = useFoldersByAccount()
  const folders = useMemo(() => mergeLabelFolders(folderList, foldersByAccount), [folderList, foldersByAccount])
  return useCallback(
    (column: KanbanColumn) => {
      const account = column.accountId !== 'unified' ? accounts.find((a) => a.id === column.accountId) : undefined
      const folder = folderLabel(column, folders, accounts, t)
      return {
        folder: column.query ? `${folder} · ${column.query}` : folder,
        account:
          column.accountId === 'unified'
            ? t('accounts.unified')
            : account
              ? account.display_name || account.email || account.id
              : '',
      }
    },
    [accounts, folders, t],
  )
}

/** Whose folder a column shows, since a board can hold the same folder of several accounts. */
export function ColumnAvatar({ column, size }: { column: KanbanColumn; size: number }) {
  const accounts = useValue(accounts$)
  const account = column.accountId !== 'unified' ? accounts.find((a) => a.id === column.accountId) : undefined
  const rss = isRssAccount(account, column.accountId)
  return (
    <AccountAvatar
      accountId={column.accountId}
      name={account ? account.display_name || account.email || account.id : accountLabel(column.accountId, accounts)}
      src={account?.avatar_url}
      size={size}
      className="shrink-0"
      fallback={rss ? <Rss size={size * 0.55} /> : undefined}
    />
  )
}
