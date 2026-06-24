import { Columns3, Pause, BellOff, KeyRound, Rss } from 'lucide-react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { useValue } from '@legendapp/state/react'
import type { DragEvent } from 'react'
import type { Account } from '../../types'
import { settings$ } from '../../states/settings'
import { mail$, inboxUnread } from '../../states/mail'
import { Avatar } from '../avatar/Avatar'
import { UnreadCountBadge } from './UnreadCountBadge'

function accountLabel(account: { display_name: string; email: string }) {
  return account.display_name || account.email
}

function sortableStyle(
  transform: ReturnType<typeof useSortable>['transform'],
  transition: string | undefined,
  isDragging: boolean,
) {
  return {
    transform: CSS.Translate.toString(transform),
    transition: isDragging ? 'none' : transition,
    zIndex: isDragging ? 10 : undefined,
    opacity: isDragging ? 0.5 : undefined,
    pointerEvents: isDragging ? ('none' as const) : undefined,
  }
}

const activeIndicator = (active: boolean) => (
  <div
    className={`absolute left-0 top-1/2 -translate-y-1/2 w-1 rounded-r bg-accent transition-all duration-200 ${
      active ? 'h-7' : 'h-0 group-hover:h-3'
    }`}
  />
)

interface SortableBoardProps {
  board: { id: string; name: string; avatarUrl?: string }
  active: boolean
  onSelect: () => void
  onContextMenu: (e: React.MouseEvent) => void
}

export function SortableBoard({ board, active, onSelect, onContextMenu }: SortableBoardProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: board.id })

  return (
    <div
      ref={setNodeRef}
      style={sortableStyle(transform, transition, isDragging) as any}
      {...attributes}
      {...listeners}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      className="relative w-full flex justify-center group cursor-move"
      title={board.name}
    >
      {activeIndicator(active)}
      {board.avatarUrl ? (
        <div
          className={`flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 ${
            active
              ? 'ring-2 ring-accent ring-offset-2 ring-offset-sidebar scale-105'
              : 'opacity-75 hover:opacity-100 hover:scale-105'
          }`}
        >
          <Avatar name={board.name} src={board.avatarUrl} size={44} className="!rounded-2xl pointer-events-none" />
        </div>
      ) : (
        <div
          className={`flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 ${
            active
              ? 'bg-accent text-white shadow-lg shadow-accent/25 scale-105'
              : 'bg-white/10 text-white/60 hover:bg-white/20 hover:text-white hover:scale-105'
          }`}
        >
          <Columns3 size={19} />
        </div>
      )}
    </div>
  )
}

interface SortableAccountProps {
  account: Account
  active: boolean
  onSelect: () => void
  onContextMenu: (e: React.MouseEvent) => void
  onFeedDragOver?: (e: DragEvent<HTMLDivElement>, account: Account) => void
  onFeedDrop?: (e: DragEvent<HTMLDivElement>, account: Account) => void
}

export function SortableAccount({
  account,
  active,
  onSelect,
  onContextMenu,
  onFeedDragOver,
  onFeedDrop,
}: SortableAccountProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: account.id })

  const isRSS = account.provider === 'rss' || account.auth_type === 'rss'
  const needsReconnect = account.needs_reconnect === true
  const isPaused = account.paused ?? false
  const isMuted = account.muted ?? false
  const showUnreadBadge = useValue(settings$.showUnreadAccountBadge)
  const accountFolders = useValue(mail$.foldersByAccount[account.id])
  const unreadCount = showUnreadBadge ? inboxUnread(accountFolders) : 0
  const baseTooltip = isRSS
    ? account.display_name || 'RSS Feeds'
    : account.display_name
      ? `${account.display_name} (${account.email})`
      : account.email
  const stateSuffix = needsReconnect ? ' — Needs reconnect' : isPaused ? ' — Paused' : isMuted ? ' — Muted' : ''

  return (
    <div
      ref={setNodeRef}
      style={sortableStyle(transform, transition, isDragging) as any}
      {...attributes}
      {...listeners}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      onDragOver={(event) => onFeedDragOver?.(event, account)}
      onDrop={(event) => onFeedDrop?.(event, account)}
      className="relative w-full flex justify-center group cursor-move"
      title={baseTooltip + stateSuffix}
    >
      {activeIndicator(active)}
      {/* Badges live on this wrapper, not the dimmed avatar div, so the unread
          count stays full-opacity for inactive accounts (matching the rail's
          other badges). */}
      <div className="relative">
        <div
          className={`relative flex h-11 w-11 items-center justify-center rounded-2xl transition-all duration-200 ${
            active
              ? 'ring-2 ring-accent ring-offset-2 ring-offset-sidebar scale-105'
              : isPaused || needsReconnect
                ? 'opacity-100 hover:scale-105'
                : 'opacity-75 hover:opacity-100 hover:scale-105'
          }`}
        >
          <Avatar
            name={accountLabel(account)}
            src={account.avatar_url}
            size={44}
            fallback={isRSS ? <Rss size={20} /> : undefined}
            className={`!rounded-2xl pointer-events-none transition-all ${
              isPaused || needsReconnect ? 'grayscale opacity-40' : ''
            }`}
          />
        </div>
        {(needsReconnect || isPaused || isMuted) && (
          <span
            className={`absolute -bottom-1 -right-1 flex h-[18px] w-[18px] items-center justify-center rounded-full text-white/90 ring-2 ring-sidebar ${
              needsReconnect ? 'bg-amber-600' : 'bg-black/60 text-white/80'
            }`}
            title={needsReconnect ? 'Needs reconnect' : isPaused ? 'Paused' : 'Muted'}
          >
            {needsReconnect ? (
              <KeyRound size={10} />
            ) : isPaused ? (
              <Pause size={9} className="fill-current" />
            ) : (
              <BellOff size={9} />
            )}
          </span>
        )}
        <UnreadCountBadge count={unreadCount} />
      </div>
    </div>
  )
}
