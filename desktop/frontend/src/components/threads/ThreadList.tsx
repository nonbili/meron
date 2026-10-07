import { useEffect, useRef, useState } from 'react'
import type { CSSProperties, DragEvent, PointerEventHandler } from 'react'
import { Plus, SquarePen, MoreHorizontal, Loader2 } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openAddFeed, RSS_FEED_DRAG_TYPE } from '../../states/feeds'
import { openComposeTab, openThreadTab, openMessageTab, openDraftConversationOrCompose } from '../../states/compose'
import { compose$ } from '../../states/composeState'
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
  loadMoreThreads,
  loadThreads,
  threadListViewKey,
  listFilterKey,
} from '../../states/mail'
import { markAllRead } from '../../states/mailFlags'
import {
  isDraftFolder,
  folderUnread,
  emptiableFolder,
  emptyFolder,
  deletableFolder,
  deleteFolder,
  setFolderNotify,
  folderMatches,
} from '../../states/mailFolders'
import { exportFolderAsEml } from '../../states/emlExport'
import { isRssAccount } from '../../lib/threadActions'
import { folderLabel } from '../../lib/kanbanData'
import { isUnifiedStarred } from '../../lib/unifiedFolders'
import { EmptyState } from '../empty-state/EmptyState'
import { IconButton } from '../button/IconButton'
import { QuickSettingsMenu } from '../sidenav/QuickSettingsMenu'
import { useTitleBar } from '../titlebar/TitleBar'
import { FolderSwitcher } from '../menu/FolderSwitcher'
import { ThreadActionsMenu } from './ThreadActionsMenu'
import { ThreadContextMenu, useThreadContextMenu } from './ThreadContextMenu'
import { ThreadListItem } from './ThreadListItem'
import { ThreadSearchInput } from './ThreadSearchInput'
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
  const bulkSelection = useValue(ui$.bulkSelection)
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const system = useValue(ui$.system)
  const filteredThreads = useValue(getFilteredThreads)
  const filterMode = useValue(ui$.filterMode)
  const attachmentsOnly = useValue(ui$.attachmentsOnly)
  const threadsCursor = useValue(mail$.threadsCursor)
  const threadsLoadingMore = useValue(mail$.threadsLoadingMore)
  // The rows on hand belong to the view they were loaded for. Until that is the
  // view being rendered, an empty list is a pending one, not an empty folder —
  // true from the first paint of a navigation, which is a frame or more before
  // the effect that starts the load.
  const threadsLoadedKey = useValue(mail$.threadsLoadedKey)
  const threadsLoading =
    threadsLoadedKey !==
    threadListViewKey(selectedAccount, selectedFolder, query, listFilterKey(filterMode, attachmentsOnly))
  const threadMenu = useThreadContextMenu(accounts)
  // Starred is a folder of the unified view whose rows span every account. It
  // lists ordinary threads, so it shares this list's selection, context menu and
  // bulk selection; only the cross-account chrome below differs.
  const isStarredView = isUnifiedStarred(selectedAccount, selectedFolder)
  // Quick-settings (view + theme) anchor for the narrow-window header button.
  // The side navigation that normally hosts these controls is hidden at this width.
  const [quickMenu, setQuickMenu] = useState<{ x: number; y: number } | null>(null)
  const titleBar = useTitleBar()
  const [searchFocused, setSearchFocused] = useState(false)
  // Without a title bar of ours the search box is in this header, and searching
  // takes it over: the folder name and the icon buttons step aside so the box is
  // full width while it is being used. It stays open with text in it after blur,
  // so a typed query is never truncated out of view.
  const searchExpanded = !titleBar && (searchFocused || query.length > 0)

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
  }, [selectedThread])

  const archiveFolder = folders.find((f) => f.role === 'archive' || f.id === 'archive')
  const hasArchive = !!archiveFolder
  const archiveFolderId = archiveFolder?.id ?? 'archive'
  const inArchive = hasArchive && selectedFolder === archiveFolderId

  const activeAccount = accounts.find((acc) => acc.id === selectedAccount)
  const isRSSAccount = activeAccount?.provider === 'rss' || activeAccount?.auth_type === 'rss'
  const hasSendableAccount = accounts.some(isSendableAccount)
  // Non-null only in a per-account Trash/Junk folder, which is where the
  // destructive "empty" action is offered.
  const emptiableTarget =
    isStarredView || isRSSAccount || selectedAccount === 'unified'
      ? null
      : emptiableFolder(folders.find((folder) => folder.id === selectedFolder))
  // Non-null only in an ordinary folder of a single account, the only kind the
  // server lets us delete.
  const deletableTarget =
    isStarredView || isRSSAccount || selectedAccount === 'unified'
      ? null
      : deletableFolder(
          folders.find((folder) => folder.id === selectedFolder),
          folders,
        )
  // A whole-folder export needs one real mailbox to list on the server. The
  // folder row only supplies a name: the inbox is selected as "inbox" whatever
  // the server calls it, and the list may not have loaded yet.
  // Folders other than the inbox (always watched) can be opted in to live sync
  // and notifications, for mail a server-side rule files straight into them.
  const notifyTarget =
    isStarredView || isRSSAccount || selectedAccount === 'unified'
      ? undefined
      : folders.find(
          (folder) =>
            folder.id === selectedFolder && !['inbox', 'drafts', 'sent', 'trash', 'junk'].includes(folder.role),
        )
  const canExportFolder = !isStarredView && !isRSSAccount && !!activeAccount && !!selectedFolder
  const exportFolderName =
    folders.find((folder) => folderMatches(folder, selectedAccount, selectedFolder))?.name ?? selectedFolder
  const hasUnread = isRSSAccount
    ? filteredThreads.some((thread) => thread.unread)
    : folderUnread(folders, selectedFolder) > 0 || filteredThreads.some((thread) => thread.unread)
  // A search pages on the cursor the core mints for it, whatever the filter chip
  // says — a query is answered by a search, not by a filtered listing. Without a
  // cursor (feeds, starred) there is nothing more to load. The attachments
  // toggle narrows the same cursor-paged listing, so it does not stop paging.
  const canLoadMore = !!threadsCursor && (!!query.trim() || filterMode === 'all')
  const feedRowsDraggable = !isStarredView && isRSSAccount
  // The folder switcher needs a folder list to offer. That is an account's real
  // folders, or — in the unified view — the synthetic per-role list, where each
  // entry means "every account's own Sent/Archive/…". An RSS account has no
  // folders at all: its "inbox" is the whole feed set.
  const showFolderSwitcher = !isRSSAccount && !!selectedFolder && (selectedAccount === 'unified' || !!activeAccount)
  // Empty-state copy, which the folder switcher made it possible to land on for
  // any folder: "your inbox is empty" only holds in the inbox, and neither
  // wording holds while a search or filter is hiding the threads that are there.
  const inboxFolder = folders.find((folder) => folder.role === 'inbox' || folder.id === 'inbox')
  const inInbox = selectedFolder === (inboxFolder?.id ?? 'inbox')
  const narrowed = !!query.trim() || filterMode !== 'all' || attachmentsOnly
  const emptyStateTitle = narrowed
    ? t('empty.noMatchingMail')
    : isRSSAccount
      ? t('empty.noFeeds')
      : inInbox
        ? t('empty.noChats')
        : t('empty.nothingHereYet')
  const emptyStateText = narrowed
    ? t('empty.adjustSearchFilter')
    : isRSSAccount
      ? t('empty.addFeedToStart')
      : inArchive
        ? t('empty.noArchivedThreads')
        : inInbox
          ? t('empty.inboxEmpty')
          : t('empty.pullLatestMessages')
  const loadMoreButton = (
    <button
      className="mx-3 my-3 flex h-9 shrink-0 items-center justify-center rounded-lg border border-border text-xs font-semibold text-secondary hover:bg-hover disabled:cursor-not-allowed disabled:opacity-60 cursor-pointer transition-colors"
      disabled={threadsLoadingMore}
      onClick={() => void loadMoreThreads()}
    >
      {threadsLoadingMore ? t('common.loading') : t('threads.actions.loadMore')}
    </button>
  )
  const desktopBulk = isWailsDesktopRuntime() || !!system
  const bulkItems = selectedBulkItems()
  const bulkGroupKey = `thread-list:${selectedAccount}:${selectedFolder}`
  const bulkInThisList =
    desktopBulk && bulkItems.length > 0 && bulkItems.every((item) => item.groupKey === bulkGroupKey)

  const bulkItemFor = (thread: (typeof filteredThreads)[number]): BulkSelectionItem => {
    // Cross-account lists (unified, starred) mix mail and feed rows, so the kind
    // comes from the row's own account rather than the selected one.
    const rowIsFeed =
      isRSSAccount ||
      isRssAccount(
        accounts.find((acc) => acc.id === thread.account_id),
        thread.account_id,
      )
    return {
      key: `thread-list:${isStarredView && rowIsFeed ? thread.id : thread.thread_id}`,
      groupKey: bulkGroupKey,
      threadId: thread.thread_id,
      accountId: thread.account_id,
      folderId: thread.folder_id,
      surface: 'thread-list',
      kind: rowIsFeed ? 'feed' : 'mail',
      unread: thread.unread,
      starred: thread.starred,
      draft: isDraftFolder(thread.folder_id, thread.account_id),
      trash: folders.some((folder) => folder.id === thread.folder_id && folder.role === 'trash'),
    }
  }

  // Ctrl/Cmd+click and double-click. Starred feed rows are single articles, so
  // they open in a reader tab, as their plain click does.
  const openInNewTab = (thread: (typeof filteredThreads)[number]) => {
    if (
      isStarredView &&
      isRssAccount(
        accounts.find((acc) => acc.id === thread.account_id),
        thread.account_id,
      )
    ) {
      openMessageTab(thread)
      ui$.mobilePane.set('conversation')
      return
    }
    openThreadTab(thread)
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
      className={`relative flex w-full shrink-0 flex-col bg-chats min-[769px]:w-[var(--thread-list-width)] ${
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
        <BulkActionBar items={bulkItems} allItems={filteredThreads.map(bulkItemFor)} className="min-h-12" />
      ) : (
        // Less padding on the right than the left: the trailing icon buttons carry
        // their own, so px-4 on both sides left the row lopsided.
        <div className="flex h-12 shrink-0 flex-row items-center gap-3 pl-4 pr-2 border-b border-border/60">
          <div className="flex items-center gap-2 w-full">
            {/* Current folder, doubling as a picker: switching here retargets the
              list the same way it retargets a kanban column. Capped so a deep
              folder name can't crowd out the search box. */}
            {showFolderSwitcher && !searchExpanded && (
              <FolderSwitcher
                accountId={selectedAccount}
                folderId={selectedFolder}
                label={folderLabel({ accountId: selectedAccount, folderId: selectedFolder }, folders, accounts, t)}
                labelClassName="-ml-2 max-w-[40%] shrink-0 text-xs font-bold"
                onSelect={(nextFolderId) => ui$.selectedFolder.set(nextFolderId)}
              />
            )}
            {titleBar ? <div className="flex-1" /> : <ThreadSearchInput onFocusChange={setSearchFocused} />}

            {!searchExpanded && (
              <>
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
                {/* Compose remains in the header when the side navigation is hidden. */}
                {hasSendableAccount && !isStarredView && !isRSSAccount && (
                  <IconButton
                    icon={SquarePen}
                    iconSize={16}
                    label={t('composer.actions.newMessage')}
                    className="min-[769px]:hidden"
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
                    attachmentsOnly={attachmentsOnly}
                    onAttachmentsOnlyChange={(on) => ui$.attachmentsOnly.set(on)}
                    hasUnread={hasUnread}
                    onMarkAllRead={() => markAllRead()}
                    onEmptyFolder={
                      emptiableTarget
                        ? () =>
                            void emptyFolder(selectedAccount, selectedFolder, emptiableTarget).then(async (emptied) => {
                              if (emptied) await loadThreads(false)
                            })
                        : undefined
                    }
                    emptyFolderLabel={
                      emptiableTarget?.role === 'junk'
                        ? t('threads.actions.emptyJunk')
                        : t('threads.actions.emptyTrash')
                    }
                    onDeleteFolder={
                      deletableTarget
                        ? () =>
                            void deleteFolder(
                              selectedAccount,
                              selectedFolder,
                              deletableTarget.name,
                              deletableTarget.nested,
                            )
                        : undefined
                    }
                    onToggleNotify={
                      notifyTarget
                        ? () =>
                            void setFolderNotify(
                              selectedAccount,
                              notifyTarget.id,
                              !notifyTarget.notify,
                              notifyTarget.name,
                            )
                        : undefined
                    }
                    notifyEnabled={!!notifyTarget?.notify}
                    onExportFolder={
                      canExportFolder
                        ? () => void exportFolderAsEml(selectedAccount, selectedFolder, exportFolderName)
                        : undefined
                    }
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
                {/* View + theme: only reachable here when the side navigation is hidden
                  (narrow) and there is no title bar of ours holding it. */}
                {!titleBar && (
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
                )}
              </>
            )}
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
        className="flex-1 overflow-y-auto flex flex-col gap-0.5 p-1.5"
        onScroll={(event) => {
          if (!canLoadMore || threadsLoadingMore) return
          const el = event.currentTarget
          if (el.scrollHeight - el.scrollTop - el.clientHeight < 240) {
            void loadMoreThreads()
          }
        }}
      >
        {accounts.length === 0 ? (
          <EmptyState title={t('empty.welcomeTitle')} text={t('empty.mailSetupText')} />
        ) : filteredThreads.length === 0 ? (
          // An empty list mid-load is "not here yet", not "nothing here": telling
          // the user their folder is empty and then filling it a moment later is
          // the wrong answer twice. Spin until the load lands, same as the
          // conversation pane does while a thread is being fetched.
          threadsLoading ? (
            <div className="flex h-full items-center justify-center" role="status" aria-label={t('common.loading')}>
              <Loader2 size={28} className="animate-spin text-secondary/70" />
            </div>
          ) : isStarredView ? (
            <EmptyState title={t('empty.noStarredItems')} text={t('empty.noStarredItemsText')} />
          ) : (
            // A narrowed page can come back empty with older mail still to read
            // (the core stops reading on after a few emptied pages), so an empty
            // list is not the end while the cursor says otherwise.
            <EmptyState
              title={emptyStateTitle}
              text={emptyStateText}
              action={canLoadMore ? loadMoreButton : undefined}
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
                  active={thread.thread_id === selectedThread}
                  badgeAccount={
                    isStarredView || selectedAccount === 'unified'
                      ? accounts.find((acc) => acc.id === thread.account_id)
                      : undefined
                  }
                  contextMenuOpen={threadMenu.isOpen(thread)}
                  rootRef={thread.thread_id === selectedThread ? selectedItemRef : undefined}
                  draggable={feedRowsDraggable}
                  onDragStart={(event) => startFeedDrag(event, thread)}
                  bulkSelectable={desktopBulk && bulkInThisList}
                  bulkSelected={!!bulkSelection[bulkItem.key]}
                  onSelect={(event) => {
                    if (!bulkInThisList && (event.metaKey || event.ctrlKey)) {
                      openInNewTab(thread)
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
                    if (isDraftFolder(thread.folder_id, thread.account_id)) {
                      ui$.selectedThread.set(thread.thread_id)
                      ui$.mobilePane.set('conversation')
                      void openDraftConversationOrCompose(thread)
                      return
                    }
                    if (isStarredView) {
                      // RSS rows carry their full body: open the item directly in
                      // a reader tab. Mail rows are ordinary threads.
                      const account = accounts.find((acc) => acc.id === thread.account_id)
                      if (isRssAccount(account, thread.account_id)) {
                        openMessageTab(thread)
                        ui$.mobilePane.set('conversation')
                        return
                      }
                    }
                    // Leave any open compose/reader/thread tab first so the
                    // selectedThread retarget is recorded as the Current tab's
                    // thread (conversationThread) rather than skipped.
                    compose$.activeTab.set('')
                    ui$.selectedThread.set(thread.thread_id)
                    ui$.mobilePane.set('conversation')
                  }}
                  onOpenInNewTab={() => {
                    // A draft's first click already resumed it in the composer.
                    if (bulkInThisList || isDraftFolder(thread.folder_id, thread.account_id)) return
                    openInNewTab(thread)
                  }}
                  onContextMenu={(event) => {
                    if (bulkInThisList) {
                      event.preventDefault()
                      event.stopPropagation()
                      return
                    }
                    threadMenu.open(event, thread)
                  }}
                />
              )
            })}
            {canLoadMore && loadMoreButton}
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
    </section>
  )
}
