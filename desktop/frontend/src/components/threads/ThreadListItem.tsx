import type { DragEvent, MouseEvent, Ref } from 'react'
import { Check, Star } from 'lucide-react'
import type { Account, Message } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { AccountAvatar } from '../avatar/AccountAvatar'
import { formatThreadDate } from '../../lib/date'
import { clsx } from '../../lib/utils'
import { useTranslation } from '../../lib/i18n'
import { isDraftFolder } from '../../states/mailFolders'
import { AttachmentChips } from './AttachmentChips'

export function ThreadListItem({
  thread,
  active = false,
  badgeAccount,
  onSelect,
  onOpenInNewTab,
  onContextMenu,
  contextMenuOpen = false,
  draggable,
  onDragStart,
  onDragEnd,
  className = '',
  rootRef,
  bulkSelectable = false,
  bulkSelected = false,
}: {
  thread: Message
  active?: boolean
  // The account the thread belongs to, badged on the avatar in lists that mix
  // accounts; omitted elsewhere.
  badgeAccount?: Account
  onSelect: (event: MouseEvent<HTMLButtonElement>) => void
  // Double-click opens the row in its own tab (single click previews it).
  onOpenInNewTab?: () => void
  onContextMenu?: (event: MouseEvent) => void
  contextMenuOpen?: boolean
  draggable?: boolean
  onDragStart?: (event: DragEvent<HTMLDivElement>) => void
  onDragEnd?: (event: DragEvent<HTMLDivElement>) => void
  className?: string
  rootRef?: Ref<HTMLDivElement>
  bulkSelectable?: boolean
  bulkSelected?: boolean
}) {
  const { t } = useTranslation()
  const threadTitle = thread.subject || '(no subject)'
  // RSS feed rows carry a feed_url (and, once cached, a feed_icon). For those,
  // skip the email-based gravatar/favicon resolution and use the feed's icon.
  const isRSS = !!thread.feed_url
  const unread = thread.unread
  const hasDraft = !isRSS && (thread.has_draft || isDraftFolder(thread.folder_id, thread.account_id))

  return (
    <div
      ref={rootRef}
      draggable={draggable}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      className={clsx('group relative', draggable && 'cursor-grab active:cursor-grabbing', className)}
    >
      <button
        data-thread-row
        data-active={active || undefined}
        data-bulk-selectable={bulkSelectable || undefined}
        className={clsx(
          // Rows are inset pills, told apart by spacing rather than divider
          // lines; unread weight and the count badge mark unread, not a tint.
          'relative w-full rounded-lg px-2.5 py-2.5 text-primary transition-colors duration-150 flex items-center gap-2.5 cursor-pointer select-none text-left',
          (bulkSelectable ? bulkSelected : active)
            ? 'bg-accent/[0.13] dark:bg-accent/20'
            : contextMenuOpen
              ? 'bg-hover'
              : 'hover:bg-hover',
        )}
        onClick={onSelect}
        onDoubleClick={onOpenInNewTab}
        onContextMenu={onContextMenu}
        title={threadTitle}
      >
        {bulkSelectable ? (
          <span aria-hidden="true" className="flex h-10 w-10 shrink-0 items-center justify-center">
            <span
              className={clsx(
                'flex h-7 w-7 items-center justify-center rounded-full transition-colors',
                bulkSelected
                  ? 'bg-accent text-accent-label shadow-sm shadow-accent/20'
                  : 'border border-secondary/30 text-secondary/35',
              )}
            >
              <Check size={17} strokeWidth={2.6} />
            </span>
          </span>
        ) : (
          <div className="relative shrink-0">
            <Avatar
              name={thread.from_name || thread.from_addr}
              email={isRSS ? undefined : thread.from_addr}
              src={isRSS && thread.feed_icon ? `/media/${thread.feed_icon}` : undefined}
            />
            {badgeAccount && (
              <div className="absolute -bottom-1 -left-1 rounded-full ring-2 ring-chats overflow-hidden">
                <AccountAvatar
                  accountId={badgeAccount.id}
                  name={badgeAccount.display_name || badgeAccount.email}
                  src={badgeAccount.avatar_url}
                  size={16}
                />
              </div>
            )}
          </div>
        )}

        <div className="flex-1 min-w-0 flex flex-col justify-center gap-1">
          <div className="flex items-center gap-2 min-w-0">
            <div className="flex min-w-0 items-center gap-1">
              <span
                className={clsx(
                  'text-[0.8125rem] truncate',
                  unread ? 'font-semibold text-primary' : 'font-medium text-primary/80',
                )}
              >
                {thread.senders && thread.senders.length > 1
                  ? thread.senders
                      .map((sender) => (sender.me ? t('threads.me') : sender.name))
                      .reduce(
                        (label, name, index) =>
                          `${label}${index === 1 && thread.senders_truncated ? ' .. ' : ', '}${name}`,
                      )
                  : thread.from_name || thread.from_addr.split('@')[0]}
                {!thread.senders?.length && !!thread.recipient_overflow && (
                  <span className="ml-1 font-normal text-secondary/80">+{thread.recipient_overflow}</span>
                )}
              </span>
              {/* Gmail-style thread size, muted and only once a thread has a
                  reply. It sits right after the sender rather than in the
                  trailing badge slot so it never competes with the unread
                  count, and outside the truncating span so a long sender
                  ellipsises itself instead of clipping the count. */}
              {(thread.message_count ?? 0) > 1 && (
                <span className="shrink-0 text-[0.75rem] font-normal text-secondary/70">{thread.message_count}</span>
              )}
            </div>
            <time
              className={clsx(
                'ml-auto shrink-0 text-[0.6875rem]',
                unread ? 'font-medium text-primary' : 'font-normal text-secondary/65',
              )}
            >
              {formatThreadDate(thread.date)}
            </time>
          </div>

          <div className="flex items-center gap-1.5 min-w-0">
            {!bulkSelectable && thread.starred && <Star size={11} className="fill-amber-500 text-amber-500 shrink-0" />}
            <p
              className={clsx('flex-1 truncate text-[0.75rem] leading-snug', unread ? 'font-semibold' : 'font-normal')}
            >
              {hasDraft && <span className="mr-1 font-normal text-rose-500">{t('chat.draft')}</span>}
              <span className={clsx(unread ? 'text-primary' : 'text-primary/85')}>{threadTitle}</span>
              {thread.preview && (
                <span className={clsx(unread ? 'text-secondary/90 font-medium' : 'text-secondary/75 font-normal')}>
                  {' - '}
                  {thread.preview}
                </span>
              )}
            </p>
            {unread && bulkSelectable ? (
              <span className="h-2 w-2 shrink-0 rounded-full bg-accent" />
            ) : unread ? (
              <span className="h-4 min-w-4 px-1 flex items-center justify-center rounded-full bg-unread-bg text-unread-text text-[0.625rem] font-semibold leading-none shrink-0">
                {thread.unread_count ?? 1}
              </span>
            ) : null}
          </div>
          {!!thread.files?.length && <AttachmentChips files={thread.files} />}
        </div>
      </button>
    </div>
  )
}
