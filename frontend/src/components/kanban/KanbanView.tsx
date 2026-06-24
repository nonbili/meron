import { useEffect, useMemo, useRef, useState } from 'react'
import { closestCenter, DndContext, DragOverlay, PointerSensor, useSensor, useSensors } from '@dnd-kit/core'
import { SortableContext, horizontalListSortingStrategy } from '@dnd-kit/sortable'
import { Columns3, Search, SquarePen, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { invoke } from '../../lib/bridge'
import { accounts$, isSendableAccount } from '../../states/accounts'
import { ui$ } from '../../states/ui'
import { mail$, ensureAccountFolders } from '../../states/mail'
import { openComposeTab } from '../../states/compose'
import type { Folder } from '../../types'
import {
  addKanbanColumn,
  getKanbanColumns,
  kanbanBoardColumnKey,
  kanbanColumnKey,
  kanban$,
  removeKanbanColumn,
  setGlobalKanbanFilter,
  type KanbanColumn,
} from '../../states/kanban'
import { settings$ } from '../../states/settings'
import { EmptyState } from '../empty-state/EmptyState'
import { IconButton } from '../button/IconButton'
import { AddColumnDialog, type AccountGroup } from './AddColumnDialog'
import { SortableColumn } from './KanbanBoardColumn'
import { KanbanDragPreview } from './KanbanThreadCard'
import { useThreadContextMenu } from '../threads/ThreadContextMenu'
import { SearchScopeDropdown } from './SearchScopeDropdown'
import { BoardMenu, FilterSwitch } from './KanbanBoardMenu'
import { isRSSAccount, loadKanbanColumn } from '../../lib/kanbanData'
import { wallpaperCss } from '../../lib/wallpapers'
import { useKanbanBoardSync, useKanbanDnd } from './useKanbanBoard'

export function KanbanView({ boardId }: { boardId: string }) {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const boards = useValue(settings$.kanbanBoards)
  const globalFilter = useValue(kanban$.globalFilter)
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)
  const globalSearchFocus = useValue(ui$.globalSearchFocus)
  const board = boards.find((item) => item.id === boardId)
  // No layer at all when unset, so the default board keeps the plain theme surface.
  const boardWallpaper = board?.wallpaper ? wallpaperCss(board.wallpaper) : null
  const [dialogOpen, setDialogOpen] = useState(false)
  const [dialogGroups, setDialogGroups] = useState<AccountGroup[]>([])
  // The search bar is collapsed to an icon by default; it expands on click (or the
  // search hotkey) and folds back once it's empty and loses focus. Start open if a
  // query is already active so a persisted search stays visible.
  const [searchOpen, setSearchOpen] = useState(() => !!kanban$.searchQuery.peek().trim())
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const searchBarRef = useRef<HTMLDivElement | null>(null)
  const hasSendableAccount = accounts.some(isSendableAccount)
  const visibleColumns = useMemo(() => getKanbanColumns(boardId), [boards, boardId])
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }))
  const selectedKeys = useMemo(() => visibleColumns.map((column) => kanbanColumnKey(column)), [visibleColumns])

  useKanbanBoardSync(boardId, visibleColumns, accounts)
  const { dragPreview, setDragPreview, moveThread, handleDragStart, handleDragEnd } = useKanbanDnd(boardId, accounts)
  // One controller for the whole board so a right-click in one column closes any
  // menu open in another (only one thread menu can be open at a time).
  const threadMenu = useThreadContextMenu(accounts)

  useEffect(() => {
    if (!dialogOpen) return
    setDialogGroups(
      accounts.map((account) => ({
        accountId: account.id,
        label: account.display_name || account.email || account.id,
        email: account.email,
        avatarUrl: account.avatar_url,
        isRSS: isRSSAccount(account.id, accounts),
        folders: foldersByAccount[account.id] ?? [],
      })),
    )
  }, [accounts, dialogOpen, foldersByAccount])

  useEffect(() => {
    if (globalSearchFocus === 0) return
    setSearchOpen(true)
    requestAnimationFrame(() => {
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    })
  }, [globalSearchFocus])

  // Focus the input whenever the bar expands so it's immediately typeable.
  useEffect(() => {
    if (searchOpen) searchInputRef.current?.focus()
  }, [searchOpen])

  // Collapse the bar when the user clicks away, but only if it's empty — an active
  // query keeps the bar (and its results) visible.
  useEffect(() => {
    if (!searchOpen) return
    const onPointerDown = (event: MouseEvent) => {
      if (searchBarRef.current?.contains(event.target as Node)) return
      if (kanban$.searchQuery.peek().trim()) return
      setSearchOpen(false)
    }
    document.addEventListener('mousedown', onPointerDown)
    return () => document.removeEventListener('mousedown', onPointerDown)
  }, [searchOpen])

  async function openDialog() {
    const groups: AccountGroup[] = await Promise.all(
      accounts.map((account) =>
        ensureAccountFolders(account.id, {
          refreshIfBootstrapOnly: !isRSSAccount(account.id, accounts),
          waitForRefresh: true,
        }).then((accountFolders) => ({
          accountId: account.id,
          label: account.display_name || account.email || account.id,
          email: account.email,
          avatarUrl: account.avatar_url,
          isRSS: isRSSAccount(account.id, accounts),
          folders: accountFolders,
        })),
      ),
    )
    setDialogGroups(groups)
    setDialogOpen(true)
  }

  function searchColumn(column: KanbanColumn) {
    kanban$.searchScope.set(kanbanColumnKey(column))
    setSearchOpen(true)
    requestAnimationFrame(() => {
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    })
  }

  function applyColumns(nextKeys: string[]) {
    const next = new Set(nextKeys)
    const current = new Set(selectedKeys)

    for (const key of nextKeys) {
      if (current.has(key)) continue
      const [accountId, folderId] = key.split('\n')
      if (!accountId || !folderId) continue
      kanban$.threads[key].set([])
      kanban$.loading[key].set(true)
      addKanbanColumn(boardId, { accountId, folderId })
      void loadKanbanColumn({ accountId, folderId }, true)
    }

    for (const key of selectedKeys) {
      if (next.has(key)) continue
      const [accountId, folderId] = key.split('\n')
      if (accountId && folderId) removeKanbanColumn(boardId, { accountId, folderId })
    }

    setDialogOpen(false)
  }

  async function createDialogFolder(accountId: string, name: string): Promise<Folder> {
    const result = await invoke<{ folders: Folder[] }>('mail.folderCreate', { account_id: accountId, name })
    const folder = result.folders?.[0]
    if (!folder) throw new Error(t('folders.createdMissing'))

    const mergeFolders = (items: Folder[]) => {
      const byId = new Map(items.map((item) => [item.id, item]))
      byId.set(folder.id, folder)
      return [...byId.values()].sort((a, b) => a.name.localeCompare(b.name))
    }

    const nextFolders = mergeFolders(mail$.foldersByAccount[accountId].peek() ?? [])
    mail$.foldersByAccount[accountId].set(nextFolders)
    if (ui$.selectedAccount.peek() === accountId) {
      mail$.folders.set(mergeFolders(mail$.folders.peek() ?? []))
    }
    setDialogGroups((groups) =>
      groups.map((group) =>
        group.accountId === accountId ? { ...group, folders: mergeFolders(group.folders) } : group,
      ),
    )
    return folder
  }

  if (accounts.length === 0) {
    return (
      <section className="flex flex-1 border-r border-border bg-chats">
        <EmptyState title={t('empty.welcomeTitle')} text={t('empty.kanbanSetupText')} />
      </section>
    )
  }

  return (
    <section className="flex flex-1 min-w-0 flex-col border-r border-border bg-chats max-[768px]:w-full">
      <div className="@container relative z-30 flex min-h-16 shrink-0 items-center gap-3 border-b border-border/50 bg-white/70 backdrop-blur-md px-4 py-3 dark:bg-[#0f172a]/70">
        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          {board?.avatarUrl ? (
            <img
              src={board.avatarUrl}
              alt=""
              className="h-9 w-9 shrink-0 rounded-xl object-cover border border-accent/10"
            />
          ) : (
            <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-accent/10 text-accent shrink-0 border border-accent/10">
              <Columns3 size={16} />
            </div>
          )}
          <h2 className="truncate text-sm font-bold text-primary">{board?.name || t('kanban.board.defaultName')}</h2>
        </div>
        {searchOpen ? (
          <div
            ref={searchBarRef}
            className="flex h-9 min-w-0 basis-72 shrink items-center overflow-visible rounded-xl border border-transparent bg-hover focus-within:border-accent/40 focus-within:bg-chats"
          >
            <div className="relative h-full min-w-0 flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-secondary" size={14} />
              <input
                ref={searchInputRef}
                value={searchQuery}
                onChange={(event) => kanban$.searchQuery.set(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key !== 'Escape') return
                  if (searchQuery) kanban$.searchQuery.set('')
                  else setSearchOpen(false)
                }}
                placeholder={t('kanban.searchBoard')}
                className="h-full w-full border-0 bg-transparent py-1.5 pl-9.5 pr-8 text-xs text-primary outline-none placeholder-secondary transition-all"
              />
              {searchQuery && (
                <button
                  onClick={() => {
                    kanban$.searchQuery.set('')
                    searchInputRef.current?.focus()
                  }}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-secondary hover:text-primary cursor-pointer transition-colors"
                  title={t('common.clearSearch')}
                >
                  <X size={14} />
                </button>
              )}
            </div>
            <SearchScopeDropdown
              value={searchScope}
              onChange={(val) => kanban$.searchScope.set(val)}
              visibleColumns={visibleColumns}
            />
          </div>
        ) : (
          <IconButton icon={Search} label={t('kanban.searchBoardAction')} onClick={() => setSearchOpen(true)} />
        )}
        <FilterSwitch value={globalFilter} onChange={setGlobalKanbanFilter} />
        {hasSendableAccount && (
          <IconButton
            icon={SquarePen}
            label={t('composer.actions.newMessage')}
            onClick={() => {
              const id = openComposeTab()
              if (!id) return
              ui$.mobilePane.set('conversation')
            }}
          />
        )}
        <BoardMenu filterMode={globalFilter} onFilterChange={setGlobalKanbanFilter} onAddColumn={openDialog} />
      </div>
      {dialogOpen && (
        <AddColumnDialog
          groups={dialogGroups}
          initialSelected={selectedKeys}
          specialOptions={[
            {
              key: kanbanColumnKey({ accountId: 'unified', folderId: 'inbox' }),
              label: t('kanban.columns.unifiedInbox'),
              icon: 'inbox',
            },
            {
              key: kanbanColumnKey({ accountId: 'unified', folderId: 'starred' }),
              label: t('kanban.columns.unifiedStarred'),
              icon: 'star',
            },
          ]}
          onClose={() => setDialogOpen(false)}
          onApply={applyColumns}
          onCreateFolder={createDialogFolder}
        />
      )}
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        onDragCancel={() => setDragPreview(null)}
      >
        <div className="relative flex flex-1 min-h-0">
          {boardWallpaper && (
            <div className={`absolute inset-0 ${boardWallpaper.className}`} style={boardWallpaper.style} />
          )}
          <div className="relative flex flex-1 min-h-0 gap-2 overflow-x-auto p-2">
            {visibleColumns.length === 0 ? (
              <div className="flex flex-1 items-center justify-center">
                <EmptyState title={t('empty.noColumns')} text={t('empty.noColumnsText')} />
              </div>
            ) : (
              <SortableContext
                items={visibleColumns.map((column) => kanbanBoardColumnKey(boardId, column))}
                strategy={horizontalListSortingStrategy}
              >
                {visibleColumns.map((column) => (
                  <SortableColumn
                    key={kanbanColumnKey(column)}
                    boardId={boardId}
                    column={column}
                    onMoveThread={moveThread}
                    onSearchColumn={searchColumn}
                    threadMenu={threadMenu}
                  />
                ))}
              </SortableContext>
            )}
          </div>
        </div>
        <DragOverlay dropAnimation={null}>
          {dragPreview ? <KanbanDragPreview thread={dragPreview.thread} column={dragPreview.column} /> : null}
        </DragOverlay>
      </DndContext>
    </section>
  )
}
