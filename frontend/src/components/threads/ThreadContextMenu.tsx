import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import {
  Archive,
  ChevronRight,
  Copy,
  FolderInput,
  Mail,
  MailOpen,
  MessageSquare,
  Pencil,
  Star,
  Trash2,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { moveFeed, openFeedEdit } from '../../states/feeds'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import {
  archiveThread,
  copyThreadToFolder,
  deleteThread,
  ensureAccountFolders,
  isDraftFolder,
  isTrashFolderId,
  mail$,
  markThreadRead,
  markThreadUnread,
  moveThreadToFolder,
  starThread,
} from '../../states/mail'
import { accounts$, isSendableAccount } from '../../states/accounts'
import { isRssAccount } from '../../lib/threadActions'
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

export function ThreadContextMenu({
  controller,
  onAfterAction,
  onMove,
  onOpenThread,
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
}) {
  const { t } = useTranslation()
  const { menu, close } = controller
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const accounts = useValue(accounts$)
  const mailAccounts = accounts.filter(isSendableAccount)
  const rssAccounts = accounts.filter((account) => isRssAccount(account, account.id))
  const [moveOpen, setMoveOpen] = useState(false)
  const [copyOpen, setCopyOpen] = useState(false)
  const [copyLoading, setCopyLoading] = useState(false)
  const [, setCopyFoldersVersion] = useState(0)
  const moveAnchorRef = useRef<HTMLDivElement | null>(null)
  const copyAnchorRef = useRef<HTMLDivElement | null>(null)
  const copyLoadRef = useRef(false)
  const [moveFlyoutPosition, setMoveFlyoutPosition] = useState<{ x: number; y: number } | null>(null)
  const [copyFlyoutPosition, setCopyFlyoutPosition] = useState<{ x: number; y: number } | null>(null)

  const loadCopyTargetFolders = async () => {
    if (copyLoadRef.current) return
    copyLoadRef.current = true
    setCopyLoading(true)
    try {
      await Promise.all(
        mailAccounts.map((account) =>
          ensureAccountFolders(account.id, { refreshIfBootstrapOnly: true, waitForRefresh: true }),
        ),
      )
    } finally {
      copyLoadRef.current = false
      setCopyLoading(false)
      setCopyFoldersVersion((version) => version + 1)
    }
  }

  // Reset the "Move to" flyout each time the menu (re)opens, and make sure the
  // thread's account folders are loaded — in the unified view they aren't part
  // of the selected account's `mail$.folders`.
  const mailAccountId = menu?.kind === 'mail' ? menu.accountId : undefined
  useEffect(() => {
    setMoveOpen(false)
    setCopyOpen(false)
    if (mailAccountId) void ensureAccountFolders(mailAccountId)
  }, [mailAccountId, menu?.threadId])

  useEffect(() => {
    if (!copyOpen) return
    void loadCopyTargetFolders()
  }, [accounts, copyOpen])

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

  useLayoutEffect(() => {
    if (!copyOpen) {
      setCopyFlyoutPosition(null)
      return
    }

    const anchor = copyAnchorRef.current
    if (!anchor) return

    const gap = 4
    const flyoutWidth = 230
    const rect = anchor.getBoundingClientRect()
    const x = rect.right + gap + flyoutWidth > window.innerWidth ? rect.left - flyoutWidth - gap : rect.right + gap
    const y = rect.top

    setCopyFlyoutPosition((current) => {
      if (current?.x === x && current.y === y) return current
      return { x, y }
    })
  }, [copyOpen])

  if (!menu) return null

  const after = async (action: ThreadContextAction, threadId: string, detail?: ThreadContextActionDetail) => {
    await onAfterAction?.(action, threadId, detail)
  }

  if (menu.kind === 'feed') {
    const targetAccounts = rssAccounts.filter((account) => account.id !== menu.accountId)
    return (
      <FloatingContextMenu
        x={menu.x}
        y={menu.y}
        className="fixed z-50 min-w-[160px] rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
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
              trailing={<ChevronRight size={13} className="text-secondary" />}
              onClick={() => setMoveOpen((open) => !open)}
            />
            {moveOpen && moveFlyoutPosition && (
              <FloatingContextMenu
                x={moveFlyoutPosition.x}
                y={moveFlyoutPosition.y}
                className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[190px] overflow-y-auto rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
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

  const accountFolders = foldersByAccount[menu.accountId] ?? []
  const targetFolders = accountFolders.filter((folder) => folder.id !== menu.folderId)
  const copyAccountGroups = mailAccounts.map((account) => ({
    account,
    folders: (foldersByAccount[account.id] ?? []).filter(
      (folder) => !(account.id === menu.accountId && folder.id === menu.folderId),
    ),
  }))
  const inTrash = isTrashFolderId(menu.accountId, menu.folderId)
  const inDrafts = isDraftFolder(menu.folderId)

  return (
    <FloatingContextMenu
      x={menu.x}
      y={menu.y}
      className="fixed z-50 min-w-[190px] rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
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
      <MenuItem
        icon={<Archive size={13} className="text-secondary" />}
        label={t('threads.actions.archiveThread')}
        onClick={() => {
          const threadId = menu.threadId
          close()
          void archiveThread(threadId).then(() => after('archive', threadId))
        }}
      />
      {targetFolders.length > 0 && (
        <div
          ref={moveAnchorRef}
          onMouseEnter={() => {
            setCopyOpen(false)
            setMoveOpen(true)
          }}
          onMouseLeave={() => setMoveOpen(false)}
        >
          <MenuItem
            icon={<FolderInput size={13} className="text-secondary" />}
            label={t('threads.actions.moveTo')}
            trailing={<ChevronRight size={13} className="text-secondary" />}
            onClick={() => setMoveOpen((open) => !open)}
          />
          {moveOpen && moveFlyoutPosition && (
            <FloatingContextMenu
              x={moveFlyoutPosition.x}
              y={moveFlyoutPosition.y}
              className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[190px] overflow-y-auto rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
              dataAttribute="data-thread-context-menu"
            >
              {targetFolders.map((folder) => (
                <MenuItem
                  key={folder.id}
                  icon={<FolderInput size={13} className="shrink-0 text-secondary" />}
                  label={<span className="min-w-0 truncate">{folder.name}</span>}
                  onClick={() => {
                    const threadId = menu.threadId
                    const targetAccountId = menu.accountId
                    const targetFolderId = folder.id
                    close()
                    if (onMove) {
                      onMove(threadId, targetAccountId, targetFolderId)
                      return
                    }
                    void moveThreadToFolder(threadId, targetFolderId).then(() =>
                      after('move', threadId, { targetAccountId, targetFolderId }),
                    )
                  }}
                />
              ))}
            </FloatingContextMenu>
          )}
        </div>
      )}
      {mailAccounts.length > 0 && (
        <div
          ref={copyAnchorRef}
          onMouseEnter={() => {
            setMoveOpen(false)
            setCopyOpen(true)
            void loadCopyTargetFolders()
          }}
          onMouseLeave={() => setCopyOpen(false)}
        >
          <MenuItem
            icon={<Copy size={13} className="text-secondary" />}
            label={t('threads.actions.copyTo')}
            trailing={<ChevronRight size={13} className="text-secondary" />}
            onClick={() => {
              setCopyOpen((open) => !open)
              void loadCopyTargetFolders()
            }}
          />
          {copyOpen && copyFlyoutPosition && (
            <FloatingContextMenu
              x={copyFlyoutPosition.x}
              y={copyFlyoutPosition.y}
              className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[230px] overflow-y-auto rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
              dataAttribute="data-thread-context-menu"
            >
              {copyAccountGroups.map(({ account, folders }) => (
                <div key={account.id}>
                  <div className="px-3 pb-1 pt-2 text-[11px] font-semibold text-secondary">
                    {account.display_name || account.email || account.id}
                  </div>
                  {folders.length === 0 && (
                    <div className="px-3 py-2 text-xs font-semibold text-secondary">
                      {copyLoading ? t('folders.loading') : t('folders.noneAvailable')}
                    </div>
                  )}
                  {folders.map((folder) => (
                    <MenuItem
                      key={`${account.id}:${folder.id}`}
                      icon={<Copy size={13} className="shrink-0 text-secondary" />}
                      label={<span className="min-w-0 truncate">{folder.name}</span>}
                      onClick={() => {
                        const threadId = menu.threadId
                        const targetAccountId = account.id
                        const targetFolderId = folder.id
                        close()
                        void copyThreadToFolder(threadId, targetAccountId, targetFolderId).then(() =>
                          after('copy', threadId, { targetAccountId, targetFolderId }),
                        )
                      }}
                    />
                  ))}
                </div>
              ))}
            </FloatingContextMenu>
          )}
        </div>
      )}
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
