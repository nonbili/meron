import { useValue } from '@legendapp/state/react'
import { CalendarDays, Check, Maximize2, Plus, Repeat, X } from 'lucide-react'
import { useMemo } from 'react'
import { firstDayOfWeek, monthGrid, occursOn, sameDay } from '../../lib/calendarGrid'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import {
  AGENDA_DAYS,
  calendar$,
  closeCalendarPanel,
  openCalendar,
  openCalendarItem,
  type CalendarItem,
} from '../../states/calendar'
import { IconButton } from '../button/IconButton'
import { SIDE_PANEL_WIDTH } from '../sidepanel/SidePanelSlide'
import { useCalendarColor, useCalendarFeed, useVisibleItems } from './CalendarView'
import { CalendarFilter } from './CalendarFilter'
import { CalendarSyncButton } from './CalendarSyncButton'
import { EventEditorDialog } from './EventEditorDialog'
import { MiniCalendar } from './MiniCalendar'

/**
 * The calendar as a right-hand panel, in the slot Tasks also uses: a small
 * month to pick a day, and the week from that day on. It is the calendar's
 * everyday shape — "am I free Thursday?" is asked while reading mail, not
 * instead of it. The full month grid is a button away for planning.
 */
export function CalendarPanel() {
  const { t } = useTranslation()
  const month = new Date(useValue(calendar$.month))
  const selected = new Date(useValue(calendar$.day))
  const items = useVisibleItems()
  const editor = useValue(calendar$.editor)

  const weekStart = useMemo(() => firstDayOfWeek(), [])
  const days = useMemo(() => monthGrid(month, weekStart), [month.getFullYear(), month.getMonth(), weekStart])
  const today = new Date()
  const agenda = Array.from(
    { length: AGENDA_DAYS },
    (_, index) => new Date(selected.getFullYear(), selected.getMonth(), selected.getDate() + index),
  )
    .map((day) => ({ day, items: items.filter((item) => occursOn(item, day)) }))
    // The day picked always shows, if only to say it is free; later days only
    // when something is on.
    .filter((entry, index) => index === 0 || entry.items.length > 0)

  useCalendarFeed()

  return (
    <div
      // Hidden on a narrow window, like Tasks; the calendar button opens the
      // full view there instead.
      className="flex min-h-0 shrink-0 flex-col border-l border-border/60 bg-chats max-[900px]:hidden"
      style={{ width: SIDE_PANEL_WIDTH }}
    >
      <header className="flex h-12 shrink-0 items-center gap-1 border-b border-border pl-3 pr-1.5">
        <CalendarDays size={16} className="shrink-0 text-accent" />
        <h2 className="min-w-0 flex-1 truncate px-1.5 text-sm font-semibold text-primary">{t('calendar.title')}</h2>
        <CalendarSyncButton />
        <CalendarFilter />
        <IconButton
          label={t('calendar.newEvent')}
          icon={Plus}
          size="sm"
          onClick={() => calendar$.editor.set({ mode: 'new', day: selected.getTime() })}
        />
        <IconButton label={t('calendar.openFullCalendar')} icon={Maximize2} size="sm" onClick={openCalendar} />
        <IconButton label={t('buttons.close')} icon={X} size="sm" onClick={closeCalendarPanel} />
      </header>

      <section className="shrink-0 border-b border-border px-3 pb-2 pt-2">
        <MiniCalendar month={month} days={days} selected={selected} items={items} showToday />
      </section>

      <div className="min-h-0 flex-1 overflow-y-auto pb-2">
        {agenda.map(({ day, items: dayItems }) => (
          <section key={day.getTime()}>
            <h3
              className={clsx(
                'sticky top-0 bg-chats px-4 pb-1 pt-3 text-[0.6875rem] font-semibold',
                sameDay(day, today) ? 'text-accent' : 'text-secondary',
              )}
            >
              {sameDay(day, today) && `${t('calendar.today')} · `}
              {new Intl.DateTimeFormat([], { weekday: 'long', month: 'short', day: 'numeric' }).format(day)}
            </h3>
            {dayItems.length === 0 ? (
              <p className="px-4 py-1 text-xs text-secondary">{t('calendar.nothingScheduled')}</p>
            ) : (
              dayItems.map((item) => <AgendaRow key={item.key} item={item} day={day} />)
            )}
          </section>
        ))}
      </div>

      {editor && <EventEditorDialog editor={editor} />}
    </div>
  )
}

const CLOCK: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit', hour12: false }

/** An end of a timed item as listed under `day`: its time on that day, with
 * its date too when it falls on another, so an event running over several days
 * reads as one span wherever it is listed. */
function edge(at: Date, day: Date): string {
  if (sameDay(at, day)) return at.toLocaleTimeString([], CLOCK)
  return at.toLocaleString([], { month: 'short', day: 'numeric', ...CLOCK })
}

/** When a timed item happens, as listed under `day`. */
export function timeSpan(item: CalendarItem, day: Date): string {
  const start = new Date(item.start * 1000)
  if (item.end <= item.start) return edge(start, day)
  // An end at midnight closes the day before: 22:00 – 00:00 is one evening.
  const end = new Date(item.end * 1000)
  const endDay = new Date(item.end * 1000 - 1)
  return `${edge(start, day)} – ${sameDay(endDay, day) ? end.toLocaleTimeString([], CLOCK) : edge(end, day)}`
}

function AgendaRow({ item, day }: { item: CalendarItem; day: Date }) {
  const { t } = useTranslation()
  const task = item.source === 'task'
  const color = useCalendarColor(item)
  const title = item.title || t('invite.untitled')
  const when = item.all_day ? t('invite.allDay') : timeSpan(item, day)
  return (
    <button
      type="button"
      title={title}
      onClick={() => openCalendarItem(item)}
      className="flex w-full min-w-0 items-start gap-2.5 px-4 py-1.5 text-left cursor-pointer hover:bg-hover"
    >
      <span
        className={clsx(
          'mt-1 flex h-3 w-1 shrink-0 rounded-full',
          task ? 'bg-emerald-500' : item.response === 'tentative' ? 'border border-dashed border-accent' : 'bg-accent',
        )}
        style={color && !task && item.response !== 'tentative' ? { backgroundColor: color } : undefined}
      />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="flex min-w-0 items-center gap-1 text-[0.8125rem] text-primary">
          {task && <Check size={12} className="shrink-0 text-emerald-600 dark:text-emerald-400" />}
          <span className="truncate">{title}</span>
          {item.recurring && <Repeat size={11} className="shrink-0 text-secondary" />}
        </span>
        {/* A due task has no time worth showing beyond its day. */}
        {!task && (
          <span className="truncate text-[0.6875rem] text-secondary tabular-nums">
            {when}
            {item.location && ` · ${item.location}`}
          </span>
        )}
      </span>
    </button>
  )
}
