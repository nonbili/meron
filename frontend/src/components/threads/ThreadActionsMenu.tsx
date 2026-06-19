import { useRef, useState } from 'react'
import { MoreVertical, Inbox, Mail, Star, CheckCheck, EyeOff, RefreshCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { FilterMode } from '../../states/ui'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { MenuItem } from '../menu/MenuItem'
import { menuItemBase } from '../menu/menuStyles'

// Shared overflow menu for a list of threads: filter (all / unread / starred)
// plus mark-all-as-read. Used by the chat thread list header and each kanban
// column header. Purely presentational; the host wires state and actions.
export function ThreadActionsMenu({
  filterMode,
  onFilterChange,
  hasUnread,
  onMarkAllRead,
  onSync,
  syncing = false,
  syncLabel,
  syncingLabel,
  allLabel,
  onRemove,
  size = 16,
  triggerClassName = 'h-8 w-8',
}: {
  filterMode: FilterMode
  onFilterChange: (mode: FilterMode) => void
  hasUnread: boolean
  onMarkAllRead: () => void
  onSync?: () => void
  syncing?: boolean
  syncLabel?: string
  syncingLabel?: string
  allLabel?: string
  onRemove?: () => void
  size?: number
  triggerClassName?: string
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement | null>(null)

  useDismissOnOutside(
    open,
    (target) => Boolean(rootRef.current?.contains(target as Node)),
    () => setOpen(false),
  )

  const filterActive = filterMode !== 'all'

  const filterItem = (mode: FilterMode, label: string, icon: React.ReactNode) => (
    <button
      className={`${menuItemBase} flex-nowrap ${
        filterMode === mode ? 'bg-accent/10 dark:bg-accent/15 text-accent' : 'text-primary hover:bg-hover'
      }`}
      onClick={() => {
        onFilterChange(mode)
        setOpen(false)
      }}
    >
      {icon}
      <span className="whitespace-nowrap shrink-0">{label}</span>
    </button>
  )

  return (
    <div ref={rootRef} className="relative">
      <button
        className={`flex ${triggerClassName} shrink-0 items-center justify-center rounded-lg cursor-pointer transition-all ${
          filterActive ? 'bg-accent text-white shadow-sm shadow-accent/20' : 'hover:bg-hover text-secondary'
        }`}
        onClick={(event) => {
          event.stopPropagation()
          setOpen((value) => !value)
        }}
        title={t('threads.actions.title')}
      >
        <MoreVertical size={size} />
      </button>
      {open && (
        <div
          className="absolute right-0 mt-1.5 z-50 min-w-[160px] w-max rounded-xl border border-border bg-chats p-1 shadow-2xl animate-fade-in select-none"
          onClick={(event) => event.stopPropagation()}
        >
          {filterItem('all', allLabel ?? t('filters.all'), <Inbox size={13} className="text-secondary shrink-0" />)}
          {filterItem('unread', t('filters.unread'), <Mail size={13} className="text-secondary shrink-0" />)}
          {filterItem('starred', t('filters.starred'), <Star size={13} className="text-secondary shrink-0" />)}
          <div className="my-1 border-t border-border" />
          {onSync && (
            <MenuItem
              className="flex-nowrap"
              icon={
                <RefreshCw
                  size={13}
                  className={`text-secondary shrink-0 ${syncing ? 'animate-spin text-accent' : ''}`}
                />
              }
              label={
                <span className="whitespace-nowrap shrink-0">
                  {syncing
                    ? (syncingLabel ?? t('threads.actions.syncing'))
                    : (syncLabel ?? t('threads.actions.syncMailbox'))}
                </span>
              }
              onClick={() => {
                onSync()
                setOpen(false)
              }}
            />
          )}
          <MenuItem
            className="flex-nowrap disabled:cursor-not-allowed disabled:opacity-50"
            disabled={!hasUnread}
            icon={<CheckCheck size={13} className="text-secondary shrink-0" />}
            label={<span className="whitespace-nowrap shrink-0">{t('threads.actions.markAllAsRead')}</span>}
            onClick={() => {
              onMarkAllRead()
              setOpen(false)
            }}
          />
          {onRemove && (
            <>
              <div className="my-1 border-t border-border" />
              <MenuItem
                className="flex-nowrap"
                icon={<EyeOff size={13} className="text-secondary shrink-0" />}
                label={<span className="whitespace-nowrap shrink-0">{t('kanban.actions.hideColumn')}</span>}
                onClick={() => {
                  onRemove()
                  setOpen(false)
                }}
              />
            </>
          )}
        </div>
      )}
    </div>
  )
}
