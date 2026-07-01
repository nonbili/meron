import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Archive, ChevronRight, Copy, FolderInput, Mail, MailOpen, MoreHorizontal, Star, Trash2, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import type { BulkSelectionItem } from '../../states/ui'
import { clearBulkSelection } from '../../states/ui'
import { accounts$, isSendableAccount } from '../../states/accounts'
import {
  bulkArchiveSelected,
  bulkCopySelectedToFolder,
  bulkDeleteSelected,
  bulkMarkSelectedRead,
  bulkMarkSelectedUnread,
  bulkMoveSelectedToFolder,
  bulkStarSelected,
  ensureAccountFolders,
  mail$,
} from '../../states/mail'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

export function BulkActionBar({ items, className }: { items: BulkSelectionItem[]; className?: string }) {
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
  const mailAccounts = accounts.filter(isSendableAccount)
  const accountIds = Array.from(new Set(mailItems.map((item) => item.accountId)))
  const singleAccountId = accountIds.length === 1 ? accountIds[0] : ''
  const selectedFolders = new Set(mailItems.map((item) => item.folderId))
  const canArchive = mailItems.length > 0 && mailItems.every((item) => !item.draft && !item.trash)
  const canMove = mailItems.length > 0 && !!singleAccountId
  const canCopy = mailItems.length > 0 && mailAccounts.length > 0

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

  if (items.length === 0) return null

  const moveFolders = singleAccountId
    ? (foldersByAccount[singleAccountId] ?? []).filter((folder) => !selectedFolders.has(folder.id))
    : []
  const copyAccountGroups = mailAccounts.map((account) => ({
    account,
    folders: foldersByAccount[account.id] ?? [],
  }))

  const selectedLabel = items.length === 1 ? '1 selected' : `${items.length} selected`

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
          className="fixed z-50 min-w-[190px] rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
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
              void bulkArchiveSelected(mailItems)
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
                className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[190px] overflow-y-auto rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
              >
                {moveFolders.length === 0 && (
                  <div className="px-3 py-2 text-xs font-semibold text-secondary">{t('folders.noneAvailable')}</div>
                )}
                {moveFolders.map((folder) => (
                  <MenuItem
                    key={folder.id}
                    icon={<FolderInput size={13} className="shrink-0 text-secondary" />}
                    label={<span className="min-w-0 truncate">{folder.name}</span>}
                    onClick={() => {
                      setMenu(null)
                      setMoveOpen(false)
                      void bulkMoveSelectedToFolder(mailItems, folder.id)
                    }}
                  />
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
                className="fixed z-[51] max-h-[calc(100vh-1rem)] min-w-[230px] overflow-y-auto rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
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
                          setMenu(null)
                          setCopyOpen(false)
                          void bulkCopySelectedToFolder(mailItems, account.id, folder.id)
                        }}
                      />
                    ))}
                  </div>
                ))}
              </FloatingContextMenu>
            )}
          </div>
          <div className="my-1 border-t border-border" />
          <MenuItem
            icon={<Trash2 size={13} />}
            label={t('buttons.delete')}
            danger
            disabled={mailItems.length === 0}
            onClick={() => {
              setMenu(null)
              void bulkDeleteSelected(mailItems)
            }}
          />
        </FloatingContextMenu>
      )}
    </div>
  )
}
