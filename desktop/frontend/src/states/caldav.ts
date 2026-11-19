import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'
import { errorMessage } from '../lib/errors'
import { t } from '../lib/i18n'
import { loadCalendar, loadSyncedCalendars } from './calendar'
import { confirmAction, showToast } from './ui'

/** One calendar of a CalDAV account, as `caldav.accounts` lists it. */
export type CalendarAccountCalendar = {
  id: string
  name: string
  color: string
  read_only: boolean
  enabled: boolean
  /** Holds events (VEVENT); only these are shown in the calendar. */
  events: boolean
  /** Holds to-dos (VTODO). */
  tasks: boolean
}

/** A CalDAV account (see meron-core/src/caldav/service.rs). */
export type CalendarAccount = {
  id: string
  server_url: string
  username: string
  display_name: string
  /** Epoch seconds of the last sync that finished cleanly; 0 for never. */
  last_sync_at: number
  /** Why the last sync failed; empty when it did not. */
  last_error: string
  calendars: CalendarAccountCalendar[]
}

export const caldav$ = observable({
  accounts: [] as CalendarAccount[],
})

function setAccounts(res: { accounts?: CalendarAccount[] } | null | undefined) {
  if (res?.accounts) caldav$.accounts.set(res.accounts)
}

/** Everything that shows synced calendars reads them again. */
function refreshCalendars() {
  void loadSyncedCalendars()
  void loadCalendar()
}

export async function loadCalendarAccounts() {
  try {
    setAccounts(await invoke<{ accounts?: CalendarAccount[] }>('caldav.accounts'))
  } catch (error) {
    showToast(errorMessage(error, t('settings.calendars.loadFailed')), 'error')
  }
}

/** Connect to a server and keep the account. Throws, with the server's
 * reason, when it cannot be reached or refuses the password. */
export async function addCalendarAccount(input: { server_url: string; username: string; password: string }) {
  setAccounts(await invoke<{ accounts?: CalendarAccount[] }>('caldav.add', input))
  refreshCalendars()
}

/** Throws when the server refuses the new password. */
export async function setCalendarPassword(accountId: string, password: string) {
  setAccounts(await invoke<{ accounts?: CalendarAccount[] }>('caldav.setPassword', { account_id: accountId, password }))
}

type AccountsAnswer = {
  accounts?: CalendarAccount[]
  /** Set when nothing was done: this many local changes could not be sent
   * yet and would be lost. Asking again with `discard_unsent` goes ahead. */
  unsent?: number
}

/** Run a change that drops a calendar's events from this device. When some
 * of them were changed here and cannot reach the server yet, ask before
 * throwing those changes away. */
async function dropping(method: string, params: Record<string, unknown>) {
  try {
    let res = await invoke<AccountsAnswer>(method, params)
    if (res?.unsent) {
      setAccounts(res)
      const confirmed = await confirmAction({
        title: t('settings.calendars.unsentTitle'),
        message: t('settings.calendars.unsentConfirm', { count: res.unsent }),
        confirmLabel: t('settings.calendars.discardUnsent'),
        tone: 'danger',
      })
      if (!confirmed) return
      res = await invoke<AccountsAnswer>(method, { ...params, discard_unsent: true })
    }
    setAccounts(res)
    refreshCalendars()
  } catch (error) {
    showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
  }
}

export async function removeCalendarAccount(account: CalendarAccount) {
  const confirmed = await confirmAction({
    title: t('settings.calendars.remove'),
    message: t('settings.calendars.removeConfirm', { name: account.display_name || account.server_url }),
    confirmLabel: t('settings.calendars.remove'),
    tone: 'danger',
  })
  if (!confirmed) return
  await dropping('caldav.remove', { account_id: account.id })
}

export async function setCalendarEnabled(calendarId: string, enabled: boolean) {
  if (enabled) {
    try {
      setAccounts(await invoke<AccountsAnswer>('caldav.setCalendarEnabled', { calendar_id: calendarId, enabled }))
      refreshCalendars()
    } catch (error) {
      showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
    }
    return
  }
  await dropping('caldav.setCalendarEnabled', { calendar_id: calendarId, enabled })
}

/** Sync one account now. A failure is recorded on the account, where its
 * card shows it, so it is not also toasted. */
export async function syncCalendarAccount(accountId: string) {
  try {
    await invoke('caldav.sync', { account_id: accountId })
  } catch {
    // Shown on the account's card from last_error.
  }
  await loadCalendarAccounts()
}
