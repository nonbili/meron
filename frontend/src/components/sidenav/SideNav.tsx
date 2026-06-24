import { useState } from 'react'
import type { DragEvent } from 'react'
import { Mail, MoreHorizontal, EyeOff, Star } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { DndContext, closestCenter, KeyboardSensor, PointerSensor, useSensor, useSensors } from '@dnd-kit/core'
import type { DragEndEvent } from '@dnd-kit/core'
import { SortableContext, sortableKeyboardCoordinates, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { accounts$, reorderAccountIds } from '../../states/accounts'
import { moveFeed, RSS_FEED_DRAG_TYPE } from '../../states/feeds'
import { closeKanbanBoard, kanban$, reorderKanbanBoards, selectKanbanBoard } from '../../states/kanban'
import { mail$, inboxUnread } from '../../states/mail'
import { settings$, setUnifiedInboxSideNavVisible, setStarredSideNavVisible } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { QuickSettingsMenu } from './QuickSettingsMenu'
import { SortableBoard, SortableAccount } from './SortableRailItems'
import { UnreadCountBadge } from './UnreadCountBadge'
import { RailContextMenu, RailMenuItem } from './RailContextMenu'
import { AccountContextMenu } from './AccountContextMenu'
import { BoardContextMenu } from './BoardContextMenu'
import { BoardDialog, type BoardDialogState } from './BoardDialog'
import type { Account } from '../../types'

export function SideNav() {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const boards = useValue(settings$.kanbanBoards)
  const hiddenSideNavAccounts = useValue(settings$.hiddenSideNavAccounts)
  const showUnifiedInbox = useValue(settings$.showUnifiedInboxInSideNav)
  const showStarred = useValue(settings$.showStarredInSideNav)
  const showUnreadBadge = useValue(settings$.showUnreadAccountBadge)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const selectedAccount = useValue(ui$.selectedAccount)
  // Right-click context menu anchored at the cursor for one account.
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const menuAccount = menu ? accounts.find((acc) => acc.id === menu.id) : null
  const [boardMenu, setBoardMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const menuBoard = boardMenu ? boards.find((board) => board.id === boardMenu.id) : null
  const [boardDialog, setBoardDialog] = useState<BoardDialogState | null>(null)
  const [unifiedMenu, setUnifiedMenu] = useState<{ x: number; y: number } | null>(null)
  const [starredMenu, setStarredMenu] = useState<{ x: number; y: number } | null>(null)
  // Bottom "more" menu holding the view switcher and theme settings.
  const [moreMenu, setMoreMenu] = useState<{ x: number; y: number } | null>(null)

  const isUnifiedActive = !activeBoardId && selectedAccount === 'unified'
  const isStarredActive = !activeBoardId && selectedAccount === 'starred'
  const unifiedUnread = showUnreadBadge
    ? accounts.reduce(
        (sum, account) =>
          account.included_in_unified !== false ? sum + inboxUnread(foldersByAccount[account.id]) : sum,
        0,
      )
    : 0
  const hiddenSideNavAccountIds = new Set(hiddenSideNavAccounts)
  const sideNavAccounts = accounts.filter((account) => !hiddenSideNavAccountIds.has(account.id))
  const hasBoards = boards.length > 0
  const hasAccounts = sideNavAccounts.length > 0

  const isRssAccount = (account: { provider: string; auth_type: string }) =>
    account.provider === 'rss' || account.auth_type === 'rss'

  function parseFeedDrag(event: DragEvent) {
    if (!Array.from(event.dataTransfer.types).includes(RSS_FEED_DRAG_TYPE)) return null
    try {
      const raw = event.dataTransfer.getData(RSS_FEED_DRAG_TYPE)
      const parsed = JSON.parse(raw) as { threadId?: string; accountId?: string }
      if (!parsed.threadId) return null
      return parsed
    } catch {
      return null
    }
  }

  function handleFeedDragOver(event: DragEvent<HTMLDivElement>, account: Account) {
    if (!isRssAccount(account)) return
    const payload = parseFeedDrag(event)
    if (!payload || payload.accountId === account.id) return
    event.preventDefault()
    event.dataTransfer.dropEffect = 'move'
  }

  function handleFeedDrop(event: DragEvent<HTMLDivElement>, account: Account) {
    if (!isRssAccount(account)) return
    const payload = parseFeedDrag(event)
    if (!payload || payload.accountId === account.id) return
    event.preventDefault()
    event.stopPropagation()
    void moveFeed(payload.threadId, account.id)
  }

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event
    if (over && active.id !== over.id) {
      const visible = [...sideNavAccounts]
      const oldIndex = visible.findIndex((account) => account.id === active.id)
      const newIndex = visible.findIndex((account) => account.id === over.id)
      if (oldIndex === -1 || newIndex === -1) return
      const [removed] = visible.splice(oldIndex, 1)
      visible.splice(newIndex, 0, removed)
      const next = [...accounts]
      const visiblePositions = accounts.flatMap((account, index) =>
        hiddenSideNavAccountIds.has(account.id) ? [] : [index],
      )
      visiblePositions.forEach((position, index) => {
        next[position] = visible[index]
      })
      void reorderAccountIds(next.map((account) => account.id))
    }
  }

  function handleBoardDragEnd(event: DragEndEvent) {
    const { active, over } = event
    if (over && active.id !== over.id) {
      const oldIndex = boards.findIndex((board) => board.id === active.id)
      const newIndex = boards.findIndex((board) => board.id === over.id)
      reorderKanbanBoards(oldIndex, newIndex)
    }
  }

  const selectAccount = (id: string) => {
    closeKanbanBoard()
    ui$.selectedAccount.set(id)
    ui$.selectedFolder.set('inbox')
  }

  return (
    <aside
      className="flex w-[60px] shrink-0 flex-col items-center gap-4 border-r border-border bg-sidenav px-0 py-4 max-[768px]:hidden select-none"
      onContextMenu={(event) => {
        if (event.defaultPrevented) return
        event.preventDefault()
        setMoreMenu({ x: event.clientX, y: event.clientY })
      }}
    >
      <div className="flex min-h-0 w-full flex-1 flex-col items-center gap-4 overflow-y-auto pt-1.5">
        {/* Unified Inbox Home Button */}
        {showUnifiedInbox && (
          <div className="relative w-full flex justify-center group">
            <div
              className={`absolute left-0 top-1/2 -translate-y-1/2 w-1 rounded-r bg-accent transition-all duration-200 ${
                isUnifiedActive ? 'h-7' : 'h-0 group-hover:h-3'
              }`}
            />
            <div className="relative">
              <button
                className={`flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 cursor-pointer ${
                  isUnifiedActive
                    ? 'bg-accent text-white shadow-lg shadow-accent/25'
                    : 'bg-white/10 text-white/60 hover:bg-white/20 hover:text-white hover:scale-105'
                }`}
                onClick={() => selectAccount('unified')}
                onContextMenu={(event) => {
                  event.preventDefault()
                  event.stopPropagation()
                  setUnifiedMenu({ x: event.clientX, y: event.clientY })
                }}
                title={t('settings.sideNav.showUnifiedInbox')}
              >
                <Mail size={19} />
              </button>
              <UnreadCountBadge count={unifiedUnread} />
            </div>
          </div>
        )}

        {/* Starred view: every starred message / feed item across all accounts */}
        {showStarred && (
          <div className="relative w-full flex justify-center group">
            <div
              className={`absolute left-0 top-1/2 -translate-y-1/2 w-1 rounded-r bg-accent transition-all duration-200 ${
                isStarredActive ? 'h-7' : 'h-0 group-hover:h-3'
              }`}
            />
            <button
              className={`flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 cursor-pointer ${
                isStarredActive
                  ? 'bg-accent text-white shadow-lg shadow-accent/25'
                  : 'bg-white/10 text-white/60 hover:bg-white/20 hover:text-white hover:scale-105'
              }`}
              onClick={() => selectAccount('starred')}
              onContextMenu={(event) => {
                event.preventDefault()
                event.stopPropagation()
                setStarredMenu({ x: event.clientX, y: event.clientY })
              }}
              title={t('settings.sideNav.showStarred')}
            >
              <Star size={19} />
            </button>
          </div>
        )}

        {(showUnifiedInbox || showStarred) && (hasBoards || hasAccounts) && (
          <div className="h-px w-8 shrink-0 bg-white/10" />
        )}

        {/* Kanban Boards */}
        {hasBoards && (
          <div className="flex flex-col gap-3 w-full items-center py-1">
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleBoardDragEnd}
              modifiers={[restrictToVerticalAxis]}
            >
              <SortableContext items={boards.map((board) => board.id)} strategy={verticalListSortingStrategy}>
                {boards.map((board) => (
                  <SortableBoard
                    key={board.id}
                    board={board}
                    active={board.id === activeBoardId}
                    onSelect={() => selectKanbanBoard(board.id)}
                    onContextMenu={(e) => {
                      e.preventDefault()
                      e.stopPropagation()
                      setBoardMenu({ id: board.id, x: e.clientX, y: e.clientY })
                    }}
                  />
                ))}
              </SortableContext>
            </DndContext>
          </div>
        )}

        {hasBoards && hasAccounts && <div className="h-px w-8 shrink-0 bg-white/10" />}

        {/* Accounts List */}
        {hasAccounts && (
          <div className="flex flex-col gap-3 w-full items-center py-1">
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleDragEnd}
              modifiers={[restrictToVerticalAxis]}
            >
              <SortableContext items={sideNavAccounts.map((acc) => acc.id)} strategy={verticalListSortingStrategy}>
                {sideNavAccounts.map((account) => (
                  <SortableAccount
                    key={account.id}
                    account={account}
                    active={!activeBoardId && account.id === selectedAccount}
                    onSelect={() => selectAccount(account.id)}
                    onContextMenu={(e) => {
                      e.preventDefault()
                      e.stopPropagation()
                      setMenu({ id: account.id, x: e.clientX, y: e.clientY })
                    }}
                    onFeedDragOver={handleFeedDragOver}
                    onFeedDrop={handleFeedDrop}
                  />
                ))}
              </SortableContext>
            </DndContext>
          </div>
        )}
      </div>

      {(showUnifiedInbox || showStarred || hasBoards || hasAccounts) && (
        <div className="h-px w-8 shrink-0 bg-white/10" />
      )}

      {/* Utilities */}
      <div className="flex flex-col gap-3 items-center">
        <button
          className={`flex h-10 w-10 items-center justify-center rounded-xl transition-all duration-150 cursor-pointer ${
            moreMenu ? 'bg-white/20 text-white' : 'bg-white/10 text-white/60 hover:bg-white/20 hover:text-white'
          }`}
          onClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect()
            setMoreMenu({ x: rect.right + 8, y: rect.top })
          }}
          title={t('common.more')}
        >
          <MoreHorizontal size={18} />
        </button>
      </div>

      {/* Bottom "more" menu: view switcher + theme settings */}
      {moreMenu && (
        <QuickSettingsMenu
          anchor={{ x: moreMenu.x, y: moreMenu.y, placement: 'up' }}
          onAddKanbanBoard={() => setBoardDialog({ mode: 'create', name: t('kanban.board.defaultName') })}
          onClose={() => setMoreMenu(null)}
        />
      )}

      {unifiedMenu && (
        <RailContextMenu x={unifiedMenu.x} y={unifiedMenu.y} onClose={() => setUnifiedMenu(null)}>
          <RailMenuItem
            icon={<EyeOff size={13} className="text-secondary" />}
            label={t('sidenav.actions.hideFromSideNav')}
            onClick={() => {
              setUnifiedInboxSideNavVisible(false)
              setUnifiedMenu(null)
            }}
          />
        </RailContextMenu>
      )}

      {starredMenu && (
        <RailContextMenu x={starredMenu.x} y={starredMenu.y} onClose={() => setStarredMenu(null)}>
          <RailMenuItem
            icon={<EyeOff size={13} className="text-secondary" />}
            label={t('sidenav.actions.hideFromSideNav')}
            onClick={() => {
              setStarredSideNavVisible(false)
              setStarredMenu(null)
            }}
          />
        </RailContextMenu>
      )}

      {menu && menuAccount && (
        <AccountContextMenu account={menuAccount} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />
      )}

      {boardMenu && menuBoard && (
        <BoardContextMenu board={menuBoard} x={boardMenu.x} y={boardMenu.y} onClose={() => setBoardMenu(null)} />
      )}

      {boardDialog && (
        <BoardDialog state={boardDialog} onChange={setBoardDialog} onClose={() => setBoardDialog(null)} />
      )}
    </aside>
  )
}
