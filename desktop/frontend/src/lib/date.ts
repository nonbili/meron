// Pure date helpers shared across the UI and state layers. The sidecar sends
// `date` as Unix epoch seconds (0 when unknown); these format it for display in
// the user's local time.

/** Convert epoch seconds to a Date, or null when unknown (0/falsy). */
function fromEpochSeconds(epochSeconds: number): Date | null {
  if (!epochSeconds) return null
  return new Date(epochSeconds * 1000)
}

/** Gmail-style thread-list timestamp: time today, month/day this year, else month/day/year. */
export function formatThreadDate(epochSeconds: number): string {
  const date = fromEpochSeconds(epochSeconds)
  if (!date) return ''
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })
  }
  const options: Intl.DateTimeFormatOptions =
    date.getFullYear() === now.getFullYear()
      ? { month: 'short', day: 'numeric' }
      : { month: 'short', day: 'numeric', year: 'numeric' }
  return date.toLocaleDateString([], options)
}

/** A task's due date: month/day, with the year once it isn't this one. */
export function formatDueDate(epochSeconds: number, hasTime = false): string {
  const date = fromEpochSeconds(epochSeconds)
  if (!date) return ''
  const options: Intl.DateTimeFormatOptions =
    date.getFullYear() === new Date().getFullYear()
      ? { month: 'short', day: 'numeric' }
      : { month: 'short', day: 'numeric', year: 'numeric' }
  return (
    date.toLocaleDateString([], options) +
    (hasTime ? ` ${date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}` : '')
  )
}

/** `<input type="date">` wants a local-time YYYY-MM-DD, not an ISO instant. */
export function toDateInputValue(epochSeconds: number): string {
  const date = fromEpochSeconds(epochSeconds)
  if (!date) return ''
  const month = `${date.getMonth() + 1}`.padStart(2, '0')
  const day = `${date.getDate()}`.padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

/** The inverse: a date input's value as epoch seconds at local midnight. */
export function fromDateInputValue(value: string): number {
  if (!value) return 0
  const [year, month, day] = value.split('-').map(Number)
  if (!year || !month || !day) return 0
  return Math.floor(new Date(year, month - 1, day).getTime() / 1000)
}

/** A date-only deadline becomes overdue on the following local day. */
export function taskIsOverdue(dueAt: number, nowMillis = Date.now(), hasTime = false): boolean {
  if (hasTime) return dueAt > 0 && dueAt * 1000 < nowMillis
  const today = new Date(nowMillis)
  today.setHours(0, 0, 0, 0)
  return dueAt > 0 && dueAt * 1000 < today.getTime()
}

/** When a calendar event happens: its day (or days), and its times unless it
 * lasts all day. All-day bounds are UTC midnights with an exclusive end. */
export function formatEventRange(start: number, end: number, allDay: boolean): string {
  if (!start) return ''
  const from = new Date(start * 1000)
  if (allDay) {
    const options: Intl.DateTimeFormatOptions = {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      timeZone: 'UTC',
    }
    const last = new Date(Math.max(start, end - 86_400) * 1000)
    return formatRange(new Intl.DateTimeFormat([], options), from, last)
  }
  const options: Intl.DateTimeFormatOptions = {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }
  return formatRange(new Intl.DateTimeFormat([], options), from, new Date(Math.max(start, end) * 1000))
}

export function formatRange(format: Intl.DateTimeFormat, from: Date, to: Date): string {
  if (from.getTime() === to.getTime()) return format.format(from)
  return format.formatRange ? format.formatRange(from, to) : `${format.format(from)} – ${format.format(to)}`
}

/** Set local date and optional time together, letting Date apply DST rules. */
export function taskDueAt(date: string, time: string): number {
  const at = fromDateInputValue(date)
  if (!at || !time) return at
  const [hour, minute] = time.split(':').map(Number)
  if (!Number.isInteger(hour) || !Number.isInteger(minute) || hour < 0 || hour > 23 || minute < 0 || minute > 59)
    return at
  const value = new Date(at * 1000)
  value.setHours(hour, minute, 0, 0)
  return Math.floor(value.getTime() / 1000)
}

export function taskDueTime(at: number, hasTime = false): string {
  if (!at || !hasTime) return ''
  const value = new Date(at * 1000)
  return `${String(value.getHours()).padStart(2, '0')}:${String(value.getMinutes()).padStart(2, '0')}`
}
