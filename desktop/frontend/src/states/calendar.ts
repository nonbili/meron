import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'
import { firstDayOfWeek, gridRange, monthGrid } from '../lib/calendarGrid'
import { errorMessage } from '../lib/errors'
import { t } from '../lib/i18n'
import type { Invite, InviteResponse, Message } from '../types'
import { openThreadTabById } from './compose'
import { compose$ } from './composeState'
import { mail$ } from './mailState'
import { persistedField } from '../lib/sessionPref'
import { isNarrowWindow } from '../lib/useNarrowWindow'
import { settings$ } from './settings'
import { closeTasksPanel, openTasksPanel } from './tasks'
import { showToast, showUndoToast, ui$ } from './ui'

export type CalendarRepeat = '' | 'daily' | 'weekly' | 'monthly' | 'yearly'

/** One occurrence in a calendar range, as `calendar.events` shapes it (see
 * meron-core/src/calendar/events.rs). Times are epoch seconds; an all-day
 * item's are UTC midnights with `end` exclusive. */
export type CalendarItem = {
  /** The event's id (or the task's, for `source: 'task'`). */
  id: string
  /** Unique within one answer: a repeating event has one item per occurrence. */
  key: string
  source: 'event' | 'invite' | 'task'
  title: string
  notes: string
  location: string
  start: number
  end: number
  all_day: boolean
  recurring: boolean
  repeat: CalendarRepeat
  account: string
  thread_id: string
  message_id: string
  /** An invitation's answer: `accepted` or `tentative`. */
  response: string
  /** A task's list. */
  list_id?: string
  /** The event's own alerts, minutes before start; null follows the default
   * and an empty list means none. */
  alerts?: number[] | null
  /** Whether `alerts` count all-day days rather than minutes: the series'
   * kind, which one occurrence of an invitation can differ from. */
  alerts_all_day?: boolean
}

/** What the event editor sends, and what it was opened on. */
export type EventInput = {
  title: string
  notes: string
  location: string
  start: number
  end: number
  all_day: boolean
  repeat: CalendarRepeat
  /** Editing a repeating event: the start of the occurrence the editor was
   * opened on, so the core moves the series by the change rather than
   * restarting it there. */
  occurrence_start?: number
  /** See [CalendarItem.alerts]. */
  alerts: number[] | null
}

export type EventEditor = { mode: 'new'; day: number } | { mode: 'edit'; item: CalendarItem }

/** How many days the panel's agenda lists, from the day picked onwards. */
export const AGENDA_DAYS = 7

export const calendar$ = observable({
  /** Any instant in the month on show, epoch milliseconds. */
  month: Date.now(),
  /** The day the panel's agenda starts on, local midnight in epoch milliseconds. */
  day: startOfDay(new Date()).getTime(),
  items: [] as CalendarItem[],
  loading: false,
  editor: null as EventEditor | null,
})

let loadSeq = 0

function startOfDay(at: Date): Date {
  return new Date(at.getFullYear(), at.getMonth(), at.getDate())
}

/** Whether either calendar is on screen, so a change elsewhere re-reads it. */
function calendarShowing(): boolean {
  return ui$.calendarOpen.peek() || ui$.calendarPanelOpen.peek()
}

// A panel left open is part of how the app was arranged, as with Tasks.
const panelOpenSession = persistedField(ui$.calendarPanelOpen, 'session_calendar_panel', (raw) =>
  typeof raw === 'boolean' ? raw : undefined,
)

/** Prefs keys this module owns; boot requests them in its single prefsGet. */
export const CALENDAR_SESSION_KEYS = [panelOpenSession.key]

export function restoreCalendarSession(prefs: Record<string, unknown>) {
  if (prefs.calendar_enabled === false) return
  panelOpenSession.restore(prefs)
  if (ui$.calendarPanelOpen.peek()) openCalendarPanel()
}

/** Show the calendar in the main pane, in place of mail or a board: the rail's
 * calendar item. An open panel stays open, hidden while the full view shows the
 * same days, and is back beside mail or the board once the full view gives way
 * to them. */
export function openCalendar() {
  ui$.calendarOpen.set(true)
  void loadCalendar()
}

export function closeCalendar() {
  ui$.calendarOpen.set(false)
  calendar$.editor.set(null)
}

/** Show the calendar beside mail, in the slot Tasks would otherwise hold. From
 * the full view, collapse back into it. */
export function openCalendarPanel() {
  closeTasksPanel()
  closeCalendar()
  ui$.calendarPanelOpen.set(true)
  void loadCalendar()
}

export function closeCalendarPanel() {
  ui$.calendarPanelOpen.set(false)
  calendar$.editor.set(null)
}

// The two share the right-hand slot: Tasks opened (from its button, or a due
// task pressed in the agenda) takes it over.
ui$.tasksPanelOpen.onChange(({ value }) => {
  if (value && ui$.calendarPanelOpen.peek()) closeCalendarPanel()
})

// The event editor belongs to the calendar it was opened from. However that
// calendar goes — a board or an account opened over the full view, the setting
// switched off — the editor goes with it, rather than lingering unseen and
// keeping mail's shortcuts at bay (AppHotkeys treats an open editor as modal).
ui$.calendarOpen.onChange(({ value }) => {
  if (!value) calendar$.editor.set(null)
})
ui$.calendarPanelOpen.onChange(({ value }) => {
  if (!value) calendar$.editor.set(null)
})

// A composer or reader tab opened while the calendar is showing (the rail's
// compose button, a shortcut, "Open mail") lives in the mail pane the calendar
// replaces, so make way for it.
compose$.activeTab.onChange(({ value }) => {
  if (value && ui$.calendarOpen.peek()) closeCalendar()
})

/** Whether the panel is on screen: open, not under the full view, and on a
 * window wide enough to show it (it hides below 900px, like Tasks). */
export function calendarPanelShowing(panelOpen: boolean, fullOpen: boolean, narrow: boolean): boolean {
  return panelOpen && !fullOpen && !narrow
}

/** What the calendar panel button does: show the panel (collapsing the full
 * view into it), or hide it. A window too narrow for the panel gets the full
 * view instead. */
export function toggleCalendarPanel() {
  if (isNarrowWindow()) {
    if (ui$.calendarOpen.peek()) closeCalendar()
    else openCalendar()
  } else if (calendarPanelShowing(ui$.calendarPanelOpen.peek(), ui$.calendarOpen.peek(), false)) {
    closeCalendarPanel()
  } else {
    openCalendarPanel()
  }
}

/** Re-read the month on show, and the panel's agenda where it runs past it. A
 * read that finishes after a newer one started is dropped, so paging quickly
 * lands on the month the user stopped at. */
export async function loadCalendar() {
  const seq = ++loadSeq
  // The grid CalendarView draws, week start included, or its first days go unasked.
  const days = monthGrid(new Date(calendar$.month.peek()), firstDayOfWeek())
  const day = new Date(calendar$.day.peek())
  const agenda = [day, new Date(day.getFullYear(), day.getMonth(), day.getDate() + AGENDA_DAYS - 1)]
  const grid = gridRange(days)
  const week = gridRange(agenda)
  const from = Math.min(grid.from, week.from)
  const to = Math.max(grid.to, week.to)
  calendar$.loading.set(true)
  try {
    const res = await invoke<{ events?: CalendarItem[] }>('calendar.events', {
      from,
      to,
      include_tasks: settings$.tasksEnabled.peek(),
    })
    if (seq === loadSeq) calendar$.items.set(res?.events ?? [])
  } catch (error) {
    if (seq === loadSeq) showToast(errorMessage(error, t('calendar.loadFailed')), 'error')
  } finally {
    if (seq === loadSeq) calendar$.loading.set(false)
  }
}

/** Page the month. The panel's agenda follows: to today in the current month,
 * to the first otherwise. */
export function shiftMonth(delta: number) {
  const month = new Date(calendar$.month.peek())
  const next = new Date(month.getFullYear(), month.getMonth() + delta, 1)
  const today = new Date()
  const current = next.getFullYear() === today.getFullYear() && next.getMonth() === today.getMonth()
  calendar$.month.set(next.getTime())
  calendar$.day.set(startOfDay(current ? today : next).getTime())
  void loadCalendar()
}

export function goToToday() {
  calendar$.month.set(Date.now())
  calendar$.day.set(startOfDay(new Date()).getTime())
  void loadCalendar()
}

/** Start the panel's agenda on `day`, paging the month to it when it is one of
 * the neighbouring months' days the grid shows. */
export function selectDay(day: Date) {
  calendar$.day.set(startOfDay(day).getTime())
  const month = new Date(calendar$.month.peek())
  if (day.getFullYear() !== month.getFullYear() || day.getMonth() !== month.getMonth()) {
    calendar$.month.set(new Date(day.getFullYear(), day.getMonth(), 1).getTime())
  }
  void loadCalendar()
}

export async function createEvent(input: EventInput): Promise<boolean> {
  try {
    await invoke('calendar.create', input)
  } catch (error) {
    showToast(errorMessage(error, t('calendar.saveFailed')), 'error')
    return false
  }
  await loadCalendar()
  return true
}

export async function updateEvent(eventId: string, input: EventInput): Promise<boolean> {
  try {
    await invoke('calendar.update', { event_id: eventId, ...input })
  } catch (error) {
    showToast(errorMessage(error, t('calendar.saveFailed')), 'error')
    return false
  }
  await loadCalendar()
  return true
}

/** Delete an event, with an Undo that hands the core back what it removed:
 * the calendar lives only on this device, so a delete is otherwise final. */
export async function deleteEvent(eventId: string) {
  let restore: unknown = null
  try {
    const res = await invoke<{ restore?: unknown }>('calendar.delete', { event_id: eventId })
    restore = res?.restore ?? null
  } catch (error) {
    showToast(errorMessage(error, t('calendar.saveFailed')), 'error')
    return
  }
  clearInviteEvent(eventId)
  await loadCalendar()
  if (!restore) return
  showUndoToast(t('calendar.eventDeleted'), () => {
    void invoke('calendar.restore', { restore }).then(() => loadCalendar())
  })
}

/** What pressing an item in the grid does: edit an event, open an invitation
 * in the editor (its mail is a button away), show a task in its list. */
export function openCalendarItem(item: CalendarItem) {
  if (item.source === 'task') {
    void openTasksPanel(item.list_id ?? '')
    return
  }
  calendar$.editor.set({ mode: 'edit', item })
}

export async function openEventMail(threadId: string) {
  if (!threadId) return
  // The conversation opens in the mail pane, which the full calendar replaces;
  // without a tab of its own it would not otherwise make way. The panel sits
  // beside it and stays.
  closeCalendar()
  await openThreadTabById(threadId, t('tasks.mailNotFound'))
}

/** Show `changes` on every loaded copy of the invitation — the original, an
 * update to it, the same message in another folder — and only of the same
 * occurrence: an answer about one date is not one about the series. */
function updateInvite(message: Message, changes: Partial<Invite>) {
  const invite = message.invite
  if (!invite) return
  mail$.messages.set(
    mail$.messages
      .get()
      .map((loaded) =>
        loaded.account_id === message.account_id &&
        loaded.invite?.uid === invite.uid &&
        loaded.invite.recurrence_id === invite.recurrence_id
          ? { ...loaded, invite: { ...loaded.invite, ...changes } }
          : loaded,
      ),
  )
}

/** A loaded invitation whose event was just taken out of the calendar no
 * longer offers to take it out. */
function clearInviteEvent(eventId: string) {
  mail$.messages.set(
    mail$.messages
      .get()
      .map((message) =>
        message.invite?.event_id === eventId ? { ...message, invite: { ...message.invite, event_id: '' } } : message,
      ),
  )
}

/** Take a cancelled invitation out of the calendar: the occurrence it names,
 * or the whole event when the series is cancelled. Nothing is sent. */
export async function removeInviteFromCalendar(message: Message) {
  const invite = message.invite
  const eventId = invite?.event_id
  if (!invite || !eventId) return
  try {
    await invoke('invite.removeFromCalendar', { message: message.id })
  } catch (error) {
    showToast(errorMessage(error, t('calendar.saveFailed')), 'error')
    return
  }
  if (invite.recurrence_id === 0) {
    clearInviteEvent(eventId)
  } else {
    // Only this occurrence went: other cancelled dates of the series can
    // still be taken out.
    updateInvite(message, { event_id: '' })
  }
  if (calendarShowing()) void loadCalendar()
}

/** Put an answered invitation back in the calendar — after its event was
 * deleted — with the answer already given and nothing sent. */
export async function addInviteToCalendar(message: Message) {
  const invite = message.invite
  if (!invite) return
  let eventId = ''
  try {
    const res = await invoke<{ event_id?: string }>('invite.addToCalendar', {
      message: message.id,
      thread_id: message.thread_id,
    })
    eventId = res?.event_id ?? ''
  } catch (error) {
    showToast(errorMessage(error, t('calendar.saveFailed')), 'error')
    return
  }
  updateInvite(message, { event_id: eventId })
  if (calendarShowing()) void loadCalendar()
}

/** Answer an invitation: the core sends the organizer an iMIP reply, records
 * the answer and files the event in the calendar. The card shows it straight
 * away and goes back if the send fails. */
export async function respondToInvite(message: Message, response: InviteResponse) {
  const invite = message.invite
  if (!invite) return
  const previous = invite.response
  updateInvite(message, { response })
  try {
    const res = await invoke<{ event_id?: string }>('invite.respond', {
      message: message.id,
      thread_id: message.thread_id,
      response,
    })
    // Filed in the calendar now, or (declined) taken out of it. A declined
    // series takes its event out from under every message that pointed at it;
    // a declined occurrence leaves the series, and the other dates, in place.
    const eventId = res?.event_id ?? ''
    if (!eventId && invite.event_id && invite.recurrence_id === 0) clearInviteEvent(invite.event_id)
    updateInvite(message, { event_id: eventId })
  } catch (error) {
    updateInvite(message, { response: previous })
    showToast(errorMessage(error, t('invite.respondFailed')), 'error')
    return
  }
  if (calendarShowing()) void loadCalendar()
}
