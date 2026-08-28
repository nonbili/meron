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
}: {
  value: string
  onChange: (value: string) => void
  visibleColumns: KanbanColumn[]
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

  const selectedLabel = useMemo(() => {
    if (value === 'all') return t('kanban.searchScope.allColumns')
    const column = visibleColumns.find((c) => kanbanColumnKey(c) === value)
    if (!column) return t('kanban.searchScope.allColumns')
    return folderLabel(column, folders, accounts, t)
  }, [value, visibleColumns, folders, accounts, t])

  return (
    <div ref={containerRef} className="relative h-full shrink-0 border-l border-border/60">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex h-full items-center gap-1.5 px-3.5 text-[0.6875rem] font-semibold text-secondary hover:text-primary transition-colors cursor-pointer select-none rounded-r-xl outline-none border-0"
        title={t('kanban.searchScope.label')}
      >
        <span className="truncate max-w-[130px]">{selectedLabel}</span>
        <ChevronDown
          size={12}
          className={`text-secondary transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>
      {isOpen && (
        <div className="absolute right-0 mt-1.5 z-50 w-64 max-h-72 overflow-y-auto rounded-2xl border border-border bg-chats p-1.5 shadow-xl shadow-black/10 dark:shadow-black/35 animate-slide-up">
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
            const columnIsRss = isRssAccount(columnAccount, column.accountId)

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
                <Avatar
                  name={columnAccountLabel || accountLabel(column.accountId, accounts)}
                  email={columnIsRss ? undefined : columnAccount?.email}
                  src={columnAccount?.avatar_url}
                  size={18}
                  fallback={columnIsRss ? <Rss size={10} /> : undefined}
                />
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
