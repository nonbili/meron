import { useState } from 'react'
import type { MouseEvent } from 'react'
import type { Account, Message } from '../../types'
import { targetWithin, useDismissOnOutside } from '../menu/useDismissOnOutside'

export type ThreadMenuState =
  | {
      kind: 'feed'
      x: number
      y: number
      threadId: string
      accountId: string
      subject: string
      name: string
      url?: string
      unread: boolean
      // In the kanban view the controller is shared across columns; this marks
      // which column opened the menu so only that column renders it.
      ownerKey?: string
    }
  | {
      kind: 'mail'
      x: number
      y: number
      threadId: string
      subject: string
      accountId: string
      folderId: string
      unread: boolean
      starred: boolean
      ownerKey?: string
    }

export type ThreadContextAction =
  | 'mark-read'
  | 'mark-unread'
  | 'star'
  | 'unstar'
  | 'archive'
  | 'move'
  | 'copy'
  | 'delete'
  | 'open-tab'
  | 'edit-feed'

export type ThreadContextMenuController = {
  menu: ThreadMenuState | null
  close: () => void
  // `ownerKey` scopes the menu to a single kanban column when the controller is
  // shared across columns; omit it in single-list views (e.g. the chat view).
  open: (event: MouseEvent, thread: Message, ownerKey?: string) => void
}

export type ThreadContextActionDetail = {
  targetAccountId?: string
  targetFolderId?: string
}

function isRssThread(thread: Message, accounts: Account[]) {
  const threadAccount = accounts.find((acc) => acc.id === thread.account_id)
  return threadAccount?.provider === 'rss' || threadAccount?.auth_type === 'rss' || thread.account_id.startsWith('rss-')
}

// Manages the open/close state of a thread's right-click menu and resolves the
// clicked thread into either a feed or mail menu. Auto-closes on outside click,
// scroll (outside the menu) and resize.
export function useThreadContextMenu(accounts: Account[]): ThreadContextMenuController {
  const [menu, setMenu] = useState<ThreadMenuState | null>(null)

  useDismissOnOutside(
    menu !== null,
    (target) => targetWithin(target, '[data-thread-context-menu]'),
    () => setMenu(null),
  )

  return {
    menu,
    close: () => setMenu(null),
    open: (event, thread, ownerKey) => {
      event.preventDefault()
      if (isRssThread(thread, accounts)) {
        setMenu({
          kind: 'feed',
          x: event.clientX,
          y: event.clientY,
          threadId: thread.thread_id,
          accountId: thread.account_id,
          subject: thread.subject || '(no subject)',
          name: thread.from_name || thread.subject || 'this feed',
          url: thread.feed_url,
          unread: thread.unread,
          ownerKey,
        })
        return
      }

      setMenu({
        kind: 'mail',
        x: event.clientX,
        y: event.clientY,
        threadId: thread.thread_id,
        subject: thread.subject || '(no subject)',
        accountId: thread.account_id,
        folderId: thread.folder_id,
        unread: thread.unread,
        starred: thread.starred,
        ownerKey,
      })
    },
  }
}
