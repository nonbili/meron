import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { useValue } from '@legendapp/state/react'
import { ChevronRight, Copy, FolderInput, History } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { accounts$, isLocalAccountId, isMailFolderAccount } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { ensureAccountFolders } from '../../states/mailFolders'
import { copyThreadToFolder, moveThreadToFolder } from '../../states/mailMoves'
import { recentThreadTransfers$, type RecentThreadTransfer } from '../../states/recentThreadTransfers'
import { AccountAvatar } from '../avatar/AccountAvatar'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { FolderMenuTree } from '../menu/FolderMenuTree'
import { MenuItem } from '../menu/MenuItem'
import type { Account, Folder } from '../../types'

const MOVE_FLYOUT_WIDTH = 230
const COPY_FLYOUT_WIDTH = 230
const ACCOUNT_FLYOUT_WIDTH = 230
const RECENT_LIMIT = 3

// The flyouts are portalled out of the menu that opens them, so that menu's
// "click outside" check has to count them as inside: they carry this marker.
// Above every menu that hosts them, the conversation's overlay included.
const flyoutClass = 'fixed z-[62] max-h-[calc(100vh-1rem)] overflow-y-auto shadow-xl'

/** Where a flyout of `width` opens beside its menu row: to the right, or to the
 *  left when the window has no room there. Null while closed. */
function useFlyoutPosition(open: boolean, anchorRef: RefObject<HTMLDivElement | null>, width: number) {
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null)
  useLayoutEffect(() => {
    if (!open) {
      setPosition(null)
      return
    }
    const anchor = anchorRef.current
    if (!anchor) return
    const gap = 4
    const rect = anchor.getBoundingClientRect()
    const x = rect.right + gap + width > window.innerWidth ? rect.left - width - gap : rect.right + gap
    const y = rect.top
    setPosition((current) => (current?.x === x && current.y === y ? current : { x, y }))
  }, [open, anchorRef, width])
  return position
}

/**
 * The "Move to" and "Copy to" rows of a mail thread's menu, each opening a
 * flyout: recent targets for that action, then one row per account (the
 * thread's own first) opening its folders a level further out. Shared by the thread list's right-click menu and the open
 * conversation's actions menu, which style their rows differently — hence
 * `itemClassName` and `iconSize`.
 */
export function ThreadMoveCopyItems({
  threadId,
  accountId,
  folderId,
  close,
  onMove,
  onMoved,
  onCopied,
  itemClassName,
  iconSize = 13,
}: {
  threadId: string
  accountId: string
  folderId: string
  close: () => void
  // When provided, the move is delegated whole to the caller (e.g. the kanban
  // view's optimistic move-and-revert) instead of running the default one.
  onMove?: (threadId: string, targetAccountId: string, targetFolderId: string) => void
  onMoved?: (targetAccountId: string, targetFolderId: string) => void | Promise<void>
  onCopied?: (targetAccountId: string, targetFolderId: string) => void | Promise<void>
  itemClassName?: string
  iconSize?: number
}) {
  const { t } = useTranslation()
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const accounts = useValue(accounts$)
  const recentTransfers = useValue(recentThreadTransfers$)
  const mailAccounts = accounts.filter(isMailFolderAccount)
  const [moveOpen, setMoveOpen] = useState(false)
  const [copyOpen, setCopyOpen] = useState(false)
  const [openAccountId, setOpenAccountId] = useState<string | null>(null)
  const [copyLoading, setCopyLoading] = useState(false)
  const [, setCopyFoldersVersion] = useState(0)
  const moveAnchorRef = useRef<HTMLDivElement | null>(null)
  const copyAnchorRef = useRef<HTMLDivElement | null>(null)
  const copyLoadRef = useRef(false)
  const moveFlyoutPosition = useFlyoutPosition(moveOpen, moveAnchorRef, MOVE_FLYOUT_WIDTH)
  const copyFlyoutPosition = useFlyoutPosition(copyOpen, copyAnchorRef, COPY_FLYOUT_WIDTH)

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

  // Reset the flyouts each time the menu turns to another thread, and make
  // sure the thread's account folders are loaded — in the unified view they
  // aren't part of the selected account's `mail$.folders`.
  useEffect(() => {
    setMoveOpen(false)
    setCopyOpen(false)
    setOpenAccountId(null)
    if (accountId) void ensureAccountFolders(accountId)
  }, [accountId, threadId])

  useEffect(() => {
    if (!copyOpen && !moveOpen) return
    void loadCopyTargetFolders()
  }, [accounts, copyOpen, moveOpen])

  useEffect(() => {
    const targetAccounts = new Set(
      recentTransfers.filter((entry) => entry.sourceAccountId === accountId).map((entry) => entry.targetAccountId),
    )
    for (const targetAccountId of targetAccounts) {
      if (
        targetAccountId !== accountId &&
        accounts.some((account) => account.id === targetAccountId && isMailFolderAccount(account))
      ) {
        void ensureAccountFolders(targetAccountId)
      }
    }
  }, [accountId, accounts, recentTransfers])

  const canMove = mailAccounts.length > 0
  // The thread's own account leads; a lone account needs no row of its own.
  const targetAccounts = [
    ...mailAccounts.filter((account) => account.id === accountId),
    ...mailAccounts.filter((account) => account.id !== accountId),
  ]
  const inlineAccount = targetAccounts.length === 1 ? targetAccounts[0] : undefined
  // The thread's own folder stays in the tree as a parent, just not pickable.
  const excludedFolders = (targetAccountId: string) => (targetAccountId === accountId ? [folderId] : [])
  const recentTargets = (action: RecentThreadTransfer['action']) =>
    recentTransfers
      .flatMap((entry) => {
        if (entry.action !== action || entry.sourceAccountId !== accountId) return []
        if (
          entry.targetAccountId === accountId &&
          (entry.targetFolderId === folderId ||
            (entry.targetFolderId.toUpperCase() === 'INBOX' && folderId.toUpperCase() === 'INBOX'))
        )
          return []
        const account = mailAccounts.find((account) => account.id === entry.targetAccountId)
        const folder = foldersByAccount[entry.targetAccountId]?.find((folder) => folder.id === entry.targetFolderId)
        if (!account || !folder) return []
        const label = entry.targetAccountId === accountId ? folder.name : `${accountLabel(account)} / ${folder.name}`
        return [{ accountId: account.id, folder, label }]
      })
      .slice(0, RECENT_LIMIT)

  const moveTo = (targetAccountId: string, targetFolderId: string) => {
    close()
    if (onMove) {
      onMove(threadId, targetAccountId, targetFolderId)
      return
    }
    void moveThreadToFolder(threadId, targetFolderId, { targetAccountId }).then((moved) => {
      if (moved) return onMoved?.(targetAccountId, targetFolderId)
    })
  }

  const copyTo = (targetAccountId: string, targetFolderId: string) => {
    close()
    void copyThreadToFolder(threadId, targetAccountId, targetFolderId).then(() =>
      onCopied?.(targetAccountId, targetFolderId),
    )
  }

  const flyout = (action: RecentThreadTransfer['action'], position: { x: number; y: number }) => {
    const pick = (targetAccountId: string, folder: Folder) =>
      action === 'move' ? moveTo(targetAccountId, folder.id) : copyTo(targetAccountId, folder.id)
    const recents = recentTargets(action)
    return (
      <FloatingContextMenu
        x={position.x}
        y={position.y}
        className={`${flyoutClass} min-w-[230px]`}
        dataAttribute="data-thread-context-menu"
      >
        {recents.map((recent) => (
          <MenuItem
            key={`${recent.accountId}:${recent.folder.id}`}
            icon={<History size={13} className="shrink-0 text-secondary" />}
            label={
              <span className="min-w-0 max-w-[280px] truncate" title={recent.label}>
                {recent.label}
              </span>
            }
            onClick={() => pick(recent.accountId, recent.folder)}
          />
        ))}
        {recents.length > 0 && targetAccounts.length > 0 && <div className="my-1 border-t border-border" />}
        {inlineAccount ? (
          <>
            {(foldersByAccount[inlineAccount.id] ?? []).length === 0 && <FolderListStatus loading={copyLoading} />}
            <FolderMenuTree
              folders={foldersByAccount[inlineAccount.id] ?? []}
              excludedFolderIds={excludedFolders(inlineAccount.id)}
              onPick={(folder) => pick(inlineAccount.id, folder)}
            />
          </>
        ) : (
          targetAccounts.map((account) => (
            <AccountFolderItem
              key={account.id}
              account={account}
              folders={foldersByAccount[account.id] ?? []}
              excludedFolderIds={excludedFolders(account.id)}
              loading={copyLoading}
              open={openAccountId === account.id}
              setOpen={(open) =>
                setOpenAccountId((current) => (open ? account.id : current === account.id ? null : current))
              }
              onPick={(folder) => pick(account.id, folder)}
            />
          ))
        )}
      </FloatingContextMenu>
    )
  }

  return (
    <>
      {canMove && (
        <div
          ref={moveAnchorRef}
          onMouseEnter={() => {
            setCopyOpen(false)
            setOpenAccountId(null)
            setMoveOpen(true)
          }}
          onMouseLeave={() => setMoveOpen(false)}
        >
          <MenuItem
            icon={<FolderInput size={iconSize} className="shrink-0 text-secondary" />}
            label={t('threads.actions.moveTo')}
            submenuOpen={moveOpen}
            trailing={<ChevronRight size={13} className="text-secondary" />}
            className={itemClassName}
            onClick={() => setMoveOpen((open) => !open)}
          />
          {moveOpen && moveFlyoutPosition && flyout('move', moveFlyoutPosition)}
        </div>
      )}
      {mailAccounts.length > 0 && (
        <div
          ref={copyAnchorRef}
          onMouseEnter={() => {
            setMoveOpen(false)
            setOpenAccountId(null)
            setCopyOpen(true)
            void loadCopyTargetFolders()
          }}
          onMouseLeave={() => setCopyOpen(false)}
        >
          <MenuItem
            icon={<Copy size={iconSize} className="shrink-0 text-secondary" />}
            label={t('threads.actions.copyTo')}
            submenuOpen={copyOpen}
            trailing={<ChevronRight size={13} className="text-secondary" />}
            className={itemClassName}
            onClick={() => {
              setCopyOpen((open) => !open)
              void loadCopyTargetFolders()
            }}
          />
          {copyOpen && copyFlyoutPosition && flyout('copy', copyFlyoutPosition)}
        </div>
      )}
    </>
  )
}

function accountLabel(account: Account) {
  return account.display_name || account.email || account.id
}

function FolderListStatus({ loading }: { loading: boolean }) {
  const { t } = useTranslation()
  return (
    <div className="px-3 py-2 text-xs font-semibold text-secondary">
      {loading ? t('folders.loading') : t('folders.noneAvailable')}
    </div>
  )
}

/** A row in the folder flyout for one account, opening that account's
 *  folders one level further out. */
function AccountFolderItem({
  account,
  folders,
  excludedFolderIds,
  loading,
  open,
  setOpen,
  onPick,
}: {
  account: Account
  folders: Folder[]
  excludedFolderIds: string[]
  loading: boolean
  open: boolean
  setOpen: (open: boolean) => void
  onPick: (folder: Folder) => void
}) {
  const { t } = useTranslation()
  const anchorRef = useRef<HTMLDivElement | null>(null)
  const position = useFlyoutPosition(open, anchorRef, ACCOUNT_FLYOUT_WIDTH)
  const label = accountLabel(account)
  return (
    <div ref={anchorRef} onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)}>
      <MenuItem
        icon={
          <AccountAvatar accountId={account.id} name={label} src={account.avatar_url} size={18} className="shrink-0" />
        }
        label={
          <span
            className="min-w-0 truncate"
            title={isLocalAccountId(account.id) ? `${label} (${t('accounts.local.subtitle')})` : label}
          >
            {label}
          </span>
        }
        submenuOpen={open}
        trailing={<ChevronRight size={13} className="text-secondary" />}
        onClick={() => setOpen(!open)}
      />
      {open && position && (
        <FloatingContextMenu
          x={position.x}
          y={position.y}
          className={`${flyoutClass} min-w-[230px]`}
          dataAttribute="data-thread-context-menu"
        >
          {folders.length === 0 && <FolderListStatus loading={loading} />}
          <FolderMenuTree folders={folders} excludedFolderIds={excludedFolderIds} onPick={onPick} />
        </FloatingContextMenu>
      )}
    </div>
  )
}
