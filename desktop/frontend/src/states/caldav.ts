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
  /** Google's primary calendar, which `name` calls by the account's name. */
  primary?: boolean
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
  google: boolean
  /** The name the user gave; empty for a Google account that goes by its
   * mailbox's. */
  display_name: string
  /** What the account is called everywhere. */
  name: string
  /** Its mailbox's picture, for a Google account. */
  avatar_url?: string
  /** Epoch seconds of the last sync that finished cleanly; 0 for never. */
  last_sync_at: number
  /** Why the last sync failed; empty when it did not. */
  last_error: string
  discovery_warning?: string
  /** Minutes between automatic syncs; 0 for only when asked. */
  sync_interval: number
  calendars: CalendarAccountCalendar[]
}

/** The choices the sync interval offers, in minutes; 0 is manual. */
export const SYNC_INTERVALS = [5, 15, 30, 60, 0] as const

export const caldav$ = observable({
  accounts: [] as CalendarAccount[],
  /** A sync asked for from the calendar is running. */
  syncing: false,
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

/** Locate a calendar server without using the mailbox's credentials. */
export async function discoverCalendarServer(email: string) {
  return (await invoke<{ server_url: string | null }>('caldav.discoverServer', { email }))?.server_url ?? null
}

/** Connect to a server and keep the account. Throws, with the server's
 * reason, when it cannot be reached or refuses the password. */
export async function addCalendarAccount(input: {
  server_url: string
  username: string
  password: string
  /** With a blank password: use this mail account's, when it has the same login. */
  mail_account_id?: string
  /** Empty names it after the server's host. */
  display_name?: string
}) {
  setAccounts(await invoke<{ accounts?: CalendarAccount[] }>('caldav.add', input))
  refreshCalendars()
}

export async function connectGoogleCalendar({ accountId, email }: { accountId?: string; email: string }) {
  const res = await invoke<{ account_id: string; accounts?: CalendarAccount[] }>('caldav.connectGoogle', {
    account_id: accountId,
    login_hint: email,
  })
  setAccounts(res)
  refreshCalendars()
  return res?.account_id
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
async function dropping(method: string, params: Record<string, unknown>): Promise<boolean> {
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
      if (!confirmed) return false
      res = await invoke<AccountsAnswer>(method, { ...params, discard_unsent: true })
    }
    setAccounts(res)
    refreshCalendars()
    return true
  } catch (error) {
    showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
    return false
  }
}

export async function removeCalendarAccount(account: CalendarAccount) {
  const confirmed = await confirmAction({
    title: t('settings.calendars.remove'),
    message: t('settings.calendars.removeConfirm', { name: account.name }),
    confirmLabel: t('settings.calendars.remove'),
    tone: 'danger',
  })
  if (!confirmed) return
  await dropping('caldav.remove', { account_id: account.id })
}

/** Accounts with a sync running from here, and whether another is wanted
 * once it ends: turning on several calendars in a row syncs twice at most,
 * not once per switch. */
const followUps = new Map<string, boolean>()

async function syncSoon(accountId: string) {
  if (followUps.has(accountId)) {
    followUps.set(accountId, true)
    return
  }
  do {
    followUps.set(accountId, false)
    await syncCalendarAccount(accountId)
    refreshCalendars()
  } while (followUps.get(accountId))
  followUps.delete(accountId)
}

/** Show the switch's new state at once; the answer, or a reload when the
 * change fails, puts the truth back. */
function markEnabled(calendarId: string, enabled: boolean) {
  caldav$.accounts.set(
    caldav$.accounts.peek().map((account) => ({
      ...account,
      calendars: account.calendars.map((calendar) =>
        calendar.id === calendarId ? { ...calendar, enabled } : calendar,
      ),
    })),
  )
}

export async function setCalendarEnabled(calendarId: string, enabled: boolean) {
  markEnabled(calendarId, enabled)
  if (enabled) {
    try {
      setAccounts(await invoke<AccountsAnswer>('caldav.setCalendarEnabled', { calendar_id: calendarId, enabled }))
      refreshCalendars()
    } catch (error) {
      void loadCalendarAccounts()
      showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
      return
    }
    // A calendar just turned on has no events here yet: fetch them now
    // rather than at the next scheduled sync.
    const account = caldav$.accounts.peek().find((a) => a.calendars.some((c) => c.id === calendarId))
    if (account) void syncSoon(account.id)
    return
  }
  if (!(await dropping('caldav.setCalendarEnabled', { calendar_id: calendarId, enabled }))) {
    void loadCalendarAccounts()
  }
}

/** Make a new calendar on an account's server. Throws with the server's
 * reason when it will not. */
export async function createCalendar(accountId: string, name: string, color: string) {
  setAccounts(await invoke<AccountsAnswer>('caldav.createCalendar', { account_id: accountId, name, color }))
  refreshCalendars()
}

/** Rename or recolour a calendar on its server. A colour the server will not
 * take is kept on this device, which the answer's `local` says. */
export async function updateCalendar(calendarId: string, changes: { name?: string; color?: string }) {
  const res = await invoke<AccountsAnswer & { local?: boolean }>('caldav.updateCalendar', {
    calendar_id: calendarId,
    ...changes,
  })
  setAccounts(res)
  refreshCalendars()
  if (res?.local) showToast(t('settings.calendars.colorKeptHere'))
}

/** Delete a calendar on its server, after the user confirms: every event in
 * it goes, everywhere. */
export async function deleteCalendar(calendar: CalendarAccountCalendar) {
  const confirmed = await confirmAction({
    title: t('settings.calendars.deleteCalendar'),
    message: t('settings.calendars.deleteCalendarConfirm', { name: calendar.name }),
    confirmLabel: t('settings.calendars.deleteCalendar'),
    tone: 'danger',
  })
  if (!confirmed) return
  try {
    setAccounts(await invoke<AccountsAnswer>('caldav.deleteCalendar', { calendar_id: calendar.id }))
    refreshCalendars()
  } catch (error) {
    showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
  }
}

/** What the account is called here; empty goes back to the server's host. */
export async function renameCalendarAccount(accountId: string, name: string) {
  try {
    setAccounts(await invoke<AccountsAnswer>('caldav.renameAccount', { account_id: accountId, display_name: name }))
    void loadSyncedCalendars()
  } catch (error) {
    showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
  }
}

export async function setSyncInterval(accountId: string, minutes: number) {
  try {
    setAccounts(await invoke<AccountsAnswer>('caldav.setSyncInterval', { account_id: accountId, minutes }))
  } catch (error) {
    showToast(errorMessage(error, t('settings.calendars.saveFailed')), 'error')
  }
}

/** Sync every account now, from the calendar. Failures show on the sync
 * button and the account's settings, not as a toast. */
export async function syncAllCalendars() {
  if (caldav$.syncing.peek()) return
  caldav$.syncing.set(true)
  try {
    await invoke('caldav.sync')
  } catch {
    // Recorded on the account.
  } finally {
    caldav$.syncing.set(false)
  }
  await loadCalendarAccounts()
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
