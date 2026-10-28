import { t } from './i18n'

// Event alerts, Apple Calendar style: app-wide defaults for timed and all-day
// events, and up to two alerts an event can set for itself. A timed event's
// leads are minutes before its start; an all-day event's are days before its
// date, at 09:00. The core (meron-core/src/reminders.rs) plans the notifications.

/** The leads offered for timed events, in minutes. */
export const ALERT_MINUTES = [0, 5, 10, 15, 30, 60, 120, 1440, 2880]

/** The leads offered for all-day events, in days. */
export const ALL_DAY_ALERT_DAYS = [0, 1, 2, 7]

/** The default alert setting that turns event alerts off. */
export const NO_ALERT = -1

export const DEFAULT_ALERT_MINUTES = 10

export const DEFAULT_ALL_DAY_ALERT_DAYS = 0

/** The leads an event of this kind offers. */
export function alertOptions(allDay: boolean): number[] {
  return allDay ? ALL_DAY_ALERT_DAYS : ALERT_MINUTES
}

/** "At time of event", "15 minutes before", "2 hours before", "1 day before";
 * for an all-day event "On the day (9 a.m.)", "2 days before (9 a.m.)". */
export function alertLabel(lead: number, allDay = false): string {
  if (lead === NO_ALERT) return t('calendar.alert.none')
  if (allDay) {
    if (lead === 0) return t('calendar.alert.allDayOnDay')
    if (lead === 7) return t('calendar.alert.allDayWeek')
    return t('calendar.alert.allDayDays', { count: lead })
  }
  if (lead === 0) return t('calendar.alert.atStart')
  if (lead % 1440 === 0) return t('calendar.alert.days', { count: lead / 1440 })
  if (lead % 60 === 0) return t('calendar.alert.hours', { count: lead / 60 })
  return t('calendar.alert.minutes', { count: lead })
}
