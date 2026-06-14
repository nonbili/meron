import { useRef, useState } from 'react'
import { Inbox, Mail, MoreVertical, Plus, Star } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { FilterMode } from '../../states/ui'
import { IconButton } from '../button/IconButton'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { MenuItem } from '../menu/MenuItem'
import { menuItemBase } from '../menu/menuStyles'

// Inline board-wide filter, shown in the header only when there's room. On narrow
// widths it's hidden (@min-[640px]) and the same options live inside BoardMenu.
export function FilterSwitch({ value, onChange }: { value: FilterMode; onChange: (mode: FilterMode) => void }) {
  const { t } = useTranslation()
  const options: { mode: FilterMode; label: string; icon: React.ReactNode }[] = [
    { mode: 'all', label: t('filters.all'), icon: <Inbox size={13} /> },
    { mode: 'unread', label: t('filters.unread'), icon: <Mail size={13} /> },
    { mode: 'starred', label: t('filters.starred'), icon: <Star size={13} /> },
  ]
  return (
    <div className="hidden @min-[640px]:flex h-9 shrink-0 items-center gap-0.5 rounded-xl bg-active/70 p-[3px]">
      {options.map(({ mode, label, icon }) => (
        <button
          key={mode}
          className={`flex h-7 items-center gap-1.5 rounded-lg px-3 text-xs font-semibold cursor-pointer transition-all duration-200 ${
            value === mode ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:bg-hover hover:text-primary'
          }`}
          onClick={() => onChange(mode)}
          title={t('kanban.actions.showFilterInAllColumns', { filter: label.toLowerCase() })}
        >
          {icon}
          <span>{label}</span>
        </button>
      ))}
    </div>
  )
}

// Board overflow menu in the kanban header: the Add Column action, plus the
// board-wide filter options — but the filter section only renders on narrow
// widths (@min-[640px]:hidden), where the inline FilterSwitch is hidden.
export function BoardMenu({
  filterMode,
  onFilterChange,
  onAddColumn,
}: {
  filterMode: FilterMode
  onFilterChange: (mode: FilterMode) => void
  onAddColumn: () => void
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
    <div ref={rootRef} className="relative shrink-0">
      <IconButton
        icon={MoreVertical}
        label={t('kanban.actions.boardOptions')}
        variant={filterActive ? 'accentSoft' : 'ghost'}
        active={!filterActive && open}
        onClick={(event) => {
          event.stopPropagation()
          setOpen((value) => !value)
        }}
      />
      {open && (
        <div
          className="absolute right-0 mt-1.5 z-50 min-w-[180px] w-max rounded-xl border border-border bg-white dark:bg-[#1e293b] p-1 shadow-2xl animate-fade-in select-none"
          onClick={(event) => event.stopPropagation()}
        >
          <div className="@min-[640px]:hidden">
            <div className="px-3 pb-1 pt-1.5 text-[10px] font-bold uppercase tracking-wider text-secondary">
              {t('filters.label')}
            </div>
            {filterItem('all', t('filters.all'), <Inbox size={13} className="text-secondary shrink-0" />)}
            {filterItem('unread', t('filters.unread'), <Mail size={13} className="text-secondary shrink-0" />)}
            {filterItem('starred', t('filters.starred'), <Star size={13} className="text-secondary shrink-0" />)}
            <div className="my-1 border-t border-border" />
          </div>
          <MenuItem
            className="flex-nowrap"
            icon={<Plus size={13} className="text-secondary shrink-0" />}
            label={<span className="whitespace-nowrap shrink-0">{t('kanban.actions.addColumn')}</span>}
            onClick={() => {
              onAddColumn()
              setOpen(false)
            }}
          />
        </div>
      )}
    </div>
  )
}
