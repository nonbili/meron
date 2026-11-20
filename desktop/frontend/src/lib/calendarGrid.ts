// Pure helpers for the calendar's month grid. Days are local Dates at
// midnight; the core sends all-day items as UTC midnights (end exclusive) and
// timed ones as instants, so the two are placed on days differently.

const DAY_MS = 86_400_000

/** The first day of the week for the UI language: 0 Sunday … 6 Saturday. */
export function firstDayOfWeek(locale?: string): number {
  try {
    const info = new Intl.Locale(locale ?? navigator.language) as Intl.Locale & {
      weekInfo?: { firstDay: number }
      getWeekInfo?: () => { firstDay: number }
    }
    const firstDay = (info.getWeekInfo?.() ?? info.weekInfo)?.firstDay
    // Intl numbers Monday 1 … Sunday 7.
    if (firstDay) return firstDay % 7
  } catch {
    // An unknown tag, or a webview without Intl.Locale: Sunday.
  }
  return 0
}

/** The six weeks a month view shows for `month` (any Date in it), starting on
 *  `weekStart`. Always 42 days, so the grid does not jump between months. */
export function monthGrid(month: Date, weekStart = 0): Date[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1)
  const offset = (first.getDay() - weekStart + 7) % 7
  return Array.from({ length: 42 }, (_, index) => new Date(first.getFullYear(), first.getMonth(), 1 - offset + index))
}

/** The range a grid covers, as the epoch seconds `calendar.events` takes. A
 *  day either side catches all-day items in zones east or west of UTC. */
export function gridRange(days: Date[]): { from: number; to: number } {
  const first = days[0]
  const last = days[days.length - 1]
  const end = new Date(last.getFullYear(), last.getMonth(), last.getDate() + 1)
  return { from: Math.floor((first.getTime() - DAY_MS) / 1000), to: Math.floor((end.getTime() + DAY_MS) / 1000) }
}

export type DayItem = { start: number; end: number; all_day: boolean }

/** Whether `item` happens on local day `day`. */
export function occursOn(item: DayItem, day: Date): boolean {
  if (item.all_day) {
    // The calendar date, read in UTC, against the day's own date.
    const date = Date.UTC(day.getFullYear(), day.getMonth(), day.getDate()) / 1000
    return item.start <= date && date < Math.max(item.end, item.start + 86_400)
  }
  const dayStart = day.getTime() / 1000
  const dayEnd = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1).getTime() / 1000
  if (item.end <= item.start) return item.start >= dayStart && item.start < dayEnd
  return item.start < dayEnd && item.end > dayStart
}

export function sameMonth(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth()
}

export function sameDay(a: Date, b: Date): boolean {
  return sameMonth(a, b) && a.getDate() === b.getDate()
}

/** `<input type="date">`'s value for a local day. */
export function dateValue(day: Date): string {
  const month = `${day.getMonth() + 1}`.padStart(2, '0')
  const date = `${day.getDate()}`.padStart(2, '0')
  return `${day.getFullYear()}-${month}-${date}`
}

/** `<input type="time">`'s value for a local instant. */
export function timeValue(at: Date): string {
  return `${`${at.getHours()}`.padStart(2, '0')}:${`${at.getMinutes()}`.padStart(2, '0')}`
}

/** A date input's value as the local Date it names, or null when blank. */
export function parseDateValue(value: string): Date | null {
  const [year, month, day] = value.split('-').map(Number)
  if (!year || !month || !day) return null
  return new Date(year, month - 1, day)
}

/** The local instant a date and a time input name together, in epoch seconds. */
export function localInstant(date: string, time: string): number {
  const day = parseDateValue(date)
  if (!day) return 0
  const [hours, minutes] = time.split(':').map(Number)
  day.setHours(hours || 0, minutes || 0, 0, 0)
  return Math.floor(day.getTime() / 1000)
}

/** A date input's day as the UTC midnight the core stores all-day bounds in. */
export function utcDate(date: string): number {
  const day = parseDateValue(date)
  return day ? Date.UTC(day.getFullYear(), day.getMonth(), day.getDate()) / 1000 : 0
}

/** The date input value for an all-day bound (a UTC midnight). */
export function utcDateValue(epochSeconds: number): string {
  const at = new Date(epochSeconds * 1000)
  return dateValue(new Date(at.getUTCFullYear(), at.getUTCMonth(), at.getUTCDate()))
}
