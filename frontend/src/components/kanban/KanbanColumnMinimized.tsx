import { useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Loader2, Pause } from 'lucide-react'
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
  kanbanColumnUnreadCount,
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
  const unreadCount = useValue(kanban$.unreadCounts)[key]
  const loading = useValue(kanban$.loading)[key] ?? false
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)

  const labelFolders = useMemo(() => mergeLabelFolders(folders, foldersByAccount), [folders, foldersByAccount])
  const searchActive = columnSearchActive(key, searchQuery, searchScope)
  const overWallpaper = !!useValue(settings$.kanbanBoards).find((board) => board.id === boardId)?.wallpaper
  const columnUnreadCount = kanbanColumnUnreadCount(column, unreadCount, rawThreads)
  const columnAccount = column.accountId !== 'unified' ? accounts.find((a) => a.id === column.accountId) : undefined
  const columnAccountLabel = columnAccount ? columnAccount.display_name || columnAccount.email || columnAccount.id : ''
  const isPaused = !!columnAccount?.paused
  const label = folderLabel(column, labelFolders, accounts)

  // WebKitGTK (Wails on Linux) can mis-measure the intrinsic size of vertical-rl
  // text, and horizontal and vertical glyph advances are not necessarily equal.
  // Keep the label in the browser's ordinary horizontal text layout and rotate it
  // visually instead. The outer box still needs the horizontal text width as its
  // preferred height so flexbox can make room for the rotated label.
  const measureRef = useRef<HTMLSpanElement | null>(null)
  const labelBoxRef = useRef<HTMLDivElement | null>(null)
  const [labelHeight, setLabelHeight] = useState<number>()
  const [labelMaxWidth, setLabelMaxWidth] = useState<number>()
  useLayoutEffect(() => {
    const measure = measureRef.current
    const labelBox = labelBoxRef.current
    if (!measure || !labelBox) return
    const update = () => {
      const preferredHeight = Math.ceil(measure.getBoundingClientRect().width)
      setLabelHeight(preferredHeight)
      setLabelMaxWidth(labelBox.clientHeight || preferredHeight)
    }
    update()
    const observer = new ResizeObserver(update)
    observer.observe(measure)
    observer.observe(labelBox)
    return () => observer.disconnect()
  }, [])

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
      <div className="relative shrink-0">
        <Avatar
          name={columnAccountLabel || accountLabel(column.accountId, accounts)}
          email={columnAccount?.email}
          src={columnAccount?.avatar_url}
          size={26}
          className={isPaused ? 'grayscale opacity-40' : undefined}
        />
        {isPaused && (
          <span
            className="absolute -bottom-1 -right-1 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-black/60 text-white/80 ring-2 ring-chats"
            title={t('settings.account.paused', { defaultValue: 'Paused' })}
          >
            <Pause size={7} className="fill-current" />
          </span>
        )}
      </div>
      <span ref={measureRef} aria-hidden className="invisible absolute whitespace-nowrap text-xs font-bold">
        {label}
      </span>
      <div ref={labelBoxRef} className="relative min-h-0 w-full shrink overflow-hidden" style={{ height: labelHeight }}>
        <span
          className={clsx(
            'absolute top-0 block truncate whitespace-nowrap text-xs font-bold leading-4',
            isPaused ? 'text-secondary' : 'text-primary',
          )}
          style={{
            left: 'calc(50% + 0.5rem)',
            width: labelMaxWidth,
            transform: 'rotate(90deg)',
            transformOrigin: 'top left',
            visibility: labelMaxWidth === undefined ? 'hidden' : undefined,
          }}
        >
          {label}
        </span>
      </div>
      {columnUnreadCount > 0 && (
        <div className="h-4.5 min-w-4.5 px-1.5 flex items-center justify-center rounded-full bg-accent text-white text-[10px] font-bold shadow-sm shadow-accent/20 leading-none shrink-0">
          {columnUnreadCount}
        </div>
      )}
      {searchActive && loading && <Loader2 size={14} className="animate-spin text-accent" />}
    </section>
  )
}
