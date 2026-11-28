import { useValue } from '@legendapp/state/react'
import { CalendarDays, Check, ChevronLeft, ChevronRight, Minimize2, Plus, Repeat, Settings2, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState, type DragEvent } from 'react'
import {
  droppedEventStart,
  firstDayOfWeek,
  monthGrid,
  occursOn,
  sameDay,
  sameMonth,
  weekGrid,
} from '../../lib/calendarGrid'
import { formatEventRange, formatRange } from '../../lib/date'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { settings$ } from '../../states/settings'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { withAlpha } from '../../lib/color'
import {
  calendar$,
  calendarColor,
  calendarOf,
  canRescheduleEvent,
  rescheduleEvent,
  visibleItems,
  closeCalendar,
  goToToday,
  openCalendarPanel,
  loadCalendar,
  loadSyncedCalendars,
  openCalendarItem,
  syncCalendarsInBackground,
  selectDay,
  shiftMonth,
  shiftWeek,
  type CalendarItem,
} from '../../states/calendar'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { SelectInput } from '../field/Field'
import { EventEditorDialog } from './EventEditorDialog'
import { CalendarFilter, CalendarList } from './CalendarFilter'
import { confirmAction, openSettingsAt } from '../../states/ui'
import { CalendarFileMenu } from './CalendarFileMenu'
import { CalendarSyncButton } from './CalendarSyncButton'
import { MiniCalendar } from './MiniCalendar'

/** How many items a day cell lists before folding the rest into "+N more". */
const VISIBLE_PER_DAY = 3

// The full calendar shows events, accepted invitations and due tasks.
export function CalendarView() {
  const { t } = useTranslation()
  const month = new Date(useValue(calendar$.month))
  const selected = new Date(useValue(calendar$.day))
  const items = useVisibleItems()
  const editor = useValue(calendar$.editor)
  const [view, setView] = useState<'month' | 'week'>('month')
  const drag = useRef<{ item: CalendarItem; day: Date } | null>(null)
  const [dropTarget, setDropTarget] = useState<string | null>(null)
  const pending = useRef(new Set<string>())
  const week = view === 'week'
  const [expandedDay, setExpandedDay] = useState<number | null>(null)

  const weekStart = useMemo(() => firstDayOfWeek(), [])
  const days = useMemo(() => monthGrid(month, weekStart), [month.getFullYear(), month.getMonth(), weekStart])
  const today = new Date()
  // System locale, like every other date in the app (see lib/date.ts).
  const weekDays = weekGrid(selected, weekStart)
  const title = new Intl.DateTimeFormat([], { month: 'long', year: 'numeric', calendar: 'gregory' }).format(month)
  const weekTitle = formatRange(
    new Intl.DateTimeFormat([], { month: 'short', day: 'numeric', year: 'numeric', calendar: 'gregory' }),
    weekDays[0],
    weekDays[6],
  )
  const fullDate = new Intl.DateTimeFormat([], { dateStyle: 'full', calendar: 'gregory' })
  const weekdays = days.slice(0, 7).map((day) => new Intl.DateTimeFormat([], { weekday: 'short' }).format(day))

  useCalendarFeed()

  useEscapeKey(() => setExpandedDay(null), expandedDay !== null && !editor)

  const jumpToToday = () => {
    setExpandedDay(null)
    goToToday()
  }

  const endDrag = () => {
    drag.current = null
    setDropTarget(null)
  }

  const startDrag = (event: DragEvent<HTMLButtonElement>, item: CalendarItem, day: Date) => {
    if (pending.current.has(item.id)) {
      event.preventDefault()
      return
    }
    drag.current = { item, day }
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('application/x-meron-calendar-event', item.key)
  }

  const dropProps = (day: Date, hour?: number) => {
    const key = `${day.getTime()}:${hour ?? ''}`
    const accepts = () => drag.current && (!week || drag.current.item.all_day === (hour === undefined))
    return {
      onDragOver: (event: DragEvent) => {
        if (!accepts()) return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'move'
        setDropTarget(key)
      },
      onDragLeave: (event: DragEvent) => {
        if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return
        setDropTarget((current) => (current === key ? null : current))
      },
      onDrop: async (event: DragEvent) => {
        if (!accepts() || !drag.current) return
        event.preventDefault()
        const { item, day: from } = drag.current
        endDrag()
        const start = droppedEventStart(item, from, day, hour)
        if (start === item.start || pending.current.has(item.id)) return
        pending.current.add(item.id)
        try {
          if (
            item.recurring &&
            !(await confirmAction({
              title: t('calendar.moveSeries', { defaultValue: 'Move recurring series' }),
              message: t('calendar.moveSeriesConfirm', {
                defaultValue: 'Move every occurrence in this recurring series?',
              }),
              confirmLabel: t('buttons.move', { defaultValue: 'Move' }),
            }))
          )
            return
          await rescheduleEvent(item, start)
        } finally {
          pending.current.delete(item.id)
        }
      },
    }
  }

  const renderChip = (item: CalendarItem, day: Date) => (
    <CalendarChip key={item.key} item={item} onDragStart={(event) => startDrag(event, item, day)} onDragEnd={endDrag} />
  )
  const changeView = (next: 'month' | 'week') => {
    setExpandedDay(null)
    endDrag()
    setView(next)
  }

  return (
    <section className="flex min-w-0 flex-1 flex-col bg-chats [--calendar-sidebar-width:calc(var(--spacing)*56)]">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-border/50 bg-header/70 px-4 min-[1100px]:ps-0">
        <div className="flex min-w-0 items-center gap-2 min-[1100px]:gap-0">
          <div className="flex min-w-0 items-center gap-2 min-[1100px]:w-(--calendar-sidebar-width) min-[1100px]:shrink-0 min-[1100px]:px-4">
            <CalendarDays size={17} className="shrink-0 text-accent" />
            <h1 className="min-w-0 truncate text-sm font-semibold text-primary">{week ? weekTitle : title}</h1>
          </div>
          <Button variant="secondary" size="sm" onClick={jumpToToday}>
            {t('calendar.today')}
          </Button>
        </div>
        <div className="flex-1" />
        <div className={clsx('flex shrink-0 items-center gap-1', !week && 'min-[1100px]:hidden')}>
          <IconButton
            label={week ? t('calendar.previousWeek', { defaultValue: 'Previous week' }) : t('calendar.previousMonth')}
            icon={ChevronLeft}
            size="sm"
            onClick={() => (week ? shiftWeek(-1) : shiftMonth(-1))}
          />
          <IconButton
            label={week ? t('calendar.nextWeek', { defaultValue: 'Next week' }) : t('calendar.nextMonth')}
            icon={ChevronRight}
            size="sm"
            onClick={() => (week ? shiftWeek(1) : shiftMonth(1))}
          />
        </div>
        <SelectInput
          aria-label={t('calendar.view', { defaultValue: 'Calendar view' })}
          wrapperClassName="shrink-0"
          value={view}
          onChange={(event) => changeView(event.target.value === 'week' ? 'week' : 'month')}
        >
          <option value="month">{t('calendar.month')}</option>
          <option value="week">{t('calendar.week', { defaultValue: 'Week' })}</option>
        </SelectInput>
        <CalendarSyncButton />
        <CalendarFileMenu />
        {/* The pane beside the month lists the calendars on a wide window. */}
        <CalendarFilter className="min-[1100px]:hidden" />
        <Button
          size="sm"
          leftIcon={Plus}
          onClick={() => calendar$.editor.set({ mode: 'new', day: selected.getTime() })}
        >
          {t('calendar.newEvent')}
        </Button>
        {/* Back to the panel it was expanded from; there is no panel on a
            narrow window, where the button would only close the calendar. */}
        <IconButton
          label={t('calendar.openInSidePanel')}
          icon={Minimize2}
          size="sm"
          className="max-[900px]:hidden"
          onClick={openCalendarPanel}
        />
        <IconButton label={t('buttons.close')} icon={X} size="sm" onClick={closeCalendar} />
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="hidden w-(--calendar-sidebar-width) shrink-0 flex-col gap-3 overflow-y-auto border-r border-border/50 p-3 min-[1100px]:flex">
          <MiniCalendar
            month={month}
            days={days}
            selected={selected}
            items={items}
            onNavigate={() => setExpandedDay(null)}
          />
          <div className="flex flex-col gap-1 border-t border-border/50 pt-3">
            <h2 className="px-2 text-[0.6875rem] font-semibold text-secondary">{t('calendar.calendars')}</h2>
            <CalendarList />
            <button
              type="button"
              className="mt-1 flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-left text-xs text-secondary cursor-pointer hover:bg-hover hover:text-primary"
              onClick={() => openSettingsAt('calendars', '')}
            >
              <Settings2 size={13} className="shrink-0" />
              {t('calendar.manageCalendars')}
            </button>
          </div>
        </aside>
        {week ? (
          <CalendarWeekGrid
            days={weekDays}
            today={today}
            items={items}
            renderChip={renderChip}
            dropProps={dropProps}
            dropTarget={dropTarget}
          />
        ) : (
          <div className="flex min-w-0 flex-1 flex-col">
            <div className="grid shrink-0 grid-cols-7 border-b border-border/50 text-[0.6875rem] font-semibold text-secondary">
              {weekdays.map((weekday, index) => (
                <div key={index} className="px-2 py-1.5 text-right">
                  {weekday}
                </div>
              ))}
            </div>

            <div className="grid min-h-0 flex-1 grid-cols-7 grid-rows-6">
              {days.map((day) => {
                const dayItems = items.filter((item) => occursOn(item, day))
                const expanded = expandedDay === day.getTime()
                const shown = expanded ? dayItems : dayItems.slice(0, VISIBLE_PER_DAY)
                const hidden = dayItems.length - shown.length
                const isToday = sameDay(day, today)
                const isSelected = sameDay(day, selected)
                return (
                  <div
                    key={day.getTime()}
                    {...dropProps(day)}
                    title={fullDate.format(day)}
                    className={clsx(
                      'relative flex min-h-0 min-w-0 flex-col gap-0.5 border-b border-r border-border/40 p-1 cursor-pointer hover:bg-hover/40',
                      !sameMonth(day, month) && 'bg-black/[0.02] dark:bg-white/[0.02]',
                      dropTarget === `${day.getTime()}:` && 'ring-2 ring-inset ring-accent',
                    )}
                    onClick={() => {
                      setExpandedDay(null)
                      selectDay(day)
                      calendar$.editor.set({ mode: 'new', day: day.getTime() })
                    }}
                  >
                    <span
                      className={clsx(
                        'ml-auto flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[0.6875rem] font-semibold',
                        isToday
                          ? 'bg-accent text-accent-label'
                          : sameMonth(day, month)
                            ? 'text-primary'
                            : 'text-secondary/60',
                        isSelected && !isToday && 'min-[1100px]:bg-accent/15 min-[1100px]:text-accent',
                      )}
                      aria-current={isToday ? 'date' : undefined}
                    >
                      {day.getDate()}
                    </span>
                    <div
                      className={clsx(
                        'flex min-h-0 flex-col gap-0.5',
                        expanded &&
                          'absolute inset-x-0 top-0 z-10 max-h-72 overflow-y-auto rounded-lg border border-border bg-chats p-1 pt-7 shadow-xl',
                      )}
                    >
                      {shown.map((item) => renderChip(item, day))}
                      {hidden > 0 && (
                        <button
                          type="button"
                          className="truncate rounded px-1 text-left text-[0.65625rem] font-semibold text-secondary hover:bg-hover hover:text-primary cursor-pointer"
                          onClick={(event) => {
                            event.stopPropagation()
                            setExpandedDay(day.getTime())
                          }}
                        >
                          {t('common.moreCount', { count: hidden })}
                        </button>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </div>

      {editor && <EventEditorDialog editor={editor} />}
    </section>
  )
}

const WEEK_COLUMNS = 'grid grid-cols-[3.5rem_repeat(7,minmax(0,1fr))]'
const HOURS = Array.from({ length: 24 }, (_, hour) => hour)

function CalendarWeekGrid({
  days,
  today,
  items,
  renderChip,
  dropProps,
  dropTarget,
}: {
  days: Date[]
  today: Date
  items: CalendarItem[]
  renderChip: (item: CalendarItem, day: Date) => React.ReactNode
  dropProps: (day: Date, hour?: number) => React.HTMLAttributes<HTMLDivElement>
  dropTarget: string | null
}) {
  const { t } = useTranslation()
  const heading = useRef<HTMLDivElement>(null)
  const morning = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const row = morning.current
    if (row?.parentElement) row.parentElement.scrollTop = row.offsetTop - (heading.current?.offsetHeight ?? 0)
  }, [])

  const columns = days.map((day) => ({ day, items: items.filter((item) => occursOn(item, day)) }))
  const dateLabel = new Intl.DateTimeFormat([], { weekday: 'short', day: 'numeric' })
  const timeLabel = new Intl.DateTimeFormat([], { hour: '2-digit', minute: '2-digit', hour12: false })
  const cellClass = (day: Date, hour?: number) =>
    clsx(
      'flex min-w-0 flex-col gap-0.5 border-b border-r border-border/40 p-1',
      dropTarget === `${day.getTime()}:${hour ?? ''}` && 'ring-2 ring-inset ring-accent',
    )

  return (
    <div className="relative min-h-0 min-w-0 flex-1 overflow-auto">
      <div ref={heading} className="sticky top-0 z-10 bg-chats">
        <div className={WEEK_COLUMNS}>
          <div className="border-b border-border/40" />
          {days.map((day) => (
            <div
              key={day.getTime()}
              className={clsx(
                'border-b border-r border-border/40 px-1 py-1.5 text-center text-xs font-semibold',
                sameDay(day, today) && 'text-accent',
              )}
              aria-current={sameDay(day, today) ? 'date' : undefined}
            >
              {dateLabel.format(day)}
            </div>
          ))}
        </div>
        <div className={WEEK_COLUMNS}>
          <div className="border-b border-border/40 px-1 py-1.5 text-center text-[0.6875rem] text-secondary">
            {t('invite.allDay')}
          </div>
          {columns.map(({ day, items }) => (
            <div key={day.getTime()} className={clsx(cellClass(day), 'max-h-24 overflow-y-auto')} {...dropProps(day)}>
              {items.filter((item) => item.all_day).map((item) => renderChip(item, day))}
            </div>
          ))}
        </div>
      </div>
      {HOURS.map((hour) => (
        <div key={hour} ref={hour === 8 ? morning : undefined} className={clsx(WEEK_COLUMNS, 'min-h-9')}>
          <div className="border-b border-border/40 px-1 py-1 text-center text-[0.6875rem] tabular-nums text-secondary">
            {timeLabel.format(new Date(2000, 0, 1, hour))}
          </div>
          {columns.map(({ day, items }) => (
            <div
              key={day.getTime()}
              className={cellClass(day, hour)}
              title={`${dateLabel.format(day)} ${timeLabel.format(new Date(2000, 0, 1, hour))}`}
              {...dropProps(day, hour)}
            >
              {items
                .filter((item) => {
                  if (item.all_day) return false
                  const start = new Date(item.start * 1000)
                  return hour === (sameDay(start, day) ? start.getHours() : 0)
                })
                .map((item) => renderChip(item, day))}
            </div>
          ))}
        </div>
      ))}
    </div>
  )
}

/** Keep the loaded items current while a calendar is on screen. */
export function useCalendarFeed() {
  const tasksEnabled = useValue(settings$.tasksEnabled)

  useEffect(() => {
    void loadCalendar()
    // Opening the calendar is a good moment to catch up with its servers;
    // what comes in arrives as calendar.changed.
    syncCalendarsInBackground()
    // Another surface changed the calendar (an invitation answered, the other
    // window, a sync): re-read rather than replay the change.
    const eventsOn = (window as any).runtime?.EventsOn
    // Tasks too: a due task done, moved or deleted in the panel beside it.
    const offs =
      typeof eventsOn === 'function'
        ? [
            ...['calendar.changed', 'tasks.changed'].map((name) => eventsOn(name, () => void loadCalendar())),
            // An account added or a calendar renamed or recoloured.
            eventsOn('caldav.changed', () => void loadSyncedCalendars()),
          ]
        : []
    return () => {
      for (const off of offs) if (typeof off === 'function') off()
    }
  }, [])

  // Due tasks come and go with the Tasks setting; the first load is above.
  const tasksShown = useRef(tasksEnabled)
  useEffect(() => {
    if (tasksShown.current === tasksEnabled) return
    tasksShown.current = tasksEnabled
    void loadCalendar()
  }, [tasksEnabled])
}

/** The colour a synced calendar's events are drawn in; none (the accent)
 * for this device's. */
export function useCalendarColor(item: CalendarItem): string | undefined {
  const calendars = useValue(calendar$.calendars)
  const calendar = calendarOf(item, calendars)
  return calendar ? calendarColor(calendar) : undefined
}

/** The loaded items of the calendars shown. */
export function useVisibleItems(): CalendarItem[] {
  const items = useValue(calendar$.items)
  const hidden = useValue(calendar$.hiddenCalendars)
  return useMemo(() => visibleItems(items, hidden), [items, hidden])
}

function CalendarChip({
  item,
  onDragStart,
  onDragEnd,
}: {
  item: CalendarItem
  onDragStart: (event: DragEvent<HTMLButtonElement>) => void
  onDragEnd: () => void
}) {
  const { t } = useTranslation()
  const color = useCalendarColor(item)
  const time = item.all_day
    ? ''
    : new Date(item.start * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })
  const task = item.source === 'task'
  const calendars = useValue(calendar$.calendars)
  const draggable = canRescheduleEvent(item, calendars)
  const title = item.title || t('invite.untitled')
  return (
    <button
      type="button"
      draggable={draggable}
      onDragStart={draggable ? onDragStart : undefined}
      onDragEnd={onDragEnd}
      title={task ? title : `${title} · ${formatEventRange(item.start, item.end, item.all_day)}`}
      onClick={(event) => {
        event.stopPropagation()
        openCalendarItem(item)
      }}
      className={clsx(
        'flex min-w-0 items-center gap-1 rounded px-1 py-px text-left text-[0.6875rem] leading-4 cursor-pointer',
        task
          ? 'text-emerald-700 hover:bg-emerald-500/15 dark:text-emerald-400'
          : item.all_day
            ? 'bg-accent/15 font-medium text-primary hover:bg-accent/25'
            : 'text-primary hover:bg-hover',
        draggable && 'active:cursor-grabbing',
        item.response === 'tentative' && 'border border-dashed border-accent/60 bg-transparent',
      )}
      style={color && item.all_day && !task ? { backgroundColor: withAlpha(color, 0.18) } : undefined}
    >
      {task ? (
        <Check size={11} className="shrink-0" />
      ) : (
        !item.all_day && (
          <span
            className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent"
            style={color ? { backgroundColor: color } : undefined}
          />
        )
      )}
      {time && <span className="shrink-0 text-secondary tabular-nums">{time}</span>}
      <span className="truncate">{title}</span>
      {item.recurring && <Repeat size={10} className="shrink-0 text-secondary" />}
    </button>
  )
}
