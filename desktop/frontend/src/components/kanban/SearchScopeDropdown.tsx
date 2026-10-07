import { useEffect, useMemo, useRef, useState } from 'react'
import { ChevronDown, Columns3, Rss } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { accounts$ } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { kanbanColumnKey, type KanbanColumn } from '../../states/kanban'
import { accountLabel, folderLabel, mergeLabelFolders, useFoldersByAccount } from '../../lib/kanbanData'
import { isRssAccount } from '../../lib/threadActions'
import { Avatar } from '../avatar/Avatar'

// Custom dropdown component for the search scope selector
export function SearchScopeDropdown({
  value,
  onChange,
  visibleColumns,
  compact = false,
  quiet = false,
}: {
  value: string
  onChange: (value: string) => void
  visibleColumns: KanbanColumn[]
  /** In the title bar: its colors, until the search box around it is focused. */
  compact?: boolean
  /** Soften the trigger while its search is empty and unfocused. */
  quiet?: boolean
}) {
  const { t } = useTranslation()
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const accounts = useValue(accounts$)
  const folderList = useValue(mail$.folders)
  const foldersByAccount = useFoldersByAccount()
  const folders = useMemo(() => mergeLabelFolders(folderList, foldersByAccount), [folderList, foldersByAccount])

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  useEscapeKey(() => setIsOpen(false), isOpen)

  const selectedColumn = value === 'all' ? undefined : visibleColumns.find((c) => kanbanColumnKey(c) === value)
  const selectedLabel = useMemo(
    () => (selectedColumn ? folderLabel(selectedColumn, folders, accounts, t) : t('kanban.searchScope.allColumns')),
    [selectedColumn, folders, accounts, t],
  )

  return (
    <div
      ref={containerRef}
      // The title bar's box keeps no divider, so the quiet search reads as one field.
      className={`relative h-full shrink-0 ${compact ? '' : 'border-l border-border/60'}`}
    >
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`flex h-full items-center gap-1.5 text-[0.6875rem] font-semibold transition-[color,opacity] cursor-pointer select-none outline-none border-0 ${
          quiet && !isOpen ? 'opacity-60 hover:opacity-100 group-focus-within:opacity-100' : ''
        } ${
          compact
            ? 'px-2.5 rounded-r-lg text-sidenav-ink/60 hover:text-sidenav-ink group-focus-within:text-secondary group-focus-within:hover:text-primary'
            : 'px-3.5 rounded-r-xl text-secondary hover:text-primary'
        }`}
        title={t('kanban.searchScope.label')}
      >
        {/* Whose folder it is, since a board can hold the same folder of several accounts. */}
        {selectedColumn && <ColumnAvatar column={selectedColumn} size={16} />}
        <span className="truncate max-w-[130px]">{selectedLabel}</span>
        <ChevronDown size={12} className={`transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`} />
      </button>
      {isOpen && (
        // data-no-drag: in the title bar, a press between the options must not move the window.
        <div
          data-no-drag
          className="absolute right-0 mt-1.5 z-50 w-64 max-h-72 overflow-y-auto rounded-2xl border border-border bg-chats p-1.5 shadow-xl shadow-black/10 dark:shadow-black/35 animate-slide-up"
        >
          <button
            type="button"
            onClick={() => {
              onChange('all')
              setIsOpen(false)
            }}
            className={`w-full flex items-center gap-2 px-3 py-2 text-xs rounded-xl transition-colors cursor-pointer select-none ${
              value === 'all' ? 'bg-accent/10 text-accent font-bold' : 'text-primary hover:bg-hover'
            }`}
          >
            <Columns3 size={13} className={value === 'all' ? 'text-accent' : 'text-secondary'} />
            <span className="font-semibold">{t('kanban.searchScope.allColumns')}</span>
          </button>

          <div className="my-1 border-t border-border/50" />

          {visibleColumns.map((column) => {
            const key = kanbanColumnKey(column)
            const isSelected = value === key
            const columnAccount =
              column.accountId !== 'unified' ? accounts.find((a) => a.id === column.accountId) : undefined
            const columnAccountLabel = columnAccount
              ? columnAccount.display_name || columnAccount.email || columnAccount.id
              : ''

            return (
              <button
                key={key}
                type="button"
                onClick={() => {
                  onChange(key)
                  setIsOpen(false)
                }}
                className={`w-full flex items-center gap-2.5 px-3 py-2 text-xs rounded-xl transition-colors cursor-pointer select-none ${
                  isSelected ? 'bg-accent/10 text-accent font-bold' : 'text-primary hover:bg-hover'
                }`}
              >
                <ColumnAvatar column={column} size={18} />
                <div className="min-w-0 flex-1 text-left">
                  <div className="truncate font-semibold">{folderLabel(column, folders, accounts, t)}</div>
                  <div className="truncate text-[0.625rem] text-secondary font-medium">
                    {column.accountId === 'unified' ? t('accounts.unified') : columnAccountLabel}
                  </div>
                </div>
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

function ColumnAvatar({ column, size }: { column: KanbanColumn; size: number }) {
  const accounts = useValue(accounts$)
  const account = column.accountId !== 'unified' ? accounts.find((a) => a.id === column.accountId) : undefined
  const rss = isRssAccount(account, column.accountId)
  return (
    <Avatar
      name={account ? account.display_name || account.email || account.id : accountLabel(column.accountId, accounts)}
      src={account?.avatar_url}
      size={size}
      className="shrink-0"
      fallback={rss ? <Rss size={size * 0.55} /> : undefined}
    />
  )
}
