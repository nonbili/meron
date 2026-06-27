import { useEffect, useRef } from 'react'
import { useDraggable } from '@dnd-kit/core'
import { CSS } from '@dnd-kit/utilities'
import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import { ui$ } from '../../states/ui'
import { compose$, openMessageTab, openDraftCompose } from '../../states/compose'
import { isDraftFolder } from '../../states/mail'
import { thread$ } from '../../states/thread'
import { kanbanBoardColumnKey, kanban$, type KanbanColumn } from '../../states/kanban'
import { isUnifiedStarredColumn } from '../../lib/kanbanData'
import { isRssAccount } from '../../lib/threadActions'
import type { Message } from '../../types'
import { ThreadListItem } from '../threads/ThreadListItem'
import type { ThreadContextMenuController } from '../threads/ThreadContextMenu'

export function KanbanThreadCard({
  boardId,
  thread,
  column,
  threadMenu,
  ownerKey,
}: {
  boardId: string
  thread: Message
  column: KanbanColumn
  threadMenu: ThreadContextMenuController
  // Identifies this card's column so the shared menu only renders here.
  ownerKey: string
}) {
  const accounts = useValue(accounts$)
  const selectedStarredItem = useValue(ui$.selectedStarredItem)
  // Highlight is keyed off the open pane, not ui$.selectedThread, so a card can
  // never read as "selected" while its conversation pane is closed.
  const paneThreadId = useValue(kanban$.paneThreadId)
  const movingThread = useValue(kanban$.movingThread)
  const selectedItemRef = useRef<HTMLDivElement | null>(null)
  const starredColumn = isUnifiedStarredColumn(column)
  const draggableId = starredColumn ? thread.id : thread.thread_id
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: draggableId,
    data: { type: 'thread', threadId: thread.thread_id, source: column },
    disabled: column.accountId === 'unified',
  })
  const active = starredColumn ? thread.id === selectedStarredItem : thread.thread_id === paneThreadId

  useEffect(() => {
    if (active) selectedItemRef.current?.scrollIntoView({ block: 'nearest' })
  }, [active])

  const style = {
    transform: transform && !isDragging ? CSS.Translate.toString(transform) : undefined,
    opacity: isDragging ? 0.18 : movingThread === thread.thread_id ? 0.55 : undefined,
    zIndex: isDragging ? 20 : undefined,
  }

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      <ThreadListItem
        thread={thread}
        accounts={accounts}
        selectedAccount={starredColumn ? 'starred' : column.accountId}
        selectedThread={paneThreadId}
        active={starredColumn ? active : undefined}
        rootRef={active ? selectedItemRef : undefined}
        showAccountBadge={column.accountId === 'unified'}
        className="rounded-lg border border-border bg-chats shadow-sm overflow-hidden"
        onSelect={() => {
          // A draft card opens the full composer rather than a read-only pane.
          if (!starredColumn && isDraftFolder(thread.folder_id)) {
            void openDraftCompose(thread)
            ui$.selectedThread.set(thread.thread_id)
            kanban$.paneThreadId.set(thread.thread_id)
            kanban$.paneColumnKey.set(kanbanBoardColumnKey(boardId, column))
            ui$.mobilePane.set('conversation')
            return
          }
          if (starredColumn) {
            ui$.selectedStarredItem.set(thread.id)
            const account = accounts.find((acc) => acc.id === thread.account_id)
            if (isRssAccount(account, thread.account_id)) {
              openMessageTab(thread)
              kanban$.paneThreadId.set(thread.thread_id)
              kanban$.paneColumnKey.set(kanbanBoardColumnKey(boardId, column))
              ui$.mobilePane.set('conversation')
              return
            }
            thread$.pendingScrollMessageId.set(thread.id)
          }
          ui$.selectedFolder.set(thread.folder_id)
          // Leave any open compose/reader/thread tab first so the selectedThread
          // retarget is recorded as the Current tab's thread (conversationThread).
          compose$.activeTab.set('')
          ui$.selectedThread.set(thread.thread_id)
          kanban$.paneThreadId.set(thread.thread_id)
          kanban$.paneColumnKey.set(kanbanBoardColumnKey(boardId, column))
          ui$.mobilePane.set('conversation')
        }}
        onContextMenu={(event) => threadMenu.open(event, thread, ownerKey)}
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
        accounts={accounts}
        selectedAccount={column.accountId}
        selectedThread=""
        showAccountBadge={column.accountId === 'unified'}
        className="rounded-lg border border-border bg-chats shadow-lg overflow-hidden"
        onSelect={() => undefined}
      />
    </div>
  )
}
