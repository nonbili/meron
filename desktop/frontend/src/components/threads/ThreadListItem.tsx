import type { DragEvent, MouseEvent, Ref } from 'react'
import { Check, Star } from 'lucide-react'
import type { Account, Message } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { formatThreadDate } from '../../lib/date'
import { clsx } from '../../lib/utils'
import { useTranslation } from '../../lib/i18n'
import { isDraftFolder } from '../../states/mail'

export function ThreadListItem({
  thread,
  accounts,
  selectedAccount,
  selectedThread,
  active,
  onSelect,
  onContextMenu,
  draggable,
  onDragStart,
  onDragEnd,
  className = '',
  rootRef,
  showAccountBadge,
  bulkSelectable = false,
  bulkSelected = false,
}: {
  thread: Message
  accounts: Account[]
  selectedAccount: string
  selectedThread: string
  active?: boolean
  onSelect: (event: MouseEvent<HTMLButtonElement>) => void
  onContextMenu?: (event: MouseEvent) => void
  draggable?: boolean
  onDragStart?: (event: DragEvent<HTMLDivElement>) => void
  onDragEnd?: (event: DragEvent<HTMLDivElement>) => void
  className?: string
  rootRef?: Ref<HTMLDivElement>
  showAccountBadge?: boolean
  bulkSelectable?: boolean
  bulkSelected?: boolean
}) {
  const { t } = useTranslation()
  const isActive = active ?? thread.thread_id === selectedThread
  const threadAccount = accounts.find((acc) => acc.id === thread.account_id)
  const badgeLabel = threadAccount ? threadAccount.display_name || threadAccount.email : ''
  const accountBadgeVisible = showAccountBadge ?? selectedAccount === 'unified'
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
      className={clsx(
        'group relative border-b border-border/50 last:border-b-0',
        draggable && 'cursor-grab active:cursor-grabbing',
        className,
      )}
    >
      <button
        className={clsx(
          'relative w-full px-2 py-3 transition-all duration-150 flex items-center gap-2 cursor-pointer select-none text-left',
          bulkSelectable
            ? bulkSelected
              ? 'bg-accent/[0.13] text-primary'
              : 'bg-chats hover:bg-hover text-primary'
            : isActive
              ? 'bg-accent/20 dark:bg-accent/30 text-primary'
              : unread
                ? 'bg-accent/[0.07] hover:bg-accent/[0.12] text-primary'
                : 'bg-chats hover:bg-hover text-primary',
        )}
        onClick={onSelect}
        onContextMenu={onContextMenu}
        title={threadTitle}
      >
        {!bulkSelectable && isActive && (
          <span aria-hidden="true" className="absolute inset-y-0 left-0 w-[3px] bg-accent" />
        )}
        {bulkSelectable ? (
          <span aria-hidden="true" className="flex h-10 w-10 shrink-0 items-center justify-center">
            <span
              className={clsx(
                'flex h-7 w-7 items-center justify-center rounded-full transition-colors',
                bulkSelected
                  ? 'bg-accent text-white shadow-sm shadow-accent/20'
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
            {accountBadgeVisible && threadAccount && (
              <div className="absolute -bottom-1 -left-1 rounded-full ring-2 ring-chats overflow-hidden">
                <Avatar name={badgeLabel} src={threadAccount.avatar_url} size={16} />
              </div>
            )}
          </div>
        )}

        <div className="flex-1 min-w-0 flex flex-col justify-center gap-1">
          <div className="flex items-center gap-2 min-w-0">
            <div className="flex min-w-0 items-center gap-1">
              <span
                className={clsx('text-[0.8125rem] font-semibold truncate', unread ? 'text-primary' : 'text-primary/85')}
              >
                {thread.from_name || thread.from_addr.split('@')[0]}
                {!!thread.recipient_overflow && (
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
                'ml-auto shrink-0 text-[0.6875rem] font-normal',
                unread ? 'text-accent' : 'text-secondary/65',
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
              <span className="h-4 min-w-4 px-1 flex items-center justify-center rounded-full bg-accent text-white text-[0.625rem] font-bold shadow-sm shadow-accent/20 leading-none shrink-0">
                {thread.unread_count ?? 1}
              </span>
            ) : null}
          </div>
        </div>
      </button>
    </div>
  )
}
