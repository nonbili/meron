import { useRef, useState } from 'react'
import { Check, Columns3, Inbox, Lock, Mail, Minus, MoreVertical, Plus, Settings, Star } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { type FilterMode, ui$ } from '../../states/ui'
import {
  clampKanbanColumnWidth,
  KANBAN_COLUMN_MAX_WIDTH,
  KANBAN_COLUMN_MIN_WIDTH,
  settings$,
} from '../../states/settings'
import { IconButton } from '../button/IconButton'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { MenuItem } from '../menu/MenuItem'
import { menuItemBase } from '../menu/menuStyles'

// Deliberately coarser than the settings dialog's 10px: this is a
// click-at-a-time stepper for eyeballing density against the board. Settings
// keeps the number field for an exact width.
const COLUMN_WIDTH_STEP = 20

// Built per-state rather than with disabled:/hover: variants: the enabled and
// disabled rules touch the same utilities, and variant order would decide the
// winner instead of this call site.
const stepperButton = (disabled: boolean) =>
  `flex h-6 w-6 items-center justify-center rounded-md transition-colors ${
    disabled ? 'cursor-default text-secondary/40' : 'cursor-pointer text-secondary hover:bg-hover hover:text-primary'
  }`

// menuItemBase minus its cursor-pointer: this row is a label with controls, not
// a clickable item.
const stepperRow =
  'flex h-8 w-full items-center gap-2 whitespace-nowrap rounded-lg px-2 text-left text-[0.8125rem] font-normal leading-normal text-primary'

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

// Board overflow menu in the kanban header: the Add Column action, the board
// layout controls (column width, drag scroll lock), shortcuts to the board's
// settings, plus the board-wide filter options — but the filter
// section only renders on narrow widths (@min-[640px]:hidden), where the inline
// FilterSwitch is hidden.
export function BoardMenu({
  boardId,
  filterMode,
  onFilterChange,
  onAddColumn,
}: {
  boardId: string
  filterMode: FilterMode
  onFilterChange: (mode: FilterMode) => void
  onAddColumn: () => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement | null>(null)
  const lockScroll = useValue(settings$.kanbanLockScroll)
  const columnWidth = useValue(settings$.kanbanColumnWidth)

  useDismissOnOutside(
    open,
    (target) => Boolean(rootRef.current?.contains(target as Node)),
    () => setOpen(false),
  )

  const filterActive = filterMode !== 'all'

  const stepColumnWidth = (delta: number) =>
    settings$.kanbanColumnWidth.set(clampKanbanColumnWidth(columnWidth + delta))

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
          className="absolute right-0 mt-1.5 z-50 min-w-[180px] w-max rounded-xl border border-border bg-chats p-1 shadow-2xl animate-fade-in select-none"
          onClick={(event) => event.stopPropagation()}
        >
          <div className="@min-[640px]:hidden">
            <div className="px-3 pb-1 pt-1.5 text-[0.625rem] font-bold uppercase tracking-wider text-secondary">
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
          <div className="my-1 border-t border-border" />
          {/* Board layout lives here as well as in Settings: column width and the
              drag-time scroll lock are both judged by looking at the board, so
              they belong within reach of it. */}
          <div className={stepperRow}>
            <Columns3 size={13} className="shrink-0 text-secondary" />
            <span className="min-w-0 flex-1 whitespace-nowrap text-left">{t('settings.kanban.columnWidth')}</span>
            <span className="flex shrink-0 items-center gap-0.5">
              <button
                className={stepperButton(columnWidth <= KANBAN_COLUMN_MIN_WIDTH)}
                disabled={columnWidth <= KANBAN_COLUMN_MIN_WIDTH}
                aria-label={t('kanban.actions.narrowColumns')}
                title={t('kanban.actions.narrowColumns')}
                onClick={() => stepColumnWidth(-COLUMN_WIDTH_STEP)}
              >
                <Minus size={12} />
              </button>
              <span className="w-12 text-center text-xs font-semibold tabular-nums text-secondary">
                {columnWidth}px
              </span>
              <button
                className={stepperButton(columnWidth >= KANBAN_COLUMN_MAX_WIDTH)}
                disabled={columnWidth >= KANBAN_COLUMN_MAX_WIDTH}
                aria-label={t('kanban.actions.widenColumns')}
                title={t('kanban.actions.widenColumns')}
                onClick={() => stepColumnWidth(COLUMN_WIDTH_STEP)}
              >
                <Plus size={12} />
              </button>
            </span>
          </div>
          <button
            className={`${menuItemBase} flex-nowrap ${
              lockScroll ? 'bg-accent/10 dark:bg-accent/15 text-accent' : 'text-primary hover:bg-hover'
            }`}
            onClick={() => settings$.kanbanLockScroll.set(!lockScroll)}
          >
            <Lock size={13} className={`shrink-0 ${lockScroll ? 'text-accent' : 'text-secondary'}`} />
            <span className="min-w-0 flex-1 whitespace-nowrap text-left">{t('settings.kanban.lockScroll')}</span>
            {lockScroll && <Check size={13} className="shrink-0 text-accent" />}
          </button>
          <MenuItem
            className="flex-nowrap"
            icon={<Settings size={13} className="text-secondary shrink-0" />}
            label={<span className="whitespace-nowrap shrink-0">{t('kanban.board.settings')}</span>}
            onClick={() => {
              ui$.accountSettingsId.set(boardId)
              ui$.settingsOpen.set(true)
              setOpen(false)
            }}
          />
        </div>
      )}
    </div>
  )
}
