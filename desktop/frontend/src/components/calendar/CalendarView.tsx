import { useValue } from '@legendapp/state/react'
import { CalendarDays, Check, ChevronLeft, ChevronRight, Minimize2, Plus, Repeat, Settings2, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { firstDayOfWeek, monthGrid, occursOn, sameDay } from '../../lib/calendarGrid'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { settings$ } from '../../states/settings'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { withAlpha } from '../../lib/color'
import {
  calendar$,
  calendarColor,
  calendarOf,
  visibleItems,
  closeCalendar,
  goToToday,
  openCalendarPanel,
  loadCalendar,
  loadSyncedCalendars,
  openCalendarItem,
  syncCalendarsInBackground,
  shiftMonth,
  type CalendarItem,
} from '../../states/calendar'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { EventEditorDialog } from './EventEditorDialog'
import { CalendarFilter, CalendarList } from './CalendarFilter'
import { openSettingsAt } from '../../states/ui'
import { CalendarSyncButton } from './CalendarSyncButton'

/** How many items a day cell lists before folding the rest into "+N more". */
const VISIBLE_PER_DAY = 3

// The optional calendar: a month of events, accepted invitations and (with
// Tasks on) the open tasks due, in place of mail. Mirrors mobile's
// CalendarScreen.
export function CalendarView() {
  const { t } = useTranslation()
  const month = new Date(useValue(calendar$.month))
  const items = useVisibleItems()
  const editor = useValue(calendar$.editor)
  // With nothing synced there is only this device's calendar to list.
  const showPane = useValue(calendar$.calendars).length > 0
  const [expandedDay, setExpandedDay] = useState<number | null>(null)

  const weekStart = useMemo(() => firstDayOfWeek(), [])
  const days = useMemo(() => monthGrid(month, weekStart), [month.getFullYear(), month.getMonth(), weekStart])
  const today = new Date()
  // System locale, like every other date in the app (see lib/date.ts).
  const title = new Intl.DateTimeFormat([], { month: 'long', year: 'numeric' }).format(month)
  const weekdays = days.slice(0, 7).map((day) => new Intl.DateTimeFormat([], { weekday: 'short' }).format(day))

  useCalendarFeed()

  useEscapeKey(() => setExpandedDay(null), expandedDay !== null && !editor)

  return (
    <section className="flex min-w-0 flex-1 flex-col bg-chats">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-border/50 bg-header/70 px-4">
        <CalendarDays size={17} className="shrink-0 text-accent" />
        <h1 className="min-w-0 flex-1 truncate text-sm font-semibold text-primary">{title}</h1>
        <Button variant="secondary" size="sm" onClick={goToToday}>
          {t('calendar.today')}
        </Button>
        <IconButton label={t('calendar.previousMonth')} icon={ChevronLeft} size="sm" onClick={() => shiftMonth(-1)} />
        <IconButton label={t('calendar.nextMonth')} icon={ChevronRight} size="sm" onClick={() => shiftMonth(1)} />
        <CalendarSyncButton />
        {/* The pane beside the month lists the calendars on a wide window. */}
        <CalendarFilter className={clsx(showPane && 'min-[1100px]:hidden')} />
        <Button
          size="sm"
          leftIcon={Plus}
          onClick={() => {
            const day = sameMonth(today, month) ? today : new Date(month.getFullYear(), month.getMonth(), 1)
            calendar$.editor.set({ mode: 'new', day: day.getTime() })
          }}
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
        {showPane && (
          <aside className="flex w-56 shrink-0 flex-col gap-3 overflow-y-auto border-r border-border/50 p-3 max-[1100px]:hidden">
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
          </aside>
        )}
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
              return (
                <div
                  key={day.getTime()}
                  className={clsx(
                    'relative flex min-h-0 min-w-0 flex-col gap-0.5 border-b border-r border-border/40 p-1 cursor-pointer hover:bg-hover/40',
                    !sameMonth(day, month) && 'bg-black/[0.02] dark:bg-white/[0.02]',
                  )}
                  onClick={() => calendar$.editor.set({ mode: 'new', day: day.getTime() })}
                >
                  <span
                    className={clsx(
                      'ml-auto flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[0.6875rem] font-semibold',
                      isToday
                        ? 'bg-accent text-accent-label'
                        : sameMonth(day, month)
                          ? 'text-primary'
                          : 'text-secondary/60',
                    )}
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
                    {shown.map((item) => (
                      <CalendarChip key={item.key} item={item} />
                    ))}
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
      </div>

      {editor && <EventEditorDialog editor={editor} />}
    </section>
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

export function sameMonth(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth()
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

function CalendarChip({ item }: { item: CalendarItem }) {
  const { t } = useTranslation()
  const color = useCalendarColor(item)
  const time = item.all_day
    ? ''
    : new Date(item.start * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })
  const task = item.source === 'task'
  return (
    <button
      type="button"
      title={item.title || t('invite.untitled')}
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
      <span className="truncate">{item.title || t('invite.untitled')}</span>
      {item.recurring && <Repeat size={10} className="shrink-0 text-secondary" />}
    </button>
  )
}
