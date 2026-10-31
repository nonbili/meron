import { useEffect, useRef } from 'react'
import { useDraggable } from '@dnd-kit/core'
import { CSS } from '@dnd-kit/utilities'
import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import { ui$ } from '../../states/ui'
import { openMessageTab, openDraftConversationOrCompose, openThreadTab } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { isDraftFolder } from '../../states/mailFolders'
import { thread$ } from '../../states/thread'
import { focusKanbanThreadFolder, kanbanBoardColumnKey, kanban$, type KanbanColumn } from '../../states/kanban'
import { isUnifiedStarredColumn } from '../../lib/kanbanData'
import { isRssAccount } from '../../lib/threadActions'
import type { Message } from '../../types'
import { ThreadListItem } from '../threads/ThreadListItem'
import type { ThreadContextMenuController } from '../threads/ThreadContextMenu'
import { PANE_ANIMATION_MS } from './KanbanConversationPane'

export function KanbanThreadCard({
  boardId,
  thread,
  column,
  threadMenu,
  ownerKey,
  bulkSelectable = false,
  bulkEnabled = false,
  bulkSelected = false,
  onBulkRangeSelect,
  onBulkModeSelect,
  onBulkPlainSelect,
}: {
  boardId: string
  thread: Message
  column: KanbanColumn
  threadMenu: ThreadContextMenuController
  // Identifies this card's column so the shared menu only renders here.
  ownerKey: string
  bulkSelectable?: boolean
  bulkEnabled?: boolean
  bulkSelected?: boolean
  onBulkRangeSelect?: () => void
  onBulkModeSelect?: () => void
  onBulkPlainSelect?: () => void
}) {
  const accounts = useValue(accounts$)
  // Highlight is keyed off the open pane, not ui$.selectedThread, so a card can
  // never read as "selected" while its conversation pane is closed.
  const paneThreadId = useValue(kanban$.paneThreadId)
  const movingThread = useValue(kanban$.movingThread)
  const selectedItemRef = useRef<HTMLDivElement | null>(null)
  const starredColumn = isUnifiedStarredColumn(column)
  const account = accounts.find((item) => item.id === thread.account_id)
  const starredFeed = starredColumn && isRssAccount(account, thread.account_id)
  // Scoped to the column: the same thread can sit in two columns of one board
  // (a unified inbox next to the account's own), and draggable ids must be unique.
  const draggableId = `${ownerKey}\n${starredFeed ? thread.id : thread.thread_id}`
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: draggableId,
    // A unified column's cards are draggable too: the card carries the thread's
    // real account/folder, which the drop resolves as the move's origin.
    data: { type: 'thread', threadId: thread.thread_id, source: column },
  })
  // A starred feed item opens in its own reader tab rather than as the card
  // conversation, so it reads as selected while that tab is on screen. Its
  // thread_id is the whole feed's, so match the tab by the item's own id.
  const activeTab = useValue(compose$.activeTab)
  const active = starredFeed ? activeTab === thread.id : thread.thread_id === paneThreadId

  // Follow the card for as long as the pane slides open: the pane narrows the
  // board after the card is selected, so a card that fits now can end up under it.
  useEffect(() => {
    if (!active) return
    const keepInView = () => selectedItemRef.current?.scrollIntoView({ block: 'nearest' })
    keepInView()
    const until = performance.now() + PANE_ANIMATION_MS
    let frame = requestAnimationFrame(function follow(now) {
      keepInView()
      if (now < until) frame = requestAnimationFrame(follow)
    })
    return () => cancelAnimationFrame(frame)
  }, [active])

  // Ctrl/Cmd+click and double-click. Starred feed items are single articles and
  // open in a reader tab, as their plain click does. The pane shows for any
  // active tab, so paneThreadId stays the card conversation behind the tabs:
  // pointing it at the tab would leave an empty, unclosable Current behind.
  const openInNewTab = () => {
    if (starredFeed) openMessageTab(thread)
    else openThreadTab(thread)
    kanban$.paneColumnKey.set(kanbanBoardColumnKey(boardId, column))
    ui$.mobilePane.set('conversation')
  }

  // Cards are dragged by pointer only: the board's keyboard sensor is for column
  // headers, and here it would swallow Enter/Space meant for the row.
  const { onKeyDown: _keyboardDrag, ...pointerListeners } = listeners ?? {}

  const style = {
    transform: transform && !isDragging ? CSS.Translate.toString(transform) : undefined,
    opacity: isDragging ? 0.18 : movingThread === thread.thread_id ? 0.55 : undefined,
    zIndex: isDragging ? 20 : undefined,
  }

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...pointerListeners}>
      <ThreadListItem
        thread={thread}
        contextMenuOpen={threadMenu.isOpen(thread, ownerKey)}
        active={starredColumn ? active : thread.thread_id === paneThreadId}
        rootRef={active ? selectedItemRef : undefined}
        badgeAccount={column.accountId === 'unified' ? account : undefined}
        // The margin reaches past the column's own padding and border to the
        // board's gutter, so scrolling to a card shows its whole column.
        className="scroll-mx-[13px] rounded-lg border border-border bg-chats shadow-sm overflow-hidden"
        bulkSelectable={bulkSelectable}
        bulkSelected={bulkSelected}
        onSelect={(event) => {
          if (!bulkSelectable && (event.metaKey || event.ctrlKey)) {
            openInNewTab()
            return
          }
          if (bulkEnabled && event.shiftKey) {
            onBulkRangeSelect?.()
            return
          }
          if (bulkSelectable) {
            onBulkModeSelect?.()
            return
          }
          onBulkPlainSelect?.()
          // Draft replies open in context; standalone drafts resume in composer.
          if (isDraftFolder(thread.folder_id, thread.account_id)) {
            ui$.selectedThread.set(thread.thread_id)
            kanban$.paneThreadId.set(thread.thread_id)
            kanban$.paneColumnKey.set(kanbanBoardColumnKey(boardId, column))
            ui$.mobilePane.set('conversation')
            void openDraftConversationOrCompose(thread)
            return
          }
          if (starredFeed) {
            // Feed rows carry their full body: open the item in a reader tab.
            // Mail rows are ordinary threads and open like any other card.
            openInNewTab()
            return
          }
          focusKanbanThreadFolder(thread.folder_id)
          // Leave any open compose/reader/thread tab first so the selectedThread
          // retarget is recorded as the Current tab's thread (conversationThread).
          compose$.activeTab.set('')
          ui$.selectedThread.set(thread.thread_id)
          kanban$.paneThreadId.set(thread.thread_id)
          kanban$.paneColumnKey.set(kanbanBoardColumnKey(boardId, column))
          ui$.mobilePane.set('conversation')
        }}
        onOpenInNewTab={() => {
          // A draft's first click already resumed it in the composer.
          if (bulkSelectable || isDraftFolder(thread.folder_id, thread.account_id)) return
          openInNewTab()
        }}
        onContextMenu={(event) => {
          if (bulkSelectable) {
            event.preventDefault()
            event.stopPropagation()
            return
          }
          threadMenu.open(event, thread, ownerKey)
        }}
      />
    </div>
  )
}

export function KanbanDragPreview({ thread, column }: { thread: Message; column: KanbanColumn }) {
  const accounts = useValue(accounts$)

  return (
    <div className="w-[310px] max-w-[calc(100vw-32px)] cursor-grabbing opacity-95 shadow-2xl">
      <ThreadListItem
        thread={thread}
        badgeAccount={
          column.accountId === 'unified' ? accounts.find((item) => item.id === thread.account_id) : undefined
        }
        className="rounded-lg border border-border bg-chats shadow-lg overflow-hidden"
        onSelect={() => undefined}
      />
    </div>
  )
}
