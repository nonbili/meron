import { useValue } from '@legendapp/state/react'
import { Layers } from 'lucide-react'
import { useId, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from '../../lib/i18n'
import {
  calendar$,
  calendarColor,
  LOCAL_CALENDAR,
  toggleCalendarShown,
  type SyncedCalendar,
} from '../../states/calendar'
import { IconButton } from '../button/IconButton'
import { Checkbox } from '../field/Checkbox'
import { CalendarColorEditor } from './CalendarColorEditor'
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { clsx } from '../../lib/utils'
import { Avatar } from '../avatar/Avatar'

/** Which calendars the views draw: this device's events, then each synced
 * calendar under its account, in its colour. Hiding one only leaves it out
 * of the views; it keeps syncing and its alerts still fire. Shown once
 * there is a synced calendar to tell apart from this device's. */
export function CalendarFilter({ className }: { className?: string }) {
  const { t } = useTranslation()
  const calendars = useValue(calendar$.calendars)
  const hidden = useValue(calendar$.hiddenCalendars)
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useDismissOnOutside(
    open,
    (target) => Boolean(ref.current?.contains(target as Node)),
    () => setOpen(false),
  )
  if (calendars.length === 0) return null
  const label = t('calendar.chooseCalendars')

  return (
    <div ref={ref} className={clsx('relative', className)}>
      <IconButton
        icon={Layers}
        size="sm"
        label={label}
        // Something is hidden: say so, or a missing event looks lost.
        active={open || hidden.length > 0}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      />
      {open && (
        <MenuSurface
          role="dialog"
          aria-label={label}
          className="absolute right-0 top-full z-50 mt-1 flex max-h-96 w-64 flex-col overflow-y-auto p-1.5"
        >
          <CalendarList />
        </MenuSurface>
      )}
    </div>
  )
}

/** This device's events, then each account's calendars under its name, each
 * with its colour and whether it is shown. */
export function CalendarList() {
  const { t } = useTranslation()
  const calendars = useValue(calendar$.calendars)
  const hidden = useValue(calendar$.hiddenCalendars)
  const accounts = new Map<string, { name: string; avatar?: string; google: boolean; calendars: SyncedCalendar[] }>()
  for (const calendar of calendars) {
    const account = accounts.get(calendar.account_id) ?? {
      name: calendar.account_name,
      avatar: calendar.account_avatar_url,
      google: !!calendar.account_google,
      calendars: [],
    }
    account.calendars.push(calendar)
    accounts.set(calendar.account_id, account)
  }
  return (
    <div className="flex flex-col gap-0.5">
      <CalendarRow
        name={t('calendar.onThisDevice')}
        colorClass="bg-accent"
        shown={!hidden.includes(LOCAL_CALENDAR)}
        onToggle={() => toggleCalendarShown(LOCAL_CALENDAR)}
      />
      {[...accounts.entries()].map(([id, account]) => (
        <div key={id} className="flex flex-col gap-0.5">
          <span className="mt-1.5 flex min-w-0 items-center gap-1.5 px-2 text-[0.6875rem] font-semibold text-secondary">
            {account.google && (
              <span aria-hidden className="shrink-0">
                <Avatar name={account.name} src={account.avatar} size={16} />
              </span>
            )}
            <span className="truncate">{account.name}</span>
          </span>
          {account.calendars.map((calendar) => (
            <CalendarRow
              key={calendar.id}
              name={calendar.name}
              color={calendarColor(calendar)}
              calendarId={calendar.id}
              shown={!hidden.includes(calendar.id)}
              onToggle={() => toggleCalendarShown(calendar.id)}
            />
          ))}
        </div>
      ))}
    </div>
  )
}

function CalendarRow({
  name,
  color,
  colorClass,
  calendarId,
  shown,
  onToggle,
}: {
  name: string
  color?: string
  colorClass?: string
  /** A synced calendar, whose colour can be changed from here. */
  calendarId?: string
  shown: boolean
  onToggle: () => void
}) {
  const id = useId()
  const row = (dot: ReactNode) => (
    <div className="flex min-w-0 items-center gap-1 rounded-lg ps-2 text-xs text-primary hover:bg-hover">
      <Checkbox id={id} checked={shown} onChange={onToggle} />
      {dot}
      <label htmlFor={id} className="min-w-0 flex-1 cursor-pointer truncate py-1.5 pe-2" title={name}>
        {name}
      </label>
    </div>
  )
  if (calendarId && color) {
    return (
      <CalendarColorEditor calendarId={calendarId} color={color}>
        {row}
      </CalendarColorEditor>
    )
  }
  return row(
    <span className="flex size-5 shrink-0 items-center justify-center">
      <span
        aria-hidden
        className={`size-2.5 rounded-full ${colorClass ?? ''}`}
        style={color ? { backgroundColor: color } : undefined}
      />
    </span>,
  )
}
