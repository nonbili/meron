import { CalendarDays, Check, Copy, Mail, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { dateValue, localInstant, parseDateValue, timeValue, utcDate, utcDateValue } from '../../lib/calendarGrid'
import { formatEventRange } from '../../lib/date'
import { Dialog } from '../dialog/Dialog'
import { NO_ALERT, alertLabel, alertOptions } from '../../lib/eventAlerts'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { copyText, openExternal } from '../../lib/native'
import {
  calendar$,
  calendarColor,
  calendarOf,
  createEvent,
  deleteEvent,
  openEventMail,
  updateEvent,
  type CalendarItem,
  type CalendarRepeat,
  type EventEditor,
  type EventInput,
  type SyncedCalendar,
} from '../../states/calendar'
import { settings$ } from '../../states/settings'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { Checkbox } from '../field/Checkbox'
import { LinkifiedText } from '../field/LinkifiedText'
import { SelectInput, TextInput } from '../field/Field'

const CUSTOM_REPEAT: { value: CalendarRepeat; label: string } = { value: 'custom', label: 'calendar.repeat.custom' }

/** A calendar as the picker names it: with its account, since two accounts
 * can each have a "Personal". */
function calendarLabel(calendar: SyncedCalendar, note: string): string {
  const name = calendar.account_name ? `${calendar.name} (${calendar.account_name})` : calendar.name
  return note ? `${name} · ${note}` : name
}

const REPEATS: { value: CalendarRepeat; label: string }[] = [
  { value: '', label: 'calendar.repeat.none' },
  { value: 'daily', label: 'calendar.repeat.daily' },
  { value: 'weekly', label: 'calendar.repeat.weekly' },
  { value: 'monthly', label: 'calendar.repeat.monthly' },
  { value: 'yearly', label: 'calendar.repeat.yearly' },
]

type Form = {
  title: string
  notes: string
  location: string
  allDay: boolean
  startDate: string
  startTime: string
  endDate: string
  endTime: string
  repeat: CalendarRepeat
  /** 'default' follows the app-wide alert; NO_ALERT turns alerts off. */
  alert: 'default' | number
  /** A second lead, only alongside a first one; NO_ALERT for none. */
  secondAlert: number
  /** The synced calendar it is (or goes) in; empty for this device. */
  calendar: string
}

/** The form's alert fields for an event's stored alerts. */
function alertFields(alerts: number[] | null | undefined): Pick<Form, 'alert' | 'secondAlert'> {
  if (alerts == null) return { alert: 'default', secondAlert: NO_ALERT }
  return { alert: alerts[0] ?? NO_ALERT, secondAlert: alerts[1] ?? NO_ALERT }
}

/** What the core stores for the form's alerts. */
function formAlerts(form: Form): number[] | null {
  if (form.alert === 'default') return null
  if (form.alert === NO_ALERT) return []
  return form.secondAlert === NO_ALERT ? [form.alert] : [form.alert, form.secondAlert]
}

/** The editor's fields for what it was opened on. A new event starts at 09:00
 * for an hour on the chosen day. An all-day item's end is exclusive in the
 * core and inclusive here, where a date input names the last day. */
function initialForm(editor: EventEditor): Form {
  if (editor.mode === 'new') {
    const day = dateValue(new Date(editor.day))
    // The calendar the last event went in, while it can still take one.
    const last = calendar$.lastCalendar.peek()
    const calendar = calendar$.calendars.peek().some((c) => c.id === last && c.writable) ? last : ''
    return {
      title: '',
      notes: '',
      location: '',
      allDay: false,
      startDate: day,
      startTime: '09:00',
      endDate: day,
      endTime: '10:00',
      repeat: '',
      ...alertFields(null),
      calendar,
    }
  }
  const { item } = editor
  const start = new Date(item.start * 1000)
  const end = new Date(Math.max(item.end, item.start) * 1000)
  return {
    title: item.title,
    notes: item.notes,
    location: item.location,
    allDay: item.all_day,
    startDate: item.all_day ? utcDateValue(item.start) : dateValue(start),
    startTime: timeValue(start),
    endDate: item.all_day ? utcDateValue(Math.max(item.start, item.end - 86_400)) : dateValue(end),
    endTime: timeValue(end),
    // A synced event repeating by a rule the choices cannot say.
    repeat: hasCustomRepeat(item) ? 'custom' : item.repeat,
    ...alertFields(item.alerts),
    calendar: item.calendar ?? '',
  }
}

function hasCustomRepeat(item: CalendarItem): boolean {
  return !!item.calendar && item.recurring && !item.repeat
}

function toInput(form: Form): EventInput {
  if (form.allDay) {
    const start = utcDate(form.startDate)
    return {
      title: form.title,
      notes: form.notes,
      location: form.location,
      all_day: true,
      start,
      end: Math.max(start, utcDate(form.endDate)) + 86_400,
      repeat: form.repeat,
      alerts: formAlerts(form),
      calendar: form.calendar,
    }
  }
  const start = localInstant(form.startDate, form.startTime)
  return {
    title: form.title,
    notes: form.notes,
    location: form.location,
    all_day: false,
    start,
    end: Math.max(start, localInstant(form.endDate, form.endTime)),
    repeat: form.repeat,
    alerts: formAlerts(form),
    calendar: form.calendar,
  }
}

// Apple Calendar's "Alert" and "Second Alert": minutes before a timed event,
// days before an all-day one (at 09:00).
function AlertFields({
  form,
  allDay,
  set,
}: {
  form: Form
  /** The kind the alerts count in. */
  allDay: boolean
  set: <K extends keyof Form>(key: K, value: Form[K]) => void
}) {
  const { t } = useTranslation()
  const defaultMinutes = useValue(settings$.reminderLeadMinutes)
  const defaultDays = useValue(settings$.reminderAllDayDays)
  const label = 'text-[0.6875rem] font-semibold text-secondary'
  const defaultLabel = t('calendar.alert.default', {
    value: alertLabel(allDay ? defaultDays : defaultMinutes, allDay),
  })
  const first = form.alert
  const options = alertOptions(allDay)
  return (
    <div className="grid grid-cols-[auto_1fr] items-center gap-x-2 gap-y-2">
      <span className={label}>{t('calendar.alert.label')}</span>
      <SelectInput
        aria-label={t('calendar.alert.label')}
        value={String(first)}
        onChange={(event) => {
          const value = event.target.value
          set('alert', value === 'default' ? 'default' : Number(value))
        }}
      >
        <option value="default">{defaultLabel}</option>
        <option value={NO_ALERT}>{alertLabel(NO_ALERT)}</option>
        {options.map((lead) => (
          <option key={lead} value={lead}>
            {alertLabel(lead, allDay)}
          </option>
        ))}
      </SelectInput>
      {typeof first === 'number' && first !== NO_ALERT && (
        <>
          <span className={label}>{t('calendar.alert.second')}</span>
          <SelectInput
            aria-label={t('calendar.alert.second')}
            value={String(form.secondAlert)}
            onChange={(event) => set('secondAlert', Number(event.target.value))}
          >
            {[NO_ALERT, ...options.filter((lead) => lead !== first)].map((lead) => (
              <option key={lead} value={lead}>
                {alertLabel(lead, allDay)}
              </option>
            ))}
          </SelectInput>
        </>
      )}
    </div>
  )
}

// An invitation's location is often its meeting link: open it, or copy it.
function InviteLocation({ location }: { location: string }) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(false)
  if (!/^https?:\/\/\S+$/.test(location.trim())) return <span>{location}</span>
  const url = location.trim()
  const copy = async () => {
    await copyText(url).catch(() => undefined)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }
  return (
    <span className="flex min-w-0 items-center gap-1">
      <a
        href={url}
        className="min-w-0 truncate text-accent hover:underline"
        title={url}
        onClick={(event) => {
          event.preventDefault()
          openExternal(url)
        }}
      >
        {url}
      </a>
      <IconButton
        icon={copied ? Check : Copy}
        iconSize={12}
        label={copied ? t('common.copied') : t('chat.actions.copyLinkAddress')}
        size="sm"
        onClick={() => void copy()}
      />
    </span>
  )
}

// Notes read with their links clickable, as an invitation's usually carry a
// meeting link; a click elsewhere in them, or Enter, edits them. Empty notes
// start as the textarea.
function NotesField({
  notes,
  className,
  onChange,
}: {
  notes: string
  className: string
  onChange: (notes: string) => void
}) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(() => !notes.trim())
  const label = t('calendar.fields.notes')

  if (editing) {
    return (
      <textarea
        className={`${className} resize-y placeholder-secondary`}
        placeholder={label}
        aria-label={label}
        value={notes}
        // Mounted with notes only when the read view was clicked.
        autoFocus={!!notes.trim()}
        onChange={(event) => onChange(event.target.value)}
        onBlur={() => setEditing(!notes.trim())}
      />
    )
  }
  return (
    <div
      role="button"
      tabIndex={0}
      aria-label={label}
      className={`${className} max-h-80 cursor-text overflow-y-auto whitespace-pre-wrap break-words`}
      onClick={() => {
        // Selecting text to copy it is not a request to edit.
        if (window.getSelection()?.isCollapsed !== false) setEditing(true)
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter' && event.target === event.currentTarget) {
          event.preventDefault()
          setEditing(true)
        }
      }}
    >
      <LinkifiedText text={notes} />
    </div>
  )
}

// Create or change an event made here. An accepted invitation opens read-only
// but for its notes and alerts: its time and title are the organizer's to change. Mirrors
// mobile's EventEditorSheet.
export function EventEditorDialog({ editor }: { editor: EventEditor }) {
  const { t } = useTranslation()
  const [form, setForm] = useState(() => initialForm(editor))
  // What the fields held when the editor opened, so the core can tell an
  // edit from a value a sync has since replaced.
  const [original] = useState(() => toInput(initialForm(editor)))
  const [saving, setSaving] = useState(false)
  const item = editor.mode === 'edit' ? editor.item : null
  const invite = item?.source === 'invite'
  const calendars = useValue(calendar$.calendars)
  const synced = item ? calendarOf(item, calendars) : undefined
  // A subscription or a calendar shared read-only: shown, not changed.
  const readOnly = !!synced && !synced.writable
  const writableCalendars = calendars.filter((calendar) => calendar.writable)
  const pickedCalendar = calendars.find((calendar) => calendar.id === form.calendar)
  const repeats = item && hasCustomRepeat(item) ? [...REPEATS, CUSTOM_REPEAT] : REPEATS
  const close = () => calendar$.editor.set(null)

  const set = <K extends keyof Form>(key: K, value: Form[K]) => setForm((current) => ({ ...current, [key]: value }))
  // Moving the start moves the end with it, keeping the event's length.
  const moveStart = (startDate: string, startTime: string) =>
    setForm((current) => {
      // All-day bounds are dates: count whole days, never local hours, which
      // a DST change between the two would make 23 or 25.
      if (current.allDay) {
        const days = (utcDate(startDate) - utcDate(current.startDate)) / 86_400
        if (!utcDate(startDate) || !utcDate(current.endDate)) return { ...current, startDate }
        return { ...current, startDate, endDate: utcDateValue(utcDate(current.endDate) + days * 86_400) }
      }
      const before = localInstant(current.startDate, current.startTime)
      const after = localInstant(startDate, startTime)
      if (!before || !after) return { ...current, startDate, startTime }
      const end = new Date((localInstant(current.endDate, current.endTime) + after - before) * 1000)
      return { ...current, startDate, startTime, endDate: dateValue(end), endTime: timeValue(end) }
    })
  const canSave = !!parseDateValue(form.startDate) && !saving && !readOnly

  const save = async () => {
    if (!canSave) return
    setSaving(true)
    const input = toInput(form)
    const saved = item
      ? await updateEvent(item.id, {
          ...input,
          occurrence_start: item.recurring ? item.start : undefined,
          original: { ...original, occurrence_start: item.recurring ? item.start : undefined },
        })
      : await createEvent(input)
    setSaving(false)
    if (saved) close()
  }

  const remove = async () => {
    if (!item) return
    close()
    await deleteEvent(item.id)
  }

  const label = 'text-[0.6875rem] font-semibold text-secondary'

  return (
    <Dialog onClose={close} dismissible={!saving} aria-labelledby="event-editor-title" className="max-w-md p-6">
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          void save()
        }}
      >
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <CalendarDays size={17} />
          </div>
          <h2 id="event-editor-title" className="min-w-0 flex-1 truncate text-[0.9375rem] font-bold tracking-tight">
            {item ? (invite ? item.title || t('invite.untitled') : t('calendar.editEvent')) : t('calendar.newEvent')}
          </h2>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={close} />
        </div>

        {invite && item ? (
          <div className="flex flex-col gap-1 text-xs text-secondary select-text">
            <span>
              {formatEventRange(item.start, item.end, item.all_day)}
              {item.all_day && ` · ${t('invite.allDay')}`}
            </span>
            {item.location && <InviteLocation location={item.location} />}
            {item.response === 'tentative' && <span>{t('invite.youTentative')}</span>}
          </div>
        ) : (
          <>
            <TextInput
              fieldSize="md"
              autoFocus
              placeholder={t('calendar.fields.title')}
              aria-label={t('calendar.fields.title')}
              value={form.title}
              onChange={(event) => set('title', event.target.value)}
            />
            <label className="flex items-center gap-2 text-xs font-medium">
              <Checkbox
                checked={form.allDay}
                // Leads mean minutes for a timed event and days for an all-day
                // one, so the switch starts the alerts over from the default.
                onChange={(event) =>
                  setForm((current) => ({ ...current, allDay: event.target.checked, ...alertFields(null) }))
                }
              />
              {t('invite.allDay')}
            </label>
            <div className="grid grid-cols-[auto_1fr_auto] items-center gap-x-2 gap-y-2">
              <span className={label}>{t('calendar.fields.starts')}</span>
              <TextInput
                type="date"
                aria-label={t('calendar.fields.starts')}
                value={form.startDate}
                onChange={(event) => moveStart(event.target.value, form.startTime)}
              />
              {form.allDay ? (
                <span />
              ) : (
                <TextInput
                  type="time"
                  aria-label={t('calendar.fields.startTime')}
                  value={form.startTime}
                  onChange={(event) => moveStart(form.startDate, event.target.value)}
                />
              )}
              <span className={label}>{t('calendar.fields.ends')}</span>
              <TextInput
                type="date"
                aria-label={t('calendar.fields.ends')}
                value={form.endDate}
                min={form.startDate}
                onChange={(event) => set('endDate', event.target.value)}
              />
              {form.allDay ? (
                <span />
              ) : (
                <TextInput
                  type="time"
                  aria-label={t('calendar.fields.endTime')}
                  value={form.endTime}
                  onChange={(event) => set('endTime', event.target.value)}
                />
              )}
              <span className={label}>{t('calendar.fields.repeat')}</span>
              <SelectInput
                wrapperClassName="col-span-2"
                aria-label={t('calendar.fields.repeat')}
                value={form.repeat}
                onChange={(event) => set('repeat', event.target.value as CalendarRepeat)}
              >
                {repeats.map(({ value, label: key }) => (
                  <option key={value} value={value}>
                    {t(key)}
                  </option>
                ))}
              </SelectInput>
              {(writableCalendars.length > 0 || !!synced) && (
                <>
                  <span className={label}>{t('calendar.fields.calendar')}</span>
                  <div className="col-span-2 flex min-w-0 items-center gap-2">
                    {/* The chosen calendar's colour, as its events show: the
                      picker's own options cannot carry one. */}
                    <span
                      className={clsx('h-2.5 w-2.5 shrink-0 rounded-full', !pickedCalendar && 'bg-accent')}
                      style={pickedCalendar ? { backgroundColor: calendarColor(pickedCalendar) } : undefined}
                      aria-hidden
                    />
                    <SelectInput
                      wrapperClassName="min-w-0 flex-1"
                      aria-label={t('calendar.fields.calendar')}
                      value={form.calendar}
                      // Choosing another calendar moves the event there on save.
                      disabled={readOnly}
                      onChange={(event) => set('calendar', event.target.value)}
                    >
                      {readOnly && synced ? (
                        <option value={synced.id}>{calendarLabel(synced, t('calendar.readOnly'))}</option>
                      ) : (
                        <>
                          {/* A server's own repeat rule cannot live on this device. */}
                          {form.repeat !== 'custom' && <option value="">{t('calendar.onThisDevice')}</option>}
                          {writableCalendars.map((calendar) => (
                            <option key={calendar.id} value={calendar.id}>
                              {calendarLabel(calendar, '')}
                            </option>
                          ))}
                        </>
                      )}
                    </SelectInput>
                  </div>
                </>
              )}
            </div>
            <TextInput
              placeholder={t('calendar.fields.location')}
              aria-label={t('calendar.fields.location')}
              value={form.location}
              onChange={(event) => set('location', event.target.value)}
            />
          </>
        )}

        <AlertFields
          form={form}
          // An invitation's alerts belong to its series, whose kind one
          // occurrence can differ from.
          allDay={invite ? (item.alerts_all_day ?? item.all_day) : form.allDay}
          set={set}
        />

        <NotesField
          notes={form.notes}
          className={`${invite ? 'min-h-48' : 'min-h-20'} rounded-lg border border-border bg-chats px-2.5 py-1.5 text-xs text-primary outline-none transition-colors focus:border-accent select-text`}
          onChange={(notes) => set('notes', notes)}
        />

        <div className="flex items-center gap-2">
          {item && !readOnly && (
            <IconButton
              icon={Trash2}
              label={invite ? t('calendar.removeFromCalendar') : t('buttons.delete')}
              size="md"
              onClick={() => void remove()}
            />
          )}
          {item?.thread_id && (
            <Button variant="ghost" size="sm" leftIcon={Mail} onClick={() => void openEventMail(item.thread_id)}>
              {t('calendar.openMail')}
            </Button>
          )}
          <div className="flex-1" />
          <Button variant="secondary" size="sm" onClick={close}>
            {t('buttons.cancel')}
          </Button>
          <Button type="submit" size="sm" disabled={!canSave}>
            {t('buttons.save')}
          </Button>
        </div>
      </form>
    </Dialog>
  )
}
