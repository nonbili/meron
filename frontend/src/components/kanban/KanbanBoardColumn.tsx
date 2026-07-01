import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties, MouseEvent as ReactMouseEvent } from 'react'
import { useTranslation } from '../../lib/i18n'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Loader2, Minus, Pause } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { clsx } from '../../lib/utils'
import { accounts$ } from '../../states/accounts'
import { isDraftFolder, mail$ } from '../../states/mail'
import {
  clearBulkSelection,
  isWailsDesktopRuntime,
  setBulkSelection,
  toggleBulkSelection,
  ui$,
  type BulkSelectionItem,
} from '../../states/ui'
import {
  kanbanBoardColumnKey,
  kanbanColumnKey,
  kanban$,
  markColumnAllRead,
  removeKanbanColumn,
  type KanbanColumn,
} from '../../states/kanban'
import { settings$ } from '../../states/settings'
import { filterThreads, isRssAccount } from '../../lib/threadActions'
import {
  accountLabel,
  columnDropTargetClass,
  columnEmptyText,
  columnSearchActive,
  columnSearchHighlightClass,
  folderLabel,
  loadKanbanColumn,
  loadMoreKanbanColumn,
  syncKanbanColumn,
  mergeLabelFolders,
  refreshKanbanContextAction,
  isUnifiedStarredColumn,
  kanbanColumnUnreadCount,
} from '../../lib/kanbanData'
import { openThreadTab } from '../../states/compose'
import { ThreadActionsMenu, ThreadActionsMenuItems } from '../threads/ThreadActionsMenu'
import { ThreadContextMenu, type ThreadContextMenuController } from '../threads/ThreadContextMenu'
import { Avatar } from '../avatar/Avatar'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { KanbanThreadCard } from './KanbanThreadCard'
import { KanbanColumnMinimized } from './KanbanColumnMinimized'
import { BulkActionBar } from '../threads/BulkActionBar'

// A board column is both a dnd-kit sortable item and a drop target; this bundles
// the wiring the column renderers need from those hooks.
export type ColumnWrapper = {
  setNodeRef: (node: HTMLElement | null) => void
  scrollIntoView: () => void
  isOver: boolean
  style?: CSSProperties
  // Header drag props for reordering.
  dragHandle?: Record<string, unknown>
}

function KanbanColumnContent({
  boardId,
  column,
  wrapper,
  onMoveThread,
  onSearchColumn,
  threadMenu,
}: {
  boardId: string
  column: KanbanColumn
  wrapper: ColumnWrapper
  onMoveThread: (threadId: string, source: KanbanColumn, target: KanbanColumn) => void
  onSearchColumn: (column: KanbanColumn) => void
  threadMenu: ThreadContextMenuController
}) {
  const { t } = useTranslation()
  const key = kanbanColumnKey(column)
  const boardKey = kanbanBoardColumnKey(boardId, column)
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const allThreads = useValue(kanban$.threads)
  const allLoading = useValue(kanban$.loading)
  const allLoadingMore = useValue(kanban$.loadingMore)
  const allCursors = useValue(kanban$.cursors)
  const allAccountCursors = useValue(kanban$.accountCursors)
  const allFilters = useValue(kanban$.filters)
  const globalFilter = useValue(kanban$.globalFilter)
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)
  const system = useValue(ui$.system)
  const bulkSelection = useValue(ui$.bulkSelection)
  const width = useValue(settings$.kanbanColumnWidth)
  const minimizedColumns = useValue(settings$.kanbanMinimizedColumns)
  const overWallpaper = !!useValue(settings$.kanbanBoards).find((board) => board.id === boardId)?.wallpaper
  const readThreads = useValue(mail$.readThreads)
  const labelFolders = useMemo(() => mergeLabelFolders(folders, foldersByAccount), [folders, foldersByAccount])
  const rawThreads = allThreads[key] ?? []
  // A column's own filter, once set, wins over the board-wide switch; the global
  // filter is only the default for columns the user hasn't touched.
  const filterMode = allFilters[key] ?? globalFilter
  // Keep a card visible (in place) only when opening it just changed its state —
  // e.g. selecting an unread card marks it read (tracked in readThreads) — not
  // merely because it's the open thread. So switching to Unread/Starred yields a
  // clean filtered list instead of pinning the currently-open conversation.
  const threads = filterThreads(rawThreads, filterMode, undefined, readThreads)
  const unreadCount = kanbanColumnUnreadCount(column, foldersByAccount, accounts, rawThreads)
  const hasUnread = unreadCount > 0
  const loading = allLoading[key] ?? false
  const loadingMore = allLoadingMore[key] ?? false
  const searchActive = columnSearchActive(key, searchQuery, searchScope)
  const starredColumn = isUnifiedStarredColumn(column)
  const hasMore = starredColumn
    ? false
    : column.accountId === 'unified'
      ? Object.keys(allAccountCursors[key] ?? {}).length > 0
      : !!(allCursors[key] ?? '')
  const minimized = !!minimizedColumns[boardKey]
  const columnAccount =
    column.accountId !== 'unified' ? accounts.find((account) => account.id === column.accountId) : undefined
  const columnAccountLabel = columnAccount ? columnAccount.display_name || columnAccount.email || columnAccount.id : ''
  const isRss = isRssAccount(columnAccount, column.accountId)
  const isPaused = !!columnAccount?.paused
  const emptyText = columnEmptyText(filterMode, searchActive, rawThreads.length > 0, isRss)
  const [syncing, setSyncing] = useState(false)
  const [headerMenu, setHeaderMenu] = useState<{ x: number; y: number } | null>(null)
  const desktopBulk = isWailsDesktopRuntime() || !!system
  const bulkItems = Object.values(bulkSelection)
  const bulkInColumn =
    desktopBulk && bulkItems.length > 0 && bulkItems.every((item) => item.groupKey === `kanban:${boardKey}`)

  const bulkItemFor = (thread: (typeof threads)[number]): BulkSelectionItem => {
    const starredFeed =
      starredColumn &&
      isRssAccount(
        accounts.find((acc) => acc.id === thread.account_id),
        thread.account_id,
      )
    const rowId = starredColumn ? thread.id : thread.thread_id
    return {
      key: `kanban:${boardKey}:${rowId}`,
      groupKey: `kanban:${boardKey}`,
      threadId: thread.thread_id,
      messageId: starredColumn && !starredFeed ? thread.id : undefined,
      accountId: thread.account_id,
      folderId: thread.folder_id,
      surface: 'kanban',
      kind: isRss || starredFeed ? 'feed' : 'mail',
      unread: thread.unread,
      starred: thread.starred,
      draft: !starredColumn && isDraftFolder(thread.folder_id),
      trash: labelFolders.some(
        (folder) =>
          folder.account_id === thread.account_id && folder.id === thread.folder_id && folder.role === 'trash',
      ),
    }
  }

  const selectRangeTo = (target: BulkSelectionItem) => {
    const anchor = ui$.bulkAnchorKey.peek()
    const rows = threads.map(bulkItemFor)
    const targetIndex = rows.findIndex((item) => item.key === target.key)
    const anchorIndex = rows.findIndex((item) => item.key === anchor)
    if (targetIndex === -1 || anchorIndex === -1) {
      toggleBulkSelection(target)
      return
    }
    const [from, to] = targetIndex < anchorIndex ? [targetIndex, anchorIndex] : [anchorIndex, targetIndex]
    setBulkSelection(rows.slice(from, to + 1), target.key)
  }

  useEffect(() => {
    void loadKanbanColumn(column, true)
  }, [column.accountId, column.folderId, filterMode])

  const openHeaderMenu = (event: ReactMouseEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.stopPropagation()
    setHeaderMenu({ x: event.clientX, y: event.clientY })
  }

  if (minimized) {
    return <KanbanColumnMinimized boardId={boardId} column={column} wrapper={wrapper} />
  }

  return (
    <section
      ref={wrapper.setNodeRef}
      style={{ width, ...wrapper.style }}
      className={clsx(
        'relative flex h-full shrink-0 flex-col rounded-lg border transition-colors',
        columnSearchHighlightClass(searchActive, overWallpaper),
        columnDropTargetClass(wrapper.isOver),
      )}
    >
      {bulkInColumn ? (
        <BulkActionBar items={bulkItems} className="min-h-12 rounded-t-lg border-b border-border bg-transparent" />
      ) : (
        <div
          className={`flex h-12 shrink-0 items-center gap-2 border-b border-border px-3 ${
            wrapper.dragHandle ? 'cursor-grab touch-none active:cursor-grabbing' : ''
          }`}
          title={wrapper.dragHandle ? t('kanban.actions.dragToReorderColumn') : undefined}
          onClick={wrapper.scrollIntoView}
          onContextMenu={openHeaderMenu}
          {...wrapper.dragHandle}
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
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <h3 className={clsx('truncate text-xs font-bold', isPaused ? 'text-secondary' : 'text-primary')}>
              {folderLabel(column, labelFolders, accounts)}
            </h3>
            {searchActive && loading && <Loader2 size={13} className="shrink-0 animate-spin text-accent" />}
            {unreadCount > 0 && (
              <span className="h-4.5 min-w-4.5 px-1.5 flex items-center justify-center rounded-full bg-accent text-white text-[10px] font-bold shadow-sm shadow-accent/20 leading-none shrink-0">
                {unreadCount}
              </span>
            )}
          </div>
          <div
            className="flex shrink-0 items-center gap-1"
            // Blank title stops the header's "Drag to reorder column" tooltip from
            // leaking onto the action buttons and the open dropdown menu.
            title=""
            onClick={(event) => event.stopPropagation()}
            onPointerDown={(event) => event.stopPropagation()}
            onContextMenu={(event) => event.stopPropagation()}
          >
            <IconButton
              icon={Minus}
              iconSize={15}
              label={t('kanban.actions.minimize')}
              size="sm"
              radius="lg"
              onClick={() => settings$.kanbanMinimizedColumns[boardKey].set(true)}
            />
            <ThreadActionsMenu
              filterMode={filterMode}
              onFilterChange={(mode) => {
                mail$.readThreads.set({})
                kanban$.filters[key].set(mode)
              }}
              hasUnread={hasUnread}
              onMarkAllRead={() => void markColumnAllRead(column)}
              onSync={async () => {
                setSyncing(true)
                try {
                  await syncKanbanColumn(column)
                } finally {
                  setSyncing(false)
                }
              }}
              syncing={syncing}
              syncLabel={isRss ? t('feeds.actions.syncFeeds') : undefined}
              syncingLabel={isRss ? t('feeds.actions.syncingFeeds') : undefined}
              onRemove={() => removeKanbanColumn(boardId, column)}
              onSearch={() => onSearchColumn(column)}
              searchLabel={t('kanban.actions.search', { defaultValue: 'Search' })}
              size={14}
              triggerClassName="h-7 w-7"
            />
          </div>
        </div>
      )}
      {headerMenu && (
        <FloatingContextMenu
          x={headerMenu.x}
          y={headerMenu.y}
          offset={4}
          onClose={() => setHeaderMenu(null)}
          overlay
          className="fixed z-50 min-w-[176px] rounded-xl border border-border bg-chats p-1 shadow-2xl animate-fade-in text-primary"
          onContextMenu={(event) => {
            event.preventDefault()
            event.stopPropagation()
          }}
        >
          <ThreadActionsMenuItems
            filterMode={filterMode}
            onFilterChange={(mode) => {
              mail$.readThreads.set({})
              kanban$.filters[key].set(mode)
            }}
            hasUnread={hasUnread}
            onMarkAllRead={() => void markColumnAllRead(column)}
            onSync={async () => {
              setSyncing(true)
              try {
                await syncKanbanColumn(column)
              } finally {
                setSyncing(false)
              }
            }}
            syncing={syncing}
            syncLabel={isRss ? t('feeds.actions.syncFeeds') : undefined}
            syncingLabel={isRss ? t('feeds.actions.syncingFeeds') : undefined}
            onRemove={() => removeKanbanColumn(boardId, column)}
            onSearch={() => onSearchColumn(column)}
            searchLabel={t('kanban.actions.search', { defaultValue: 'Search' })}
            closeMenu={() => setHeaderMenu(null)}
          />
        </FloatingContextMenu>
      )}
      <div
        data-thread-list
        className="flex-1 space-y-2 overflow-y-auto p-2"
        onScroll={(event) => {
          if (searchActive || !hasMore || loadingMore) return
          const el = event.currentTarget
          if (el.scrollHeight - el.scrollTop - el.clientHeight < 240) {
            void loadMoreKanbanColumn(column)
          }
        }}
      >
        {threads.length === 0 ? (
          <div className="py-8 text-center text-xs font-medium text-secondary">
            {loading ? t('common.loading') : emptyText}
          </div>
        ) : (
          <>
            {threads.map((thread) => {
              const bulkItem = bulkItemFor(thread)
              return (
                <KanbanThreadCard
                  key={starredColumn ? thread.id : thread.thread_id}
                  boardId={boardId}
                  thread={thread}
                  column={column}
                  threadMenu={threadMenu}
                  ownerKey={boardKey}
                  bulkSelectable={desktopBulk && bulkInColumn}
                  bulkEnabled={desktopBulk}
                  bulkSelected={!!bulkSelection[bulkItem.key]}
                  onBulkRangeSelect={() => selectRangeTo(bulkItem)}
                  onBulkModeSelect={() => toggleBulkSelection(bulkItem)}
                  onBulkPlainSelect={() => clearBulkSelection()}
                  bulkItem={bulkItem}
                />
              )
            })}
            {loadingMore && (
              <div className="py-3 text-center text-xs font-medium text-secondary">{t('common.loading')}</div>
            )}
          </>
        )}
      </div>
      {/* The controller is shared across columns; only render the menu for the
          column that opened it so it doesn't appear in every column at once. */}
      {threadMenu.menu?.ownerKey === boardKey && (
        <ThreadContextMenu
          controller={threadMenu}
          onSelectThread={(threadId) => {
            const thread = threads.find((item) => item.thread_id === threadId)
            if (thread) toggleBulkSelection(bulkItemFor(thread))
          }}
          onOpenThread={(threadId) => {
            const thread = threads.find((item) => item.thread_id === threadId)
            if (!thread) return
            openThreadTab(thread)
            kanban$.paneThreadId.set(threadId)
            kanban$.paneColumnKey.set(boardKey)
          }}
          onAfterAction={(action, _threadId, detail) => {
            void refreshKanbanContextAction(column, action, detail, searchActive ? searchQuery : '')
          }}
          // Single-account columns move optimistically (and revert on failure) via
          // the shared mover. Unified columns fall back to the default move path,
          // which resolves the thread's real account.
          onMove={
            column.accountId === 'unified'
              ? undefined
              : (threadId, targetAccountId, targetFolderId) =>
                  onMoveThread(threadId, column, { accountId: targetAccountId, folderId: targetFolderId })
          }
        />
      )}
    </section>
  )
}

// A board column: both a thread drop target and a sortable item so the header
// grip can drag it to a new slot.
export function SortableColumn({
  boardId,
  column,
  onMoveThread,
  onSearchColumn,
  threadMenu,
}: {
  boardId: string
  column: KanbanColumn
  onMoveThread: (threadId: string, source: KanbanColumn, target: KanbanColumn) => void
  onSearchColumn: (column: KanbanColumn) => void
  threadMenu: ThreadContextMenuController
}) {
  const key = kanbanBoardColumnKey(boardId, column)
  const { setNodeRef, attributes, listeners, transform, transition, isDragging, isOver } = useSortable({
    id: key,
    data: { type: 'column', column },
  })
  const nodeRef = useRef<HTMLElement | null>(null)
  const setColumnNodeRef = useCallback(
    (node: HTMLElement | null) => {
      nodeRef.current = node
      setNodeRef(node)
    },
    [setNodeRef],
  )
  const scrollColumnIntoView = useCallback(() => {
    nodeRef.current?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [])
  const style: CSSProperties = {
    transform: CSS.Translate.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : undefined,
    zIndex: isDragging ? 30 : undefined,
  }
  return (
    <KanbanColumnContent
      boardId={boardId}
      column={column}
      wrapper={{
        setNodeRef: setColumnNodeRef,
        scrollIntoView: scrollColumnIntoView,
        isOver,
        style,
        dragHandle: { ...attributes, ...listeners },
      }}
      onMoveThread={onMoveThread}
      onSearchColumn={onSearchColumn}
      threadMenu={threadMenu}
    />
  )
}
