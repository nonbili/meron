import { useEffect, useRef, useState } from 'react'
import type { CSSProperties, DragEvent, PointerEventHandler } from 'react'
import { Search, X, Plus, ChevronLeft, SquarePen, MoreHorizontal } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openAddFeed, RSS_FEED_DRAG_TYPE } from '../../states/feeds'
import {
  openComposeTab,
  openThreadTab,
  openMessageTab,
  openDraftConversationOrCompose,
  compose$,
} from '../../states/compose'
import { accounts$, isSendableAccount } from '../../states/accounts'
import {
  clearBulkSelection,
  isWailsDesktopRuntime,
  selectedBulkItems,
  setBulkSelection,
  toggleBulkSelection,
  ui$,
  type BulkSelectionItem,
} from '../../states/ui'
import { thread$ } from '../../states/thread'
import {
  mail$,
  getFilteredThreads,
  syncMail,
  markAllRead,
  loadMoreThreads,
  isDraftFolder,
  folderUnread,
} from '../../states/mail'
import { isRssAccount } from '../../lib/threadActions'
import { EmptyState } from '../empty-state/EmptyState'
import { IconButton } from '../button/IconButton'
import { QuickSettingsMenu } from '../sidenav/QuickSettingsMenu'
import { type MessageContextMenuState } from '../chat/MessageContextMenu'
import { StarredItemMenu } from './StarredItemMenu'
import { ThreadActionsMenu } from './ThreadActionsMenu'
import { ThreadContextMenu, useThreadContextMenu } from './ThreadContextMenu'
import { ThreadListItem } from './ThreadListItem'
import { BulkActionBar } from './BulkActionBar'

type ThreadListProps = {
  width?: number
  onResizeStart?: PointerEventHandler<HTMLDivElement>
}

export function ThreadList({ width, onResizeStart }: ThreadListProps = {}) {
  const { t } = useTranslation()
  const query = useValue(ui$.query)
  const busy = useValue(ui$.busy)
  const mobilePane = useValue(ui$.mobilePane)
  const selectedAccount = useValue(ui$.selectedAccount)
  const selectedFolder = useValue(ui$.selectedFolder)
  const selectedThread = useValue(ui$.selectedThread)
  const selectedStarredItem = useValue(ui$.selectedStarredItem)
  const bulkSelection = useValue(ui$.bulkSelection)
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const system = useValue(ui$.system)
  const filteredThreads = useValue(getFilteredThreads)
  const filterMode = useValue(ui$.filterMode)
  const threadsCursor = useValue(mail$.threadsCursor)
  const threadsLoadingMore = useValue(mail$.threadsLoadingMore)
  const threadMenu = useThreadContextMenu(accounts)
  // The starred view lists individual messages/feed items, so right-clicks get
  // the per-message menu instead of the thread one.
  const isStarredView = selectedAccount === 'starred'
  const [starredMenu, setStarredMenu] = useState<MessageContextMenuState | null>(null)
  // Quick-settings (view + theme) anchor for the narrow-window header button.
  // The side navigation that normally hosts these controls is hidden at this width.
  const [quickMenu, setQuickMenu] = useState<{ x: number; y: number } | null>(null)
  // Focus the search box when ⌘/Ctrl+Shift+F (or the palette) bumps the signal.
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const globalSearchFocus = useValue(ui$.globalSearchFocus)
  useEffect(() => {
    if (globalSearchFocus === 0) return
    const input = searchInputRef.current
    input?.focus()
    input?.select()
  }, [globalSearchFocus])

  // Keep the selected thread visible when it changes — chiefly so j/k keyboard
  // navigation can walk past the fold. `block: "nearest"` leaves already-visible
  // rows (e.g. mouse clicks) untouched.
  const selectedItemRef = useRef<HTMLDivElement | null>(null)
  const restoreFocusAfterDeleteRef = useRef(false)
  useEffect(() => {
    const selectedItem = selectedItemRef.current
    selectedItem?.scrollIntoView({ block: 'nearest' })
    if (restoreFocusAfterDeleteRef.current) {
      restoreFocusAfterDeleteRef.current = false
      selectedItem?.querySelector('button')?.focus()
    }
  }, [selectedThread, selectedStarredItem])

  const archiveFolder = folders.find((f) => f.role === 'archive' || f.id === 'archive')
  const hasArchive = !!archiveFolder
  const archiveFolderId = archiveFolder?.id ?? 'archive'
  const inArchive = hasArchive && selectedFolder === archiveFolderId

  const activeAccount = accounts.find((acc) => acc.id === selectedAccount)
  const isRSSAccount = activeAccount?.provider === 'rss' || activeAccount?.auth_type === 'rss'
  const hasSendableAccount = accounts.some(isSendableAccount)
  const hasUnread = isRSSAccount
    ? filteredThreads.some((thread) => thread.unread)
    : folderUnread(folders, selectedFolder) > 0 || filteredThreads.some((thread) => thread.unread)
  const canLoadMore = !!threadsCursor && !query.trim() && filterMode === 'all'
  const feedRowsDraggable = !isStarredView && isRSSAccount
  const desktopBulk = isWailsDesktopRuntime() || !!system
  const bulkItems = selectedBulkItems()
  const bulkGroupKey = isStarredView ? 'starred' : `thread-list:${selectedAccount}:${selectedFolder}`
  const bulkInThisList =
    desktopBulk && bulkItems.length > 0 && bulkItems.every((item) => item.groupKey === bulkGroupKey)

  const bulkItemFor = (thread: (typeof filteredThreads)[number]): BulkSelectionItem => {
    const starredFeed =
      isStarredView &&
      isRssAccount(
        accounts.find((acc) => acc.id === thread.account_id),
        thread.account_id,
      )
    const key = isStarredView ? `starred:${thread.id}` : `thread-list:${thread.thread_id}`
    return {
      key,
      groupKey: isStarredView ? 'starred' : `thread-list:${selectedAccount}:${selectedFolder}`,
      threadId: thread.thread_id,
      messageId: isStarredView && !starredFeed ? thread.id : undefined,
      accountId: thread.account_id,
      folderId: thread.folder_id,
      surface: isStarredView ? 'starred' : 'thread-list',
      kind: isRSSAccount || starredFeed ? 'feed' : 'mail',
      unread: thread.unread,
      starred: thread.starred,
      draft: !isStarredView && isDraftFolder(thread.folder_id, thread.account_id),
      trash: folders.some((folder) => folder.id === thread.folder_id && folder.role === 'trash'),
    }
  }

  const selectRangeTo = (target: BulkSelectionItem) => {
    const anchor = ui$.bulkAnchorKey.peek()
    const rows = filteredThreads.map(bulkItemFor)
    const targetIndex = rows.findIndex((item) => item.key === target.key)
    const anchorIndex = rows.findIndex((item) => item.key === anchor)
    if (targetIndex === -1 || anchorIndex === -1) {
      toggleBulkSelection(target)
      return
    }
    const [from, to] = targetIndex < anchorIndex ? [targetIndex, anchorIndex] : [anchorIndex, targetIndex]
    setBulkSelection(rows.slice(from, to + 1), target.key)
  }

  const startFeedDrag = (event: DragEvent<HTMLDivElement>, thread: (typeof filteredThreads)[number]) => {
    if (!feedRowsDraggable) return
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData(
      RSS_FEED_DRAG_TYPE,
      JSON.stringify({ threadId: thread.thread_id, accountId: thread.account_id }),
    )
    event.dataTransfer.setData('text/plain', thread.subject || thread.from_name || t('feeds.fallbackName'))
  }

  return (
    <section
      data-thread-list
      className={`relative flex w-full shrink-0 flex-col border-r border-border bg-chats min-[769px]:w-[var(--thread-list-width)] ${
        mobilePane === 'threads' ? 'max-[768px]:flex' : 'max-[768px]:hidden'
      }`}
      onKeyDownCapture={(event) => {
        if (event.key === 'Delete') restoreFocusAfterDeleteRef.current = true
      }}
      style={width ? ({ '--thread-list-width': `${width}px` } as CSSProperties) : undefined}
    >
      {onResizeStart && (
        <div
          className="absolute right-0 top-0 z-20 hidden h-full w-2 translate-x-1 cursor-col-resize min-[769px]:block"
          onPointerDown={onResizeStart}
          title={t('layout.resizeThreadList')}
        >
          <div className="mx-auto h-full w-px bg-transparent hover:bg-accent" />
        </div>
      )}
      {bulkInThisList ? (
        <BulkActionBar items={bulkItems} className="min-h-16" />
      ) : (
        <div className="flex h-16 shrink-0 flex-row items-center gap-3 px-4 border-b border-border bg-white dark:bg-[#0f172a]/40">
          <div className="flex items-center gap-2 w-full">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-secondary" size={15} />
              <input
                ref={searchInputRef}
                value={query}
                onChange={(event) => ui$.query.set(event.target.value)}
                placeholder={isRSSAccount ? t('threads.searchFeeds') : t('threads.searchMessages')}
                className="w-full rounded-xl bg-hover py-2 pl-9.5 pr-8 text-[13px] text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:bg-chats border border-transparent focus:border-transparent transition-all duration-150"
              />
              {query && (
                <button
                  onClick={() => ui$.query.set('')}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-secondary hover:text-primary cursor-pointer"
                >
                  <X size={15} />
                </button>
              )}
            </div>

            {/* Add account: only reachable here when the side navigation is hidden (narrow). */}
            <IconButton
              icon={Plus}
              iconSize={18}
              label={t('accounts.actions.addAccount')}
              size="md"
              radius="lg"
              className="min-[769px]:hidden"
              onClick={() => ui$.setupOpen.set(true)}
            />
            {/* New mail (compose) */}
            {hasSendableAccount && !isStarredView && !isRSSAccount && (
              <IconButton
                icon={SquarePen}
                iconSize={16}
                label={t('composer.actions.newMessage')}
                size="md"
                radius="lg"
                onClick={() => openComposeTab()}
              />
            )}
            {/* Filter + mark-all-read overflow menu (shared with kanban columns).
              Hidden in the starred view, where the list is starred-only by definition. */}
            {!isStarredView && (
              <ThreadActionsMenu
                filterMode={filterMode}
                onFilterChange={(mode) => ui$.filterMode.set(mode)}
                hasUnread={hasUnread}
                onMarkAllRead={() => markAllRead()}
                onSync={syncMail}
                syncing={busy}
                allLabel={isRSSAccount ? t('filters.allFeeds') : t('filters.all')}
                syncLabel={isRSSAccount ? t('feeds.actions.syncFeeds') : t('threads.actions.syncMailbox')}
                syncingLabel={isRSSAccount ? t('feeds.actions.syncingFeeds') : t('threads.actions.syncing')}
              />
            )}
            {isRSSAccount && (
              <IconButton
                icon={Plus}
                iconSize={16}
                label={t('feeds.actions.addToAccount')}
                size="md"
                radius="lg"
                onClick={() => openAddFeed(selectedAccount)}
              />
            )}
            {/* View + theme: only reachable here when the side navigation is hidden (narrow). */}
            <IconButton
              icon={MoreHorizontal}
              iconSize={18}
              label={t('sidenav.actions.viewAndTheme')}
              size="md"
              radius="lg"
              className="min-[769px]:hidden"
              onClick={(e) => {
                const rect = e.currentTarget.getBoundingClientRect()
                setQuickMenu({ x: rect.right - 208, y: rect.bottom })
              }}
            />
          </div>
        </div>
      )}

      {quickMenu && (
        <QuickSettingsMenu
          anchor={{ x: quickMenu.x, y: quickMenu.y, placement: 'down' }}
          onClose={() => setQuickMenu(null)}
        />
      )}

      {/* Thread List Items */}
      <div
        className="flex-1 overflow-y-auto flex flex-col"
        onScroll={(event) => {
          if (!canLoadMore || threadsLoadingMore) return
          const el = event.currentTarget
          if (el.scrollHeight - el.scrollTop - el.clientHeight < 240) {
            void loadMoreThreads()
          }
        }}
      >
        {/* Back to Inbox row if inside archive */}
        {inArchive ? (
          <button
            onClick={() => ui$.selectedFolder.set('inbox')}
            className="w-full flex items-center gap-2 px-2 py-3 bg-accent/10 hover:bg-accent/15 dark:bg-accent/15 dark:hover:bg-accent/20 text-accent transition-colors font-semibold text-xs cursor-pointer border-b border-border/50"
          >
            <ChevronLeft size={16} />
            <span>{t('threads.backToInbox')}</span>
          </button>
        ) : null}

        {accounts.length === 0 ? (
          <EmptyState title={t('empty.welcomeTitle')} text={t('empty.mailSetupText')} />
        ) : filteredThreads.length === 0 ? (
          isStarredView ? (
            <EmptyState title={t('empty.noStarredItems')} text={t('empty.noStarredItemsText')} />
          ) : (
            <EmptyState
              title={isRSSAccount ? t('empty.noFeeds') : t('empty.noChats')}
              text={
                isRSSAccount
                  ? t('empty.addFeedToStart')
                  : inArchive
                    ? t('empty.noArchivedThreads')
                    : t('empty.inboxEmpty')
              }
            />
          )
        ) : (
          <>
            {filteredThreads.map((thread) => {
              const bulkItem = bulkItemFor(thread)
              return (
                <ThreadListItem
                  key={thread.id}
                  thread={thread}
                  accounts={accounts}
                  selectedAccount={selectedAccount}
                  selectedThread={selectedThread}
                  active={isStarredView ? thread.id === selectedStarredItem : undefined}
                  rootRef={
                    (isStarredView ? thread.id === selectedStarredItem : thread.thread_id === selectedThread)
                      ? selectedItemRef
                      : undefined
                  }
                  showAccountBadge={isStarredView ? true : undefined}
                  draggable={feedRowsDraggable}
                  onDragStart={(event) => startFeedDrag(event, thread)}
                  bulkSelectable={desktopBulk && bulkInThisList}
                  bulkSelected={!!bulkSelection[bulkItem.key]}
                  onSelect={(event) => {
                    if (desktopBulk && (event.metaKey || event.ctrlKey)) {
                      toggleBulkSelection(bulkItem)
                      return
                    }
                    if (desktopBulk && event.shiftKey) {
                      selectRangeTo(bulkItem)
                      return
                    }
                    if (bulkInThisList) {
                      toggleBulkSelection(bulkItem)
                      return
                    }
                    clearBulkSelection()
                    if (!isStarredView && isDraftFolder(thread.folder_id, thread.account_id)) {
                      ui$.selectedThread.set(thread.thread_id)
                      ui$.mobilePane.set('conversation')
                      void openDraftConversationOrCompose(thread)
                      return
                    }
                    if (isStarredView) {
                      ui$.selectedStarredItem.set(thread.id)
                      // RSS rows carry their full body: open the item directly in a
                      // reader tab. Mail rows are headers only: open the thread and
                      // jump to the starred message.
                      const account = accounts.find((acc) => acc.id === thread.account_id)
                      if (isRssAccount(account, thread.account_id)) {
                        openMessageTab(thread)
                        ui$.mobilePane.set('conversation')
                        return
                      }
                      thread$.pendingScrollMessageId.set(thread.id)
                    }
                    // Leave any open compose/reader/thread tab first so the
                    // selectedThread retarget is recorded as the Current tab's
                    // thread (conversationThread) rather than skipped.
                    compose$.activeTab.set('')
                    ui$.selectedThread.set(thread.thread_id)
                    ui$.mobilePane.set('conversation')
                  }}
                  onContextMenu={(event) => {
                    if (bulkInThisList) {
                      event.preventDefault()
                      event.stopPropagation()
                      return
                    }
                    if (isStarredView) {
                      event.preventDefault()
                      setStarredMenu({ x: event.clientX, y: event.clientY, message: thread })
                      return
                    }
                    threadMenu.open(event, thread)
                  }}
                />
              )
            })}
            {canLoadMore && (
              <button
                className="mx-3 my-3 flex h-9 shrink-0 items-center justify-center rounded-lg border border-border text-xs font-semibold text-secondary hover:bg-hover disabled:cursor-not-allowed disabled:opacity-60 cursor-pointer transition-colors"
                disabled={threadsLoadingMore}
                onClick={() => void loadMoreThreads()}
              >
                {threadsLoadingMore ? t('common.loading') : t('threads.actions.loadMore')}
              </button>
            )}
          </>
        )}
      </div>

      <ThreadContextMenu
        controller={threadMenu}
        onSelectThread={(threadId) => {
          const thread = filteredThreads.find((item) => item.thread_id === threadId)
          if (thread) toggleBulkSelection(bulkItemFor(thread))
        }}
        onOpenThread={(threadId) => {
          const thread = filteredThreads.find((item) => item.thread_id === threadId)
          if (thread) openThreadTab(thread)
        }}
      />

      {starredMenu && (
        <StarredItemMenu
          state={starredMenu}
          accounts={accounts}
          onClose={() => setStarredMenu(null)}
          onSelect={(message) => toggleBulkSelection(bulkItemFor(message))}
        />
      )}
    </section>
  )
}
