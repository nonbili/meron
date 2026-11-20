// The repeat rules the custom repeat editor can say, as the core keeps them in
// an event's `repeat` (see meron-core/src/calendar/rrule.rs): a plain choice
// (`weekly`), or an RRULE in a fixed canonical form, so a rule saved without
// changes compares equal to the one the editor opened with.

export type RepeatFreq = 'daily' | 'weekly' | 'monthly' | 'yearly'

/** 0 Monday … 6 Sunday, as RRULE orders weekdays. */
export type RuleWeekday = 0 | 1 | 2 | 3 | 4 | 5 | 6

export type RepeatEnd = { kind: 'never' } | { kind: 'count'; count: number } | { kind: 'until'; date: string }

export type RepeatRule = {
  freq: RepeatFreq
  interval: number
  /** A weekly rule's days. */
  weekdays: RuleWeekday[]
  /** A monthly rule's day of the month, or its nth weekday (-1 the last). */
  monthly: { kind: 'day'; day: number } | { kind: 'weekday'; nth: number; weekday: RuleWeekday } | null
  /** The day weeks start on, which picks the weeks an every-other-week rule
   * on several days skips; Monday, RFC 5545's default, otherwise. The editor
   * keeps it but does not offer it. */
  weekStart?: RuleWeekday
  /** `until` is the last day, `YYYY-MM-DD`. */
  end: RepeatEnd
}

export const PLAIN_REPEATS: RepeatFreq[] = ['daily', 'weekly', 'monthly', 'yearly']
export const MAX_INTERVAL = 999
export const MAX_COUNT = 999
const CODES = ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU']

export function isPlainRepeat(value: string): value is RepeatFreq {
  return (PLAIN_REPEATS as string[]).includes(value)
}

/** A stored repeat (plain or RRULE) as a rule, or null for none or for one
 * outside what the editor says. */
export function parseRepeat(value: string): RepeatRule | null {
  if (isPlainRepeat(value)) return { freq: value, interval: 1, weekdays: [], monthly: null, end: { kind: 'never' } }
  if (!value.includes('=')) return null
  const parts = new Map<string, string>()
  for (const part of value.split(';')) {
    if (!part) continue
    const [key, val] = part.split('=')
    if (!key || val === undefined) return null
    parts.set(key.toUpperCase(), val.toUpperCase())
  }
  const freq = parts.get('FREQ')?.toLowerCase()
  if (!freq || !isPlainRepeat(freq)) return null
  const interval = Number(parts.get('INTERVAL') ?? 1)
  if (!Number.isInteger(interval) || interval < 1) return null
  const rule: RepeatRule = { freq, interval, weekdays: [], monthly: null, end: { kind: 'never' } }
  const byDay = parts.get('BYDAY')
  const monthDay = parts.get('BYMONTHDAY')
  if (byDay && freq === 'weekly') {
    const days = byDay.split(',').map((code) => CODES.indexOf(code))
    if (days.some((day) => day < 0)) return null
    rule.weekdays = [...new Set(days)].sort((a, b) => a - b) as RuleWeekday[]
    const weekStart = parts.get('WKST')
    if (weekStart && CODES.indexOf(weekStart) < 0) return null
    if (weekStart) rule.weekStart = CODES.indexOf(weekStart) as RuleWeekday
  } else if (byDay && freq === 'monthly') {
    const match = /^([+-]?\d)(MO|TU|WE|TH|FR|SA|SU)$/.exec(byDay)
    if (!match) return null
    rule.monthly = { kind: 'weekday', nth: Number(match[1]), weekday: CODES.indexOf(match[2]) as RuleWeekday }
  } else if (monthDay && freq === 'monthly') {
    rule.monthly = { kind: 'day', day: Number(monthDay) }
  } else if (byDay || monthDay) {
    return null
  }
  const count = parts.get('COUNT')
  const until = parts.get('UNTIL')
  if (count) rule.end = { kind: 'count', count: Number(count) }
  else if (until && /^\d{8}$/.test(until))
    rule.end = { kind: 'until', date: `${until.slice(0, 4)}-${until.slice(4, 6)}-${until.slice(6, 8)}` }
  else if (until) return null
  return rule
}

/** The rule as the core keeps it: a plain choice when it says no more. */
export function formatRepeat(rule: RepeatRule): string {
  const weekdays = rule.freq === 'weekly' ? rule.weekdays : []
  const monthly = rule.freq === 'monthly' ? rule.monthly : null
  if (rule.interval === 1 && !weekdays.length && !monthly && rule.end.kind === 'never') return rule.freq
  const parts = [`FREQ=${rule.freq.toUpperCase()}`]
  if (rule.interval !== 1) parts.push(`INTERVAL=${rule.interval}`)
  if (weekdays.length)
    parts.push(
      `BYDAY=${[...weekdays]
        .sort((a, b) => a - b)
        .map((d) => CODES[d])
        .join(',')}`,
    )
  if (monthly?.kind === 'weekday') parts.push(`BYDAY=${monthly.nth}${CODES[monthly.weekday]}`)
  if (monthly?.kind === 'day') parts.push(`BYMONTHDAY=${monthly.day}`)
  // Written only where it picks the weeks, as the core keeps it.
  if (rule.weekStart && rule.interval > 1 && weekdays.length > 1) parts.push(`WKST=${CODES[rule.weekStart]}`)
  if (rule.end.kind === 'count') parts.push(`COUNT=${rule.end.count}`)
  if (rule.end.kind === 'until') parts.push(`UNTIL=${rule.end.date.replaceAll('-', '')}`)
  return parts.join(';')
}

/** A date's weekday, Monday 0. */
export function ruleWeekday(date: Date): RuleWeekday {
  return ((date.getDay() + 6) % 7) as RuleWeekday
}

/** Which of its weekday in the month a date is (1–5), and whether it is the
 * last of them. */
export function weekdayPlace(date: Date): { nth: number; last: boolean } {
  const daysInMonth = new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate()
  return { nth: Math.ceil(date.getDate() / 7), last: date.getDate() + 7 > daysInMonth }
}

/** A stored repeat moved with its series, from starting on `from` to `to`
 * (local midnights): weekly days move by as many days, a monthly day or
 * weekday becomes the new start's, as the core moves an existing series'. */
export function alignRepeat(value: string, from: Date, to: Date): string {
  const rule = isPlainRepeat(value) ? null : parseRepeat(value)
  if (!rule) return value
  const weekday = ruleWeekday(to)
  if (rule.freq === 'weekly' && rule.weekdays.length) {
    const shift = (((Math.round((to.getTime() - from.getTime()) / 86_400_000) % 7) + 7) % 7) as number
    const days = new Set(rule.weekdays.map((day) => ((day + shift) % 7) as RuleWeekday))
    // The weeks move with the days, so an every-other-week rule keeps the
    // same weeks.
    if (rule.interval > 1 && rule.weekdays.length > 1)
      rule.weekStart = (((rule.weekStart ?? 0) + shift) % 7) as RuleWeekday
    days.add(weekday)
    rule.weekdays = [...days].sort((a, b) => a - b)
  }
  if (rule.freq === 'monthly' && rule.monthly?.kind === 'day') rule.monthly = { kind: 'day', day: to.getDate() }
  if (rule.freq === 'monthly' && rule.monthly?.kind === 'weekday') {
    const place = weekdayPlace(to)
    rule.monthly = { kind: 'weekday', nth: rule.monthly.nth === -1 && place.last ? -1 : place.nth, weekday }
  }
  return formatRepeat(rule)
}

/** A rule to start the custom editor from: the one given, or one repeating
 * as the plain choice does, from the event's first day. */
export function initialRule(value: string, first: Date): RepeatRule {
  const rule = parseRepeat(value) ?? parseRepeat('weekly')!
  if (rule.freq === 'weekly' && !rule.weekdays.length) rule.weekdays = [ruleWeekday(first)]
  if (rule.freq === 'monthly' && !rule.monthly) rule.monthly = { kind: 'day', day: first.getDate() }
  return rule
}

type Translate = (key: string, values?: Record<string, unknown>) => string

/** Weekday names in `locale`, Monday first. */
export function weekdayNames(locale: string, width: 'narrow' | 'short' | 'long'): string[] {
  const format = new Intl.DateTimeFormat(locale, { weekday: width, timeZone: 'UTC' })
  // 5 January 1970 was a Monday.
  return CODES.map((_, index) => format.format(new Date(Date.UTC(1970, 0, 5 + index))))
}

/** The word for which of its weekday in the month: "third", "last". */
export function nthKey(nth: number): string {
  return nth >= 1 && nth <= 5 ? `calendar.repeat.nth${nth}` : 'calendar.repeat.nthLast'
}

const EVERY: Record<RepeatFreq, string> = {
  daily: 'calendar.repeat.everyDays',
  weekly: 'calendar.repeat.everyWeeks',
  monthly: 'calendar.repeat.everyMonths',
  yearly: 'calendar.repeat.everyYears',
}

/** The rule in words: "Every 2 weeks on Mon, Wed, until 30 Jun 2027". */
export function describeRepeat(rule: RepeatRule, t: Translate, locale: string): string {
  let text = t(EVERY[rule.freq], { count: rule.interval })
  if (rule.freq === 'weekly' && rule.weekdays.length) {
    const names = weekdayNames(locale, 'short')
    const days = new Intl.ListFormat(locale, { style: 'short', type: 'unit' }).format(
      [...rule.weekdays].sort((a, b) => a - b).map((day) => names[day]),
    )
    text = t('calendar.repeat.summaryOnDays', { rule: text, days })
  }
  if (rule.freq === 'monthly' && rule.monthly?.kind === 'day') {
    text = t('calendar.repeat.summaryOnMonthDay', { rule: text, day: rule.monthly.day })
  }
  if (rule.freq === 'monthly' && rule.monthly?.kind === 'weekday') {
    text = t('calendar.repeat.summaryOnWeekday', {
      rule: text,
      nth: t(nthKey(rule.monthly.nth)),
      weekday: weekdayNames(locale, 'long')[rule.monthly.weekday],
    })
  }
  if (rule.end.kind === 'count') text = t('calendar.repeat.summaryCount', { rule: text, count: rule.end.count })
  if (rule.end.kind === 'until' && /^\d{4}-\d{2}-\d{2}$/.test(rule.end.date)) {
    const [year, month, day] = rule.end.date.split('-').map(Number)
    const date = new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeZone: 'UTC' }).format(
      new Date(Date.UTC(year, month - 1, day)),
    )
    text = t('calendar.repeat.summaryUntil', { rule: text, date })
  }
  return text
}
