import { useEffect, useLayoutEffect, useRef, useState, type MouseEvent } from 'react'
import { useValue } from '@legendapp/state/react'
import {
  Archive,
  SquareDashedMousePointer,
  ChevronRight,
  Download,
  FolderInput,
  Mail,
  MailOpen,
  MessageSquare,
  Pencil,
  Star,
  Trash2,
} from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { moveFeed, openFeedEdit } from '../../states/feeds'
import { settings$ } from '../../states/settings'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { AddMailToTasksMenu } from '../tasks/AddMailToTasksMenu'
import { markThreadRead, markThreadUnread, starThread } from '../../states/mailFlags'
import { deletesForever, isDraftFolder } from '../../states/mailFolders'
import { archiveThread, deleteThread } from '../../states/mailMoves'
import { exportThreadsAsEml } from '../../states/emlExport'
import { accounts$, isLocalAccountId } from '../../states/accounts'
import { isRssAccount } from '../../lib/threadActions'
import type { Account, Message } from '../../types'
import { targetWithin, useDismissOnOutside } from '../menu/useDismissOnOutside'
import { MessageContextMenu } from '../chat/MessageContextMenu'
import { ThreadMoveCopyItems } from './ThreadMoveCopyItems'
import { ThreadTagItem } from './ThreadTagItem'

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
      message: Message
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
  isOpen: (thread: Message, ownerKey?: string) => boolean
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
    isOpen: (thread, ownerKey) =>
      menu !== null &&
      menu.ownerKey === ownerKey &&
      menu.accountId === thread.account_id &&
      (menu.kind === 'feed' ? menu.message.id === thread.id : menu.threadId === thread.thread_id),
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
          message: thread,
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

export function ThreadContextMenu({
  controller,
  onAfterAction,
  onMove,
  onOpenThread,
  onSelectThread,
}: {
  controller: ThreadContextMenuController
  onAfterAction?: (
    action: ThreadContextAction,
    threadId: string,
    detail?: ThreadContextActionDetail,
  ) => void | Promise<void>
  // When provided, the "Move to…" action delegates the whole move to the caller
  // (e.g. the kanban view's optimistic move-and-revert) instead of running the
  // default backend move + onAfterAction reload.
  onMove?: (threadId: string, targetAccountId: string, targetFolderId: string) => void
  // Required so every view that mounts the menu wires up "Open in new tab" — the
  // item always renders, so a missing handler would be a dead button.
  onOpenThread: (threadId: string) => void
  onSelectThread?: (threadId: string) => void
}) {
  const { t } = useTranslation()
  const { menu, close } = controller
  const accounts = useValue(accounts$)
  const rssAccounts = accounts.filter((account) => isRssAccount(account, account.id))
  // The feed menu's "Move to" flyout; a mail thread's is ThreadMoveCopyItems'.
  const [moveOpen, setMoveOpen] = useState(false)
  const moveAnchorRef = useRef<HTMLDivElement | null>(null)
  const [moveFlyoutPosition, setMoveFlyoutPosition] = useState<{ x: number; y: number } | null>(null)
  const tasksEnabled = useValue(settings$.tasksEnabled)

  // Reset the flyout each time the menu (re)opens.
  useEffect(() => {
    setMoveOpen(false)
  }, [menu?.threadId])

  useLayoutEffect(() => {
    if (!moveOpen) {
      setMoveFlyoutPosition(null)
      return
    }

    const anchor = moveAnchorRef.current
    if (!anchor) return

    const gap = 4
    const flyoutWidth = 190
    const rect = anchor.getBoundingClientRect()
    const x = rect.right + gap + flyoutWidth > window.innerWidth ? rect.left - flyoutWidth - gap : rect.right + gap
    const y = rect.top

    setMoveFlyoutPosition((current) => {
      if (current?.x === x && current.y === y) return current
      return { x, y }
    })
  }, [moveOpen])

  if (!menu) return null

  const after = async (action: ThreadContextAction, threadId: string, detail?: ThreadContextActionDetail) => {
    await onAfterAction?.(action, threadId, detail)
  }

  if (menu.kind === 'feed') {
    // Starred RSS results are individual articles: their id includes the item
    // key while threadId identifies the whole subscription. Keep article
    // actions item-scoped instead of offering feed edit/move/read operations.
    if (menu.message.id !== menu.threadId) {
      return <MessageContextMenu state={{ x: menu.x, y: menu.y, message: menu.message }} isRSS onClose={close} />
    }
    const targetAccounts = rssAccounts.filter((account) => account.id !== menu.accountId)
    return (
      <FloatingContextMenu
        x={menu.x}
        y={menu.y}
        className="fixed z-50 min-w-[160px] shadow-xl"
        dataAttribute="data-thread-context-menu"
        onClick={(event) => event.stopPropagation()}
        onContextMenu={(event) => event.preventDefault()}
      >
        <MenuItem
          icon={<MessageSquare size={13} className="text-secondary" />}
          label={t('threads.actions.openInNewTab')}
          onClick={() => {
            const threadId = menu.threadId
            close()
            onOpenThread(threadId)
            void after('open-tab', threadId)
          }}
        />
        <MenuItem
          icon={
            menu.unread ? (
              <MailOpen size={13} className="text-secondary" />
            ) : (
              <Mail size={13} className="text-secondary" />
            )
          }
          label={menu.unread ? t('threads.actions.markAsRead') : t('threads.actions.markAsUnread')}
          onClick={() => {
            const action = menu.unread ? 'mark-read' : 'mark-unread'
            const work = menu.unread ? markThreadRead(menu.threadId) : markThreadUnread(menu.threadId)
            const threadId = menu.threadId
            close()
            void work.then(() => after(action, threadId))
          }}
        />
        <MenuItem
          icon={<Pencil size={13} className="text-secondary" />}
          label={t('buttons.edit')}
          onClick={() => {
            openFeedEdit({ threadId: menu.threadId, name: menu.name, url: menu.url })
            close()
            void after('edit-feed', menu.threadId)
          }}
        />
        {targetAccounts.length > 0 && (
          <div ref={moveAnchorRef} onMouseEnter={() => setMoveOpen(true)} onMouseLeave={() => setMoveOpen(false)}>
            <MenuItem
              icon={<FolderInput size={13} className="text-secondary" />}
              label={t('threads.actions.moveTo')}
              submenuOpen={moveOpen}
              trailing={<ChevronRight size={13} className="text-secondary" />}
              onClick={() => setMoveOpen((open) => !open)}
            />
            {moveOpen && moveFlyoutPosition && (
              <FloatingContextMenu
                x={moveFlyoutPosition.x}
                y={moveFlyoutPosition.y}
                className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[190px] overflow-y-auto shadow-xl"
                dataAttribute="data-thread-context-menu"
              >
                {targetAccounts.map((account) => (
                  <MenuItem
                    key={account.id}
                    icon={<FolderInput size={13} className="shrink-0 text-secondary" />}
                    label={
                      <span className="min-w-0 truncate">{account.display_name || account.email || account.id}</span>
                    }
                    onClick={() => {
                      const threadId = menu.threadId
                      const targetAccountId = account.id
                      close()
                      void moveFeed(threadId, targetAccountId).then(() => after('move', threadId, { targetAccountId }))
                    }}
                  />
                ))}
              </FloatingContextMenu>
            )}
          </div>
        )}
      </FloatingContextMenu>
    )
  }

  const inTrash = deletesForever(menu.accountId, menu.folderId)
  const inDrafts = isDraftFolder(menu.folderId, menu.accountId)

  return (
    <FloatingContextMenu
      x={menu.x}
      y={menu.y}
      className="fixed z-50 min-w-[190px] shadow-xl"
      dataAttribute="data-thread-context-menu"
      onClick={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.preventDefault()}
    >
      <MenuItem
        icon={
          menu.unread ? (
            <MailOpen size={13} className="text-secondary" />
          ) : (
            <Mail size={13} className="text-secondary" />
          )
        }
        label={menu.unread ? t('threads.actions.markAsRead') : t('threads.actions.markAsUnread')}
        onClick={() => {
          const action = menu.unread ? 'mark-read' : 'mark-unread'
          const work = menu.unread ? markThreadRead(menu.threadId) : markThreadUnread(menu.threadId)
          const threadId = menu.threadId
          close()
          void work.then(() => after(action, threadId))
        }}
      />
      <MenuItem
        icon={<Star size={13} className={menu.starred ? 'fill-amber-500 text-amber-500' : 'text-secondary'} />}
        label={menu.starred ? t('threads.actions.unstarThread') : t('threads.actions.starThread')}
        onClick={() => {
          const action = menu.starred ? 'unstar' : 'star'
          const starred = !menu.starred
          const threadId = menu.threadId
          close()
          void starThread(threadId, starred).then(() => after(action, threadId))
        }}
      />
      <ThreadTagItem key={menu.threadId} threadId={menu.threadId} close={close} />
      <MenuItem
        icon={<MessageSquare size={13} className="text-secondary" />}
        label={t('threads.actions.openInNewTab')}
        onClick={() => {
          const threadId = menu.threadId
          close()
          onOpenThread(threadId)
          void after('open-tab', threadId)
        }}
      />
      {onSelectThread && (
        <MenuItem
          icon={<SquareDashedMousePointer size={13} className="text-secondary" />}
          label={t('buttons.select', { defaultValue: 'Select' })}
          onClick={() => {
            const threadId = menu.threadId
            close()
            onSelectThread(threadId)
          }}
        />
      )}
      {tasksEnabled && (
        <AddMailToTasksMenu
          key={menu.threadId}
          title={menu.subject}
          account={menu.accountId}
          threadId={menu.threadId}
          onAdded={close}
        />
      )}
      <div className="my-1 border-t border-border" />
      <MenuItem
        icon={<Download size={13} className="text-secondary" />}
        label={t('chat.actions.saveAsEml')}
        onClick={() => {
          const { threadId, folderId, subject } = menu
          close()
          void exportThreadsAsEml([{ threadId, folderId }], subject)
        }}
      />
      {!isLocalAccountId(menu.accountId) && (
        <MenuItem
          icon={<Archive size={13} className="text-secondary" />}
          label={t('threads.actions.archiveThread')}
          onClick={() => {
            const threadId = menu.threadId
            close()
            void archiveThread(threadId).then(() => after('archive', threadId))
          }}
        />
      )}
      <ThreadMoveCopyItems
        threadId={menu.threadId}
        accountId={menu.accountId}
        folderId={menu.folderId}
        close={close}
        onMove={onMove}
        onMoved={(targetAccountId, targetFolderId) => after('move', menu.threadId, { targetAccountId, targetFolderId })}
        onCopied={(targetAccountId, targetFolderId) =>
          after('copy', menu.threadId, { targetAccountId, targetFolderId })
        }
      />
      <div className="my-1 border-t border-border" />
      <MenuItem
        danger
        icon={<Trash2 size={13} />}
        label={
          inTrash
            ? t('threads.actions.deleteForever')
            : inDrafts
              ? t('threads.actions.discardDraft')
              : t('threads.actions.moveToTrash')
        }
        onClick={() => {
          const threadId = menu.threadId
          close()
          void deleteThread(threadId, { permanent: inTrash }).then(() => after('delete', threadId))
        }}
      />
    </FloatingContextMenu>
  )
}
