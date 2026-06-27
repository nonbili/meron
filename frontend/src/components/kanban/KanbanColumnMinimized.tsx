import { useMemo } from 'react'
import { Loader2 } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { accounts$ } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { kanbanBoardColumnKey, kanbanColumnKey, kanban$, type KanbanColumn } from '../../states/kanban'
import {
  KANBAN_COLUMN_MINIMIZED_WIDTH,
  accountLabel,
  columnDropTargetClass,
  columnSearchActive,
  columnSearchHighlightClass,
  folderLabel,
  mergeLabelFolders,
} from '../../lib/kanbanData'
import { Avatar } from '../avatar/Avatar'
import type { ColumnWrapper } from './KanbanBoardColumn'

// Collapsed column: a vertical strip showing the account avatar, folder name and
// unread badge. Clicking it (or any unminimize) expands back to the full column.
export function KanbanColumnMinimized({
  boardId,
  column,
  wrapper,
}: {
  boardId: string
  column: KanbanColumn
  wrapper: ColumnWrapper
}) {
  const { t } = useTranslation()
  const key = kanbanColumnKey(column)
  const boardKey = kanbanBoardColumnKey(boardId, column)
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const rawThreads = useValue(kanban$.threads)[key] ?? []
  const loading = useValue(kanban$.loading)[key] ?? false
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)

  const labelFolders = useMemo(() => mergeLabelFolders(folders, foldersByAccount), [folders, foldersByAccount])
  const searchActive = columnSearchActive(key, searchQuery, searchScope)
  const overWallpaper = !!useValue(settings$.kanbanBoards).find((board) => board.id === boardId)?.wallpaper
  const unreadCount = rawThreads.reduce((count, thread) => count + (thread.unread ? 1 : 0), 0)
  const columnAccount = column.accountId !== 'unified' ? accounts.find((a) => a.id === column.accountId) : undefined
  const columnAccountLabel = columnAccount ? columnAccount.display_name || columnAccount.email || columnAccount.id : ''

  return (
    <section
      ref={wrapper.setNodeRef}
      style={{ width: KANBAN_COLUMN_MINIMIZED_WIDTH, ...wrapper.style }}
      className={clsx(
        'relative flex h-full shrink-0 flex-col items-center gap-3 rounded-lg border p-2 transition-colors',
        columnSearchHighlightClass(searchActive, overWallpaper),
        columnDropTargetClass(wrapper.isOver),
        wrapper.dragHandle ? 'cursor-grab touch-none active:cursor-grabbing' : 'cursor-pointer',
      )}
      title={t('kanban.actions.expand')}
      {...wrapper.dragHandle}
      onClick={() => {
        settings$.kanbanMinimizedColumns[boardKey].set(false)
        requestAnimationFrame(wrapper.scrollIntoView)
      }}
    >
      <Avatar
        name={columnAccountLabel || accountLabel(column.accountId, accounts)}
        email={columnAccount?.email}
        src={columnAccount?.avatar_url}
        size={26}
      />
      <div className="max-h-52 truncate text-xs font-bold text-primary" style={{ writingMode: 'vertical-rl' }}>
        {folderLabel(column, labelFolders, accounts)}
      </div>
      {unreadCount > 0 && (
        <div className="h-4.5 min-w-4.5 px-1.5 flex items-center justify-center rounded-full bg-accent text-white text-[10px] font-bold shadow-sm shadow-accent/20 leading-none shrink-0">
          {unreadCount}
        </div>
      )}
      {searchActive && loading && <Loader2 size={14} className="animate-spin text-accent" />}
    </section>
  )
}
