import { ChevronDown, ChevronLeft, ChevronRight, Minus, Plus } from 'lucide-react'
import { useId, useRef, useState } from 'react'
import { occursOn, sameDay, sameMonth } from '../../lib/calendarGrid'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { goToToday, selectDay, shiftMonth, type CalendarItem } from '../../states/calendar'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { SelectInput, TextInput } from '../field/Field'
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'

const MONTH = new Intl.DateTimeFormat([], { month: 'long', year: 'numeric', calendar: 'gregory' })
const WEEKDAY = new Intl.DateTimeFormat([], { weekday: 'narrow' })
const DATE = new Intl.DateTimeFormat([], { dateStyle: 'full', calendar: 'gregory' })
const MONTH_NAME = new Intl.DateTimeFormat([], { month: 'long', calendar: 'gregory' })
const MONTH_NAMES = Array.from({ length: 12 }, (_, index) => MONTH_NAME.format(new Date(2020, index, 1)))

/** Shared date navigator for the agenda panel and the full calendar. */
export function MiniCalendar({
  month,
  days,
  selected,
  items,
  showToday = false,
  onNavigate,
}: {
  month: Date
  days: Date[]
  selected: Date
  items: CalendarItem[]
  showToday?: boolean
  onNavigate?: () => void
}) {
  const { t } = useTranslation()
  const today = new Date()
  const navigate = (action: () => void) => () => {
    onNavigate?.()
    action()
  }

  return (
    <div className="shrink-0">
      <div className="flex items-center gap-1">
        <MonthPicker
          month={month}
          onJump={(day) => {
            onNavigate?.()
            selectDay(day)
          }}
        />
        {showToday && (
          <Button variant="secondary" size="sm" onClick={navigate(goToToday)}>
            {t('calendar.today')}
          </Button>
        )}
        <IconButton
          label={t('calendar.previousMonth')}
          icon={ChevronLeft}
          size="sm"
          onClick={navigate(() => shiftMonth(-1))}
        />
        <IconButton
          label={t('calendar.nextMonth')}
          icon={ChevronRight}
          size="sm"
          onClick={navigate(() => shiftMonth(1))}
        />
      </div>
      <div aria-hidden className="mt-1 grid grid-cols-7 text-center text-[0.65625rem] font-semibold text-secondary">
        {days.slice(0, 7).map((day) => (
          <div key={day.getTime()} className="py-1">
            {WEEKDAY.format(day)}
          </div>
        ))}
      </div>
      <div className="grid grid-cols-7">
        {days.map((day) => {
          const busy = items.some((item) => occursOn(item, day))
          const isSelected = sameDay(day, selected)
          const isToday = sameDay(day, today)
          return (
            <button
              key={day.getTime()}
              type="button"
              onClick={navigate(() => selectDay(day))}
              aria-label={DATE.format(day)}
              aria-current={isToday ? 'date' : undefined}
              aria-pressed={isSelected}
              className="flex h-8 flex-col items-center justify-center gap-px rounded-lg cursor-pointer hover:bg-hover"
            >
              <span
                className={clsx(
                  'flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[0.6875rem] font-semibold tabular-nums',
                  isSelected
                    ? 'bg-accent text-accent-label'
                    : isToday
                      ? 'bg-accent/15 text-accent'
                      : sameMonth(day, month)
                        ? 'text-primary'
                        : 'text-secondary/60',
                )}
              >
                {day.getDate()}
              </span>
              <span className={clsx('h-1 w-1 rounded-full', busy ? 'bg-accent/70' : 'bg-transparent')} />
            </button>
          )
        })}
      </div>
    </div>
  )
}

function MonthPicker({ month, onJump }: { month: Date; onJump: (day: Date) => void }) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState<{ month: number; year: string } | null>(null)
  const container = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const id = useId()
  const label = t('calendar.jumpToMonth')
  const year = Number(draft?.year)
  const validYear = !!draft && /^\d{1,4}$/.test(draft.year) && year >= 100
  const stepYear = (delta: number) => {
    if (!draft) return
    const next = (validYear ? year : month.getFullYear()) + delta
    if (next >= 100 && next <= 9999) setDraft({ ...draft, year: String(next) })
  }

  useDismissOnOutside(
    draft !== null,
    (target) => target instanceof Node && !!container.current?.contains(target),
    () => {
      if (container.current?.contains(document.activeElement)) trigger.current?.focus()
      setDraft(null)
    },
  )

  return (
    <div
      ref={container}
      className="relative min-w-0 flex-1"
      onBlur={(event) => {
        if (draft && !event.currentTarget.contains(event.relatedTarget)) setDraft(null)
      }}
    >
      <button
        ref={trigger}
        type="button"
        title={label}
        aria-haspopup="dialog"
        aria-expanded={draft !== null}
        aria-controls={draft ? id : undefined}
        className="flex h-8 w-full cursor-pointer items-center gap-1 rounded-lg px-1 text-start text-[0.8125rem] font-semibold text-primary hover:bg-hover"
        onClick={() => setDraft(draft ? null : { month: month.getMonth(), year: String(month.getFullYear()) })}
      >
        <span className="truncate">{MONTH.format(month)}</span>
        <ChevronDown size={13} className="shrink-0 text-secondary" aria-hidden />
      </button>
      {draft && (
        <MenuSurface id={id} role="dialog" aria-label={label} className="absolute start-0 top-full z-20 mt-1 w-48 p-2">
          <form
            className="flex flex-col gap-2"
            onSubmit={(event) => {
              event.preventDefault()
              if (!validYear) return
              onJump(new Date(year, draft.month, 1))
              setDraft(null)
              trigger.current?.focus()
            }}
          >
            <div className="flex flex-col gap-1">
              <label htmlFor={`${id}-month`} className="text-xs text-secondary">
                {t('calendar.month')}
              </label>
              <SelectInput
                id={`${id}-month`}
                value={draft.month}
                onChange={(event) => setDraft({ ...draft, month: Number(event.target.value) })}
              >
                {MONTH_NAMES.map((name, index) => (
                  <option key={index} value={index}>
                    {name}
                  </option>
                ))}
              </SelectInput>
            </div>
            <div className="flex flex-col gap-1">
              <label htmlFor={`${id}-year`} className="text-xs text-secondary">
                {t('calendar.year')}
              </label>
              <div className="flex items-center gap-1">
                <IconButton
                  label={t('calendar.previousYear')}
                  icon={Minus}
                  size="sm"
                  disabled={(validYear ? year : month.getFullYear()) <= 100}
                  onClick={() => stepYear(-1)}
                />
                <TextInput
                  id={`${id}-year`}
                  className="w-full text-center tabular-nums"
                  autoFocus
                  inputMode="numeric"
                  maxLength={4}
                  value={draft.year}
                  onFocus={(event) => event.currentTarget.select()}
                  onChange={(event) => setDraft({ ...draft, year: event.target.value })}
                />
                <IconButton
                  label={t('calendar.nextYear')}
                  icon={Plus}
                  size="sm"
                  disabled={(validYear ? year : month.getFullYear()) >= 9999}
                  onClick={() => stepYear(1)}
                />
              </div>
            </div>
            <Button type="submit" size="sm" disabled={!validYear}>
              {t('buttons.select')}
            </Button>
          </form>
        </MenuSurface>
      )}
    </div>
  )
}
