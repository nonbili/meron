import { useState } from 'react'
import type { DragEvent } from 'react'
import { Mail, SquareCheckBig, MoreHorizontal, EyeOff, SquarePen } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { formatShortcut, isMac, RAIL_SHORTCUT_IDS } from '../../lib/shortcuts'
import { DndContext, closestCenter, KeyboardSensor, PointerSensor, useSensor, useSensors } from '@dnd-kit/core'
import type { DragEndEvent } from '@dnd-kit/core'
import { SortableContext, sortableKeyboardCoordinates, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { accounts$, isSendableAccount, reorderAccountIds } from '../../states/accounts'
import { openComposeTab } from '../../states/compose'
import { moveFeed, RSS_FEED_DRAG_TYPE } from '../../states/feeds'
import { kanban$, openMailAccount, reorderKanbanBoards, selectKanbanBoard } from '../../states/kanban'
import { mail$ } from '../../states/mail'
import { inboxUnread } from '../../states/mailFolders'
import { settings$, setUnifiedInboxSideNavVisible } from '../../states/settings'
import { toggleTasksPanel } from '../../states/tasks'
import { ui$ } from '../../states/ui'
import { QuickSettingsMenu } from './QuickSettingsMenu'
import { SortableBoard, SortableAccount, railIndicator, railTileShape } from './SortableRailItems'
import { UnreadCountBadge } from './UnreadCountBadge'
import { RailContextMenu, RailMenuItem } from './RailContextMenu'
import { AccountContextMenu } from './AccountContextMenu'
import { BoardContextMenu } from './BoardContextMenu'
import { BoardDialog, type BoardDialogState } from './BoardDialog'
import { useTitleBar } from '../titlebar/TitleBar'
import type { Account } from '../../types'

// Every rail divider sits 9px from its neighbours, except the compose button's,
// which is tucked under the button to line up with the 48px header borders.
const railDivider = 'h-px w-8 shrink-0 bg-sidenav-ink/10'

export function SideNav() {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  useValue(settings$.shortcutOverrides)
  const tasksEnabled = useValue(settings$.tasksEnabled)
  const tasksPanelOpen = useValue(ui$.tasksPanelOpen)
  const boards = useValue(settings$.kanbanBoards)
  const hiddenSideNavAccounts = useValue(settings$.hiddenSideNavAccounts)
  const showUnifiedInbox = useValue(settings$.showUnifiedInboxInSideNav)
  const showUnreadBadge = useValue(settings$.showUnreadAccountBadge)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const selectedAccount = useValue(ui$.selectedAccount)
  const hasSendableAccount = accounts.some(isSendableAccount)
  // Tasks and the more menu live in Meron's own title bar when there is one.
  const titleBar = useTitleBar()
  const utilities = !titleBar
  // Right-click context menu anchored at the cursor for one account.
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const menuAccount = menu ? accounts.find((acc) => acc.id === menu.id) : null
  const [boardMenu, setBoardMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const menuBoard = boardMenu ? boards.find((board) => board.id === boardMenu.id) : null
  const [boardDialog, setBoardDialog] = useState<BoardDialogState | null>(null)
  const [unifiedMenu, setUnifiedMenu] = useState<{ x: number; y: number } | null>(null)
  // Bottom "more" menu holding the view switcher and theme settings.
  const [moreMenu, setMoreMenu] = useState<{ x: number; y: number } | null>(null)

  // Starred is a folder of the unified view, reachable from the column's folder
  // switcher, so the rail button covers it too.
  const isUnifiedActive = !activeBoardId && selectedAccount === 'unified'
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
  const railShortcut = (index: number) => {
    const id = RAIL_SHORTCUT_IDS[index]
    return id ? formatShortcut(id).join(isMac ? '' : '+') : undefined
  }

  const isRssAccount = (account: { provider: string; auth_type: string }) =>
    account.provider === 'rss' || account.auth_type === 'rss'

  function parseFeedDrag(event: DragEvent): { threadId: string; accountId?: string } | null {
    if (!Array.from(event.dataTransfer.types).includes(RSS_FEED_DRAG_TYPE)) return null
    try {
      const raw = event.dataTransfer.getData(RSS_FEED_DRAG_TYPE)
      const parsed = JSON.parse(raw) as { threadId?: string; accountId?: string }
      if (!parsed.threadId) return null
      return { threadId: parsed.threadId, accountId: parsed.accountId }
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

  const selectAccount = (id: string, folderId = 'inbox') => openMailAccount(id, folderId)

  return (
    <aside
      className={`flex w-[60px] shrink-0 flex-col items-center bg-sidenav px-0 pb-4 max-[768px]:hidden select-none ${titleBar ? 'pt-[5px]' : 'pt-[9px]'}`}
      onContextMenu={(event) => {
        if (event.defaultPrevented) return
        event.preventDefault()
        setMoreMenu({ x: event.clientX, y: event.clientY })
      }}
    >
      {hasSendableAccount && (
        <>
          <button
            className="relative isolate flex h-10 w-10 shrink-0 transform-gpu items-center justify-center overflow-hidden rounded-[20px] bg-accent text-accent-label transition-all duration-200 ease-out hover:rounded-2xl hover:bg-accent-hover hover:text-accent-hover-label cursor-pointer"
            onClick={() => openComposeTab()}
            title={`${t('composer.actions.newMessage')} (${formatShortcut('compose.new').join(isMac ? '' : '+')})`}
            aria-label={t('composer.actions.newMessage')}
          >
            <SquarePen size={18} />
          </button>
          {/* 4 + 40 + 3 aligns with the 48px header's bottom border, plus 1px
            for the content frame's border, and 4px more under the system
            title bar, where the frame stands clear of the top edge. */}
          <div className={`mt-[3px] ${railDivider}`} />
        </>
      )}
      <div className="flex min-h-0 w-full flex-1 flex-col items-center overflow-y-auto no-scrollbar py-[9px]">
        {/* Unified Inbox Home Button */}
        {showUnifiedInbox && (
          <div className="relative w-full flex justify-center group">
            {railIndicator(isUnifiedActive)}
            <div className="relative">
              <button
                className={`flex h-11 w-11 items-center justify-center cursor-pointer ${railTileShape(isUnifiedActive)} ${
                  isUnifiedActive
                    ? 'bg-accent text-accent-label'
                    : 'bg-sidenav-ink/10 text-sidenav-ink/60 group-hover:bg-sidenav-ink/20 group-hover:text-sidenav-ink'
                }`}
                onClick={() => selectAccount('unified')}
                onContextMenu={(event) => {
                  event.preventDefault()
                  event.stopPropagation()
                  setUnifiedMenu({ x: event.clientX, y: event.clientY })
                }}
                title={`${t('settings.sideNav.showUnifiedInbox')} (${railShortcut(0)})`}
              >
                <Mail size={19} />
              </button>
              <UnreadCountBadge count={unifiedUnread} />
            </div>
          </div>
        )}

        {showUnifiedInbox && (hasBoards || hasAccounts) && <div className={`my-[9px] ${railDivider}`} />}

        {/* Kanban Boards */}
        {hasBoards && (
          <div className="flex flex-col gap-3 w-full items-center">
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleBoardDragEnd}
              modifiers={[restrictToVerticalAxis]}
            >
              <SortableContext items={boards.map((board) => board.id)} strategy={verticalListSortingStrategy}>
                {boards.map((board, index) => (
                  <SortableBoard
                    key={board.id}
                    board={board}
                    shortcut={railShortcut(Number(showUnifiedInbox) + index)}
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

        {hasBoards && hasAccounts && <div className={`my-[9px] ${railDivider}`} />}

        {/* Accounts List */}
        {hasAccounts && (
          <div className="flex flex-col gap-3 w-full items-center">
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleDragEnd}
              modifiers={[restrictToVerticalAxis]}
            >
              <SortableContext items={sideNavAccounts.map((acc) => acc.id)} strategy={verticalListSortingStrategy}>
                {sideNavAccounts.map((account, index) => (
                  <SortableAccount
                    key={account.id}
                    account={account}
                    shortcut={railShortcut(Number(showUnifiedInbox) + boards.length + index)}
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

      {utilities && (showUnifiedInbox || hasBoards || hasAccounts) && <div className={`mb-[9px] ${railDivider}`} />}

      {/* Utilities */}
      {utilities && (
        <div className="flex flex-col gap-3 items-center">
          {tasksEnabled && (
            <button
              type="button"
              className="flex h-10 w-10 items-center justify-center rounded-xl bg-sidenav-ink/10 text-sidenav-ink/60 transition-colors hover:bg-sidenav-ink/20 hover:text-sidenav-ink cursor-pointer max-[900px]:hidden"
              onClick={toggleTasksPanel}
              title={t('tasks.title')}
              aria-label={t('tasks.title')}
              aria-pressed={tasksPanelOpen}
            >
              <SquareCheckBig size={18} />
            </button>
          )}
          <button
            className={`flex h-10 w-10 items-center justify-center rounded-xl transition-all duration-150 cursor-pointer ${
              moreMenu
                ? 'bg-sidenav-ink/20 text-sidenav-ink'
                : 'bg-sidenav-ink/10 text-sidenav-ink/60 hover:bg-sidenav-ink/20 hover:text-sidenav-ink'
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
      )}

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
