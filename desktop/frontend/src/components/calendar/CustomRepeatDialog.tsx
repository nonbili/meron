import { useId, useMemo, useState } from 'react'
import { firstDayOfWeek, parseDateValue } from '../../lib/calendarGrid'
import { useTranslation } from '../../lib/i18n'
import {
  MAX_COUNT,
  MAX_INTERVAL,
  describeRepeat,
  formatRepeat,
  initialRule,
  parseRepeat,
  nthKey,
  ruleWeekday,
  weekdayNames,
  weekdayPlace,
  type RepeatEnd,
  type RepeatFreq,
  type RepeatRule,
  type RuleWeekday,
} from '../../lib/repeatRule'
import { clsx } from '../../lib/utils'
import { Button } from '../button/Button'
import { SelectInput, TextInput } from '../field/Field'
import { Dialog } from '../dialog/Dialog'

const UNITS: { freq: RepeatFreq; key: string }[] = [
  { freq: 'daily', key: 'calendar.repeat.unitDays' },
  { freq: 'weekly', key: 'calendar.repeat.unitWeeks' },
  { freq: 'monthly', key: 'calendar.repeat.unitMonths' },
  { freq: 'yearly', key: 'calendar.repeat.unitYears' },
]

type Monthly = NonNullable<RepeatRule['monthly']>
const monthlyValue = (monthly: Monthly) =>
  monthly.kind === 'day' ? `day:${monthly.day}` : `weekday:${monthly.nth}:${monthly.weekday}`

/** Every N days, weeks, months or years: on which weekdays, on which day of
 * the month, and until when. Done hands back the rule as the core keeps it
 * (a plain choice when it says no more). `startDate` is the event's first
 * day, `YYYY-MM-DD`, which the choices are offered from. */
export function CustomRepeatDialog({
  value,
  startDate,
  onDone,
  onClose,
}: {
  value: string
  startDate: string
  onDone: (repeat: string) => void
  onClose: () => void
}) {
  const { t, i18n } = useTranslation()
  const id = useId()
  const locale = i18n.language.replace('_', '-')
  const first = useMemo(() => parseDateValue(startDate) ?? new Date(), [startDate])
  const startWeekday = ruleWeekday(first)
  // The series' first day is always one of a weekly rule's: RFC 5545 leaves a
  // start the rule does not match to each client to read its own way.
  // The week the picker shows, Monday 0: an every-other-week rule made here
  // skips the weeks as they are shown. A rule that already has days keeps
  // its own week start.
  const weekStart = ((firstDayOfWeek(locale) + 6) % 7) as RuleWeekday
  const [rule, setRule] = useState(() => {
    const own = parseRepeat(value)
    const rule = initialRule(value, first)
    if (!own?.weekdays.length) rule.weekStart = weekStart
    if (rule.freq === 'weekly' && !rule.weekdays.includes(startWeekday)) rule.weekdays.push(startWeekday)
    return rule
  })
  // What the number fields hold while being typed, which may be empty.
  const [intervalText, setIntervalText] = useState(String(rule.interval))
  const [countText, setCountText] = useState(rule.end.kind === 'count' ? String(rule.end.count) : '10')
  const [untilText, setUntilText] = useState(rule.end.kind === 'until' ? rule.end.date : startDate)

  const update = (changes: Partial<RepeatRule>) => setRule((current) => ({ ...current, ...changes }))
  const setFreq = (freq: RepeatFreq) =>
    setRule((current) => ({
      ...current,
      freq,
      weekdays:
        freq === 'weekly' && !current.weekdays.includes(startWeekday)
          ? [...current.weekdays, startWeekday]
          : current.weekdays,
      monthly: freq === 'monthly' && !current.monthly ? { kind: 'day', day: first.getDate() } : current.monthly,
    }))

  const order = Array.from({ length: 7 }, (_, index) => ((weekStart + index) % 7) as RuleWeekday)
  const narrow = weekdayNames(locale, 'narrow')
  const long = weekdayNames(locale, 'long')

  const place = weekdayPlace(first)
  const monthlyChoices: Monthly[] = [{ kind: 'day', day: first.getDate() }]
  monthlyChoices.push({ kind: 'weekday', nth: place.nth, weekday: ruleWeekday(first) })
  if (place.last) monthlyChoices.push({ kind: 'weekday', nth: -1, weekday: ruleWeekday(first) })
  // One a server set, from another day than this event's start, stays offered.
  if (rule.monthly && !monthlyChoices.some((choice) => monthlyValue(choice) === monthlyValue(rule.monthly!))) {
    monthlyChoices.push(rule.monthly)
  }
  const monthlyLabel = (monthly: Monthly) =>
    monthly.kind === 'day'
      ? t('calendar.repeat.onMonthDay', { day: monthly.day })
      : t('calendar.repeat.onWeekday', { nth: t(nthKey(monthly.nth)), weekday: long[monthly.weekday] })

  const interval = Number(intervalText)
  const count = Number(countText)
  const intervalValid = Number.isInteger(interval) && interval >= 1 && interval <= MAX_INTERVAL
  const end: RepeatEnd =
    rule.end.kind === 'count'
      ? { kind: 'count', count }
      : rule.end.kind === 'until'
        ? { kind: 'until', date: untilText }
        : rule.end
  const endValid =
    end.kind === 'never' ||
    (end.kind === 'count' && Number.isInteger(count) && count >= 1 && count <= MAX_COUNT) ||
    (end.kind === 'until' && !!parseDateValue(end.date) && end.date >= startDate)
  const valid = intervalValid && endValid && (rule.freq !== 'weekly' || rule.weekdays.length > 0)
  // While a field is being typed into, the summary shows what is valid of it.
  const result: RepeatRule = {
    ...rule,
    interval: intervalValid ? interval : 1,
    end: endValid ? end : { kind: 'never' },
  }

  const label = 'text-[0.6875rem] font-semibold text-secondary'
  const endOption = (kind: RepeatEnd['kind'], text: string) => (
    <label className="flex cursor-pointer items-center gap-2 text-xs text-primary">
      <input
        type="radio"
        name={`${id}-end`}
        className="accent-accent"
        checked={rule.end.kind === kind}
        onChange={() =>
          update({
            end:
              kind === 'count'
                ? { kind, count: Number(countText) || 1 }
                : kind === 'until'
                  ? { kind, date: untilText }
                  : { kind: 'never' },
          })
        }
      />
      {text}
    </label>
  )

  return (
    <Dialog onClose={onClose} aria-labelledby={`${id}-title`} className="max-w-sm p-6">
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          // React carries a submit up through the portal to the event
          // editor's own form, which would save the event.
          event.stopPropagation()
          if (valid) onDone(formatRepeat(result))
        }}
      >
        <h2 id={`${id}-title`} className="text-base font-bold">
          {t('calendar.repeat.customTitle')}
        </h2>

        <div className="flex flex-col gap-1.5">
          <span className={label}>{t('calendar.repeat.repeatEvery')}</span>
          <div className="flex items-center gap-2">
            <TextInput
              type="number"
              inputMode="numeric"
              min={1}
              max={MAX_INTERVAL}
              className="w-20"
              invalid={!intervalValid}
              aria-label={t('calendar.repeat.repeatEvery')}
              value={intervalText}
              onChange={(event) => setIntervalText(event.target.value)}
            />
            <SelectInput
              wrapperClassName="flex-1"
              aria-label={t('calendar.fields.repeat')}
              value={rule.freq}
              onChange={(event) => setFreq(event.target.value as RepeatFreq)}
            >
              {UNITS.map(({ freq, key }) => (
                <option key={freq} value={freq}>
                  {t(key, { count: intervalValid ? interval : 1 })}
                </option>
              ))}
            </SelectInput>
          </div>
        </div>

        {rule.freq === 'weekly' && (
          <div className="flex flex-col gap-1.5">
            <span id={`${id}-days`} className={label}>
              {t('calendar.repeat.repeatOn')}
            </span>
            <div role="group" aria-labelledby={`${id}-days`} className="flex gap-1.5">
              {order.map((day) => {
                const on = rule.weekdays.includes(day)
                const locked = day === startWeekday
                return (
                  <button
                    key={day}
                    type="button"
                    aria-pressed={on}
                    disabled={locked}
                    aria-label={long[day]}
                    title={long[day]}
                    onClick={() =>
                      update({ weekdays: on ? rule.weekdays.filter((d) => d !== day) : [...rule.weekdays, day] })
                    }
                    className={clsx(
                      'flex size-8 cursor-pointer items-center justify-center rounded-full text-xs font-semibold transition-colors disabled:cursor-default',
                      on ? 'bg-accent text-accent-label' : 'bg-active text-secondary hover:text-primary',
                    )}
                  >
                    {narrow[day]}
                  </button>
                )
              })}
            </div>
          </div>
        )}

        {rule.freq === 'monthly' && rule.monthly && (
          <SelectInput
            aria-label={t('calendar.repeat.repeatOn')}
            value={monthlyValue(rule.monthly)}
            onChange={(event) =>
              update({ monthly: monthlyChoices.find((choice) => monthlyValue(choice) === event.target.value) })
            }
          >
            {monthlyChoices.map((choice) => (
              <option key={monthlyValue(choice)} value={monthlyValue(choice)}>
                {monthlyLabel(choice)}
              </option>
            ))}
          </SelectInput>
        )}

        <fieldset className="flex flex-col gap-2">
          <legend className={clsx(label, 'mb-1.5')}>{t('calendar.fields.ends')}</legend>
          {endOption('never', t('calendar.repeat.endNever'))}
          <div className="flex items-center gap-3">
            <span className="w-16 shrink-0">{endOption('until', t('calendar.repeat.endOn'))}</span>
            <TextInput
              type="date"
              className="flex-1"
              min={startDate}
              disabled={rule.end.kind !== 'until'}
              invalid={rule.end.kind === 'until' && !endValid}
              aria-label={t('calendar.repeat.endOn')}
              value={untilText}
              onChange={(event) => setUntilText(event.target.value)}
            />
          </div>
          <div className="flex items-center gap-3">
            <span className="w-16 shrink-0">{endOption('count', t('calendar.repeat.endAfter'))}</span>
            <TextInput
              type="number"
              inputMode="numeric"
              min={1}
              max={MAX_COUNT}
              className="w-20"
              disabled={rule.end.kind !== 'count'}
              invalid={rule.end.kind === 'count' && !endValid}
              aria-label={t('calendar.repeat.endAfter')}
              value={countText}
              onChange={(event) => setCountText(event.target.value)}
            />
            <span className="text-xs text-secondary">
              {t('calendar.repeat.occurrences', { count: Number(countText) || 1 })}
            </span>
          </div>
        </fieldset>

        <p role="status" className="text-xs text-secondary">
          {describeRepeat(result, t, locale)}
        </p>

        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            {t('buttons.cancel')}
          </Button>
          <Button type="submit" disabled={!valid}>
            {t('buttons.done')}
          </Button>
        </div>
      </form>
    </Dialog>
  )
}
