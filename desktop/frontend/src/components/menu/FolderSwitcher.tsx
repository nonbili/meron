import { useEffect, useMemo, useState, type MouseEvent as ReactMouseEvent } from 'react'
import { useValue } from '@legendapp/state/react'
import { Bell, BellOff, Check, ChevronDown, ChevronRight } from 'lucide-react'
import { folderIcon } from '../../lib/folderIcon'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { mail$ } from '../../states/mail'
import { ensureAccountFolders, isNotifiableFolder, setFolderNotify } from '../../states/mailFolders'
import { UNIFIED_ACCOUNT, unifiedFolderLabel, unifiedFolders } from '../../lib/unifiedFolders'
import type { Folder } from '../../types'
import { FloatingContextMenu } from './FloatingContextMenu'
import { MenuItem } from './MenuItem'
import { menuItemBase } from './menuStyles'
import { buildFolderTree, type TreeNode } from '../../lib/folderTree'

const FILTER_THRESHOLD = 8

// One folder in the picker tree: the expander, the folder row itself and, when
// expanded, its children. Structural nodes (a path segment with no folder of its
// own) are shown but not selectable.
function FolderNodeRow({
  node,
  depth,
  currentFolderId,
  takenFolderIds,
  nested,
  onPick,
  onFolderMenu,
}: {
  node: TreeNode
  depth: number
  currentFolderId: string
  takenFolderIds?: string[]
  /** Whether any row in the tree has children; a flat list drops the expander gutter. */
  nested: boolean
  onPick: (folderId: string) => void
  /** Right-click on a folder that has actions of its own; absent when none do. */
  onFolderMenu?: (event: ReactMouseEvent<HTMLElement>, folder: Folder) => void
}) {
  const [expanded, setExpanded] = useState(true)
  const hasChildren = node.children.length > 0
  const current = !!node.folder && node.folder.id === currentFolderId
  const taken = !!node.folder && !current && !!takenFolderIds?.includes(node.folder.id)
  const selectable = !!node.folder && !current && !taken
  const Icon = folderIcon(node.folder)

  return (
    <div>
      <div className="flex items-center" style={{ paddingLeft: depth * 14 }}>
        {nested && (
          <button
            type="button"
            className={clsx(
              'flex h-8 w-5 shrink-0 items-center justify-center rounded text-secondary',
              hasChildren ? 'cursor-pointer hover:text-primary' : 'invisible',
            )}
            tabIndex={hasChildren ? 0 : -1}
            onClick={() => setExpanded((open) => !open)}
          >
            <ChevronRight size={13} className={clsx('transition-transform', expanded && 'rotate-90')} />
          </button>
        )}
        <button
          type="button"
          disabled={!selectable}
          className={clsx(
            menuItemBase,
            'min-w-0 flex-1',
            current ? 'font-semibold text-accent' : 'text-primary',
            selectable ? 'hover:bg-hover' : 'cursor-default',
            taken && 'opacity-40',
            !node.folder && 'text-secondary',
          )}
          onClick={() => node.folder && onPick(node.folder.id)}
          onContextMenu={(event) => {
            if (node.folder && onFolderMenu && isNotifiableFolder(node.folder)) onFolderMenu(event, node.folder)
          }}
        >
          {current ? (
            <Check size={13} className="shrink-0 text-accent" />
          ) : (
            <Icon size={13} className="shrink-0 text-secondary" />
          )}
          <span className="min-w-0 truncate">{node.name}</span>
        </button>
      </div>
      {hasChildren && expanded && (
        <div>
          {node.children.map((child) => (
            <FolderNodeRow
              key={child.folder?.id ?? `${depth}-${child.name}`}
              node={child}
              depth={depth + 1}
              currentFolderId={currentFolderId}
              takenFolderIds={takenFolderIds}
              nested={nested}
              onPick={onPick}
              onFolderMenu={onFolderMenu}
            />
          ))}
        </div>
      )}
    </div>
  )
}

// A folder name that doubles as a picker: clicking it lists the other folders of
// the same account so the surface showing it (a kanban column, the thread list)
// can be pointed elsewhere without being torn down and rebuilt.
export function FolderSwitcher({
  accountId,
  folderId,
  label,
  labelClassName,
  title,
  takenFolderIds,
  onSelect,
}: {
  accountId: string
  folderId: string
  label: string
  labelClassName?: string
  /** Tooltip for the trigger, when picking does something other than switch folders. */
  title?: string
  /** Folders already shown elsewhere (e.g. another column) and so not offered. */
  takenFolderIds?: string[]
  onSelect: (folderId: string) => void
}) {
  const { t } = useTranslation()
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const [folderMenu, setFolderMenu] = useState<{ x: number; y: number; folderId: string } | null>(null)
  const isUnified = accountId === UNIFIED_ACCOUNT
  // Keep observing the shared cache: when it contains only the bootstrap Inbox,
  // ensureAccountFolders refreshes it in the background.
  const cachedFolders = useValue(mail$.foldersByAccount[accountId]) ?? []
  // The unified view has no folders of its own to fetch — its list is the fixed
  // set of roles, each resolved per account at read time. Building it here also
  // keeps the picker working on a Kanban board, where nothing has necessarily
  // populated the unified entry of the shared folder cache. The rows are named
  // "Unified inbox" rather than plain "Inbox" so the label they set on the
  // header says by itself whose mail is listed.
  const folders = useMemo(
    () =>
      isUnified
        ? unifiedFolders(t).map((folder) => ({ ...folder, name: unifiedFolderLabel(folder.id, t) }))
        : cachedFolders,
    [isUnified, t, cachedFolders],
  )
  const [loading, setLoading] = useState(false)
  const [query, setQuery] = useState('')

  useEffect(() => {
    if (!menu || isUnified) return
    let cancelled = false
    setLoading(true)
    void ensureAccountFolders(accountId, { refreshIfBootstrapOnly: true }).finally(() => {
      if (!cancelled) setLoading(false)
    })
    return () => {
      cancelled = true
    }
  }, [menu, accountId, isUnified])

  const close = () => {
    setMenu(null)
    setFolderMenu(null)
    setQuery('')
  }

  const open = (event: ReactMouseEvent<HTMLButtonElement>) => {
    event.preventDefault()
    event.stopPropagation()
    if (menu) {
      close()
      return
    }
    const rect = event.currentTarget.getBoundingClientRect()
    setMenu({ x: rect.left, y: rect.bottom })
  }

  const needle = query.trim().toLowerCase()
  // Filtering narrows the folder set, then the tree is rebuilt from what's left,
  // so matches keep the hierarchy they sit in.
  const tree = useMemo(
    () => buildFolderTree(folders.filter((folder) => !needle || folder.name.toLowerCase().includes(needle))),
    [folders, needle],
  )
  const nested = tree.some((node) => node.children.length > 0)
  const showFilter = folders.length > FILTER_THRESHOLD
  // Looked up live so the row's label follows the folder's state.
  const menuFolder = folderMenu && folders.find((folder) => folder.id === folderMenu.folderId)

  return (
    <>
      <button
        type="button"
        // Sized well past the label's own line box: the headers hosting this are
        // 48px+ tall, so a text-height hit target left most of it dead. The padding
        // is the caller's to pull back with a negative margin if it wants the label
        // flush with the rest of the header.
        className={clsx('flex h-8 min-w-0 items-center gap-1 rounded px-2 hover:bg-hover', labelClassName)}
        title={title ?? t('kanban.actions.switchFolder')}
        onClick={open}
        onContextMenu={(event) => event.stopPropagation()}
      >
        <span className="min-w-0 truncate">{label}</span>
        <ChevronDown size={12} className="shrink-0 text-secondary" />
      </button>
      {menu && (
        <FloatingContextMenu
          x={menu.x}
          y={menu.y}
          offset={2}
          onClose={close}
          overlay
          className="fixed z-50 flex max-h-[min(420px,calc(100vh-1rem))] w-60 flex-col rounded-xl border border-border bg-chats p-1 shadow-2xl animate-fade-in text-primary"
          onContextMenu={(event) => {
            event.preventDefault()
            event.stopPropagation()
          }}
        >
          {showFilter && (
            <input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') close()
              }}
              placeholder={t('folders.searchPlaceholder')}
              className="mb-1 h-8 w-full shrink-0 rounded-lg bg-hover px-2 text-[0.8125rem] text-primary outline-none placeholder-secondary focus:ring-1 focus:ring-accent/40"
            />
          )}
          <div className="min-h-0 flex-1 overflow-y-auto">
            {tree.length === 0 ? (
              <div className="px-2 py-4 text-center text-xs font-medium text-secondary">
                {loading ? t('folders.loading') : t('folders.noneAvailable')}
              </div>
            ) : (
              tree.map((node) => (
                <FolderNodeRow
                  key={node.folder?.id ?? node.name}
                  node={node}
                  depth={0}
                  currentFolderId={folderId}
                  takenFolderIds={takenFolderIds}
                  nested={nested}
                  onPick={(picked) => {
                    close()
                    onSelect(picked)
                  }}
                  onFolderMenu={
                    isUnified
                      ? undefined
                      : (event, folder) => {
                          event.preventDefault()
                          event.stopPropagation()
                          setFolderMenu({ x: event.clientX, y: event.clientY, folderId: folder.id })
                        }
                  }
                />
              ))
            )}
          </div>
        </FloatingContextMenu>
      )}
      {folderMenu && menuFolder && (
        <FloatingContextMenu
          x={folderMenu.x}
          y={folderMenu.y}
          offset={4}
          onClose={() => setFolderMenu(null)}
          overlay
          className="fixed z-50 min-w-[176px] rounded-xl border border-border bg-chats p-1 shadow-2xl animate-fade-in text-primary"
          onContextMenu={(event) => {
            event.preventDefault()
            event.stopPropagation()
          }}
        >
          <MenuItem
            className="flex-nowrap"
            icon={
              menuFolder.notify ? (
                <BellOff size={13} className="text-secondary shrink-0" />
              ) : (
                <Bell size={13} className="text-secondary shrink-0" />
              )
            }
            label={
              <span className="whitespace-nowrap shrink-0">
                {menuFolder.notify ? t('folders.notify.disable') : t('folders.notify.enable')}
              </span>
            }
            onClick={() => {
              setFolderMenu(null)
              void setFolderNotify(accountId, menuFolder.id, !menuFolder.notify, menuFolder.name)
            }}
          />
        </FloatingContextMenu>
      )}
    </>
  )
}
