import { useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState, type Ref } from 'react'
import {
  Archive,
  ChevronRight,
  Copy,
  Download,
  FolderInput,
  Mail,
  MailOpen,
  MoreHorizontal,
  Star,
  Trash2,
  ListChecks,
  X,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import type { BulkSelectionItem } from '../../states/ui'
import { clearBulkSelection, setBulkSelection } from '../../states/ui'
import { accounts$, isLocalAccountId, isMailFolderAccount } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { bulkMarkSelectedRead, bulkMarkSelectedUnread, bulkStarSelected } from '../../states/mailFlags'
import { deletesForever, ensureAccountFolders } from '../../states/mailFolders'
import {
  bulkArchiveSelected,
  bulkCopySelectedToFolder,
  bulkDeleteSelected,
  bulkMoveSelectedToFolder,
} from '../../states/mailMoves'
import { exportThreadsAsEml } from '../../states/emlExport'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { FolderMenuTree } from '../menu/FolderMenuTree'
import { MenuItem } from '../menu/MenuItem'

export type BulkActionBarHandle = {
  /** Open the bulk actions menu at a viewport point, e.g. a right-click on a row. */
  openMenuAt: (x: number, y: number) => void
}

export function BulkActionBar({
  items,
  allItems,
  className,
  ref,
}: {
  items: BulkSelectionItem[]
  /** Every selectable row of the surface the bar belongs to, in display order — backs "select all". */
  allItems?: BulkSelectionItem[]
  className?: string
  ref?: Ref<BulkActionBarHandle>
}) {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const [moveOpen, setMoveOpen] = useState(false)
  const [copyOpen, setCopyOpen] = useState(false)
  const [copyLoading, setCopyLoading] = useState(false)
  const [, setCopyFoldersVersion] = useState(0)
  const moveAnchorRef = useRef<HTMLDivElement | null>(null)
  const copyAnchorRef = useRef<HTMLDivElement | null>(null)
  const copyLoadRef = useRef(false)
  const [moveFlyoutPosition, setMoveFlyoutPosition] = useState<{ x: number; y: number } | null>(null)
  const [copyFlyoutPosition, setCopyFlyoutPosition] = useState<{ x: number; y: number } | null>(null)
  const mailItems = useMemo(() => items.filter((item) => item.kind === 'mail'), [items])
  const mailAccounts = accounts.filter(isMailFolderAccount)
  const accountIds = Array.from(new Set(mailItems.map((item) => item.accountId)))
  const singleAccountId = accountIds.length === 1 ? accountIds[0] : ''
  const canArchive =
    mailItems.length > 0 && mailItems.every((item) => !item.draft && !item.trash && !isLocalAccountId(item.accountId))
  const canMove = mailItems.length > 0 && mailAccounts.length > 0
  const canCopy = mailItems.length > 0 && mailAccounts.length > 0
  // Mirrors deleteBulkTargets: drafts and Trash-like folders delete forever, the rest move to Trash.
  const permanentDeletes = mailItems.filter(
    (item) => item.draft || item.trash || deletesForever(item.accountId, item.folderId),
  ).length
  const deleteLabel =
    permanentDeletes === 0
      ? t('threads.actions.moveToTrash')
      : permanentDeletes < mailItems.length
        ? t('buttons.delete')
        : mailItems.every((item) => item.draft)
          ? t('threads.actions.discardDraft')
          : t('threads.actions.deleteForever')

  useImperativeHandle(ref, () => ({ openMenuAt: (x, y) => setMenu({ x, y }) }), [])

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

  useEffect(() => {
    if (singleAccountId) void ensureAccountFolders(singleAccountId, { refreshIfBootstrapOnly: true })
  }, [singleAccountId])

  useEffect(() => {
    if (!menu) {
      setMoveOpen(false)
      setCopyOpen(false)
    }
  }, [menu])

  useEffect(() => {
    if (!copyOpen && !moveOpen) return
    void loadCopyTargetFolders()
  }, [accounts, copyOpen, moveOpen])

  useLayoutEffect(() => {
    if (!moveOpen) {
      setMoveFlyoutPosition(null)
      return
    }

    const anchor = moveAnchorRef.current
    if (!anchor) return

    const gap = 4
    const flyoutWidth = 230
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

  if (items.length === 0) return null

  const copyAccountGroups = mailAccounts.map((account) => ({
    account,
    folders: foldersByAccount[account.id] ?? [],
  }))

  const selectedLabel = items.length === 1 ? '1 selected' : `${items.length} selected`
  // Hidden once every row is already selected, so the menu never offers a no-op.
  const selectableItems = allItems ?? []
  const canSelectAll = selectableItems.some((item) => !items.some((selected) => selected.key === item.key))

  return (
    <div
      className={clsx(
        'shrink-0 border-b border-accent/15 bg-accent/[0.045] px-3 select-none dark:bg-accent/[0.08]',
        className,
      )}
    >
      <div className="flex min-h-[inherit] items-center gap-3">
        <IconButton
          icon={X}
          size="md"
          radius="lg"
          label={t('common.close')}
          className="text-secondary hover:bg-accent/10 hover:text-primary"
          onClick={clearBulkSelection}
        />
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <span className="min-w-0 truncate text-sm font-semibold text-primary">{selectedLabel}</span>
        </div>
        {canSelectAll && (
          <IconButton
            icon={ListChecks}
            size="md"
            radius="lg"
            label={t('threads.actions.selectAll')}
            className="hover:bg-accent/10"
            onClick={() => setBulkSelection(selectableItems)}
          />
        )}
        <IconButton
          icon={MoreHorizontal}
          size="md"
          radius="lg"
          label={t('common.more')}
          active={!!menu}
          className="hover:bg-accent/10"
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect()
            setMenu({ x: rect.right - 190, y: rect.bottom + 4 })
          }}
        />
      </div>

      {menu && (
        <FloatingContextMenu
          x={menu.x}
          y={menu.y}
          overlay
          onClose={() => setMenu(null)}
          className="fixed z-50 min-w-[230px] shadow-xl"
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          <MenuItem
            icon={<MailOpen size={13} className="text-secondary" />}
            label={t('threads.actions.markAsRead')}
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void bulkMarkSelectedRead(mailItems)
            }}
          />
          <MenuItem
            icon={<Mail size={13} className="text-secondary" />}
            label={t('threads.actions.markAsUnread')}
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void bulkMarkSelectedUnread(mailItems)
            }}
          />
          <MenuItem
            icon={<Star size={13} className="text-secondary" />}
            label={t('threads.actions.starThread')}
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void bulkStarSelected(mailItems, true)
            }}
          />
          <MenuItem
            icon={<Star size={13} className="fill-amber-500 text-amber-500" />}
            label={t('threads.actions.unstarThread')}
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void bulkStarSelected(mailItems, false)
            }}
          />
          <div className="my-1 border-t border-border" />
          <MenuItem
            icon={<Archive size={13} className="text-secondary" />}
            label={t('threads.actions.archiveThread')}
            disabled={!canArchive}
            onClick={() => {
              setMenu(null)
              void bulkArchiveSelected(items)
            }}
          />
          <div
            ref={moveAnchorRef}
            onMouseEnter={() => {
              if (!canMove) return
              setCopyOpen(false)
              setMoveOpen(true)
            }}
            onMouseLeave={() => setMoveOpen(false)}
          >
            <MenuItem
              icon={<FolderInput size={13} className="text-secondary" />}
              label={t('threads.actions.moveTo')}
              submenuOpen={moveOpen}
              trailing={<ChevronRight size={13} className="text-secondary" />}
              disabled={!canMove}
              onClick={() => {
                if (!canMove) return
                setMoveOpen((open) => !open)
              }}
            />
            {moveOpen && moveFlyoutPosition && (
              <FloatingContextMenu
                x={moveFlyoutPosition.x}
                y={moveFlyoutPosition.y}
                className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[230px] overflow-y-auto shadow-xl"
              >
                {copyAccountGroups.map(({ account, folders }) => (
                  <div key={account.id}>
                    <div className="px-3 pb-1 pt-2 text-[0.6875rem] font-semibold text-secondary">
                      {account.display_name || account.email || account.id}
                    </div>
                    {folders.length === 0 && (
                      <div className="px-3 py-2 text-xs font-semibold text-secondary">
                        {copyLoading ? t('folders.loading') : t('folders.noneAvailable')}
                      </div>
                    )}
                    <FolderMenuTree
                      folders={folders}
                      excludedFolderIds={folders
                        .filter((folder) =>
                          mailItems.every((item) => item.accountId === account.id && item.folderId === folder.id),
                        )
                        .map((folder) => folder.id)}
                      onPick={(folder) => {
                        setMenu(null)
                        setMoveOpen(false)
                        void bulkMoveSelectedToFolder(items, folder.id, account.id)
                      }}
                    />
                  </div>
                ))}
              </FloatingContextMenu>
            )}
          </div>
          <div
            ref={copyAnchorRef}
            onMouseEnter={() => {
              if (!canCopy) return
              setMoveOpen(false)
              setCopyOpen(true)
              void loadCopyTargetFolders()
            }}
            onMouseLeave={() => setCopyOpen(false)}
          >
            <MenuItem
              icon={<Copy size={13} className="text-secondary" />}
              label={t('threads.actions.copyTo')}
              submenuOpen={copyOpen}
              trailing={<ChevronRight size={13} className="text-secondary" />}
              disabled={!canCopy}
              onClick={() => {
                if (!canCopy) return
                setCopyOpen((open) => !open)
                void loadCopyTargetFolders()
              }}
            />
            {copyOpen && copyFlyoutPosition && (
              <FloatingContextMenu
                x={copyFlyoutPosition.x}
                y={copyFlyoutPosition.y}
                className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[230px] overflow-y-auto shadow-xl"
              >
                {copyAccountGroups.map(({ account, folders }) => (
                  <div key={account.id}>
                    <div className="px-3 pb-1 pt-2 text-[0.6875rem] font-semibold text-secondary">
                      {account.display_name || account.email || account.id}
                    </div>
                    {folders.length === 0 && (
                      <div className="px-3 py-2 text-xs font-semibold text-secondary">
                        {copyLoading ? t('folders.loading') : t('folders.noneAvailable')}
                      </div>
                    )}
                    <FolderMenuTree
                      folders={folders}
                      onPick={(folder) => {
                        setMenu(null)
                        setCopyOpen(false)
                        void bulkCopySelectedToFolder(mailItems, account.id, folder.id)
                      }}
                    />
                  </div>
                ))}
              </FloatingContextMenu>
            )}
          </div>
          <MenuItem
            icon={<Download size={13} className="text-secondary" />}
            label={t('chat.actions.saveAsEml')}
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void exportThreadsAsEml(mailItems, '')
            }}
          />
          <div className="my-1 border-t border-border" />
          <MenuItem
            icon={<Trash2 size={13} />}
            label={deleteLabel}
            danger
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void bulkDeleteSelected(items)
            }}
          />
        </FloatingContextMenu>
      )}
    </div>
  )
}
