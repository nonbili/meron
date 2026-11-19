import { useValue } from '@legendapp/state/react'
import { Layers } from 'lucide-react'
import { useRef, useState } from 'react'
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
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { clsx } from '../../lib/utils'

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
  const accounts = new Map<string, { name: string; calendars: SyncedCalendar[] }>()
  for (const calendar of calendars) {
    const account = accounts.get(calendar.account_id) ?? { name: calendar.account_name, calendars: [] }
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
          <span className="mt-1.5 truncate px-2 text-[0.6875rem] font-semibold text-secondary">{account.name}</span>
          {account.calendars.map((calendar) => (
            <CalendarRow
              key={calendar.id}
              name={calendar.name}
              color={calendarColor(calendar)}
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
  shown,
  onToggle,
}: {
  name: string
  color?: string
  colorClass?: string
  shown: boolean
  onToggle: () => void
}) {
  return (
    <label className="flex min-w-0 items-center gap-2 rounded-lg px-2 py-1.5 text-xs text-primary cursor-pointer hover:bg-hover">
      <Checkbox checked={shown} onChange={onToggle} />
      <span
        className={`h-2.5 w-2.5 shrink-0 rounded-full ${colorClass ?? ''}`}
        style={color ? { backgroundColor: color } : undefined}
        aria-hidden
      />
      <span className="truncate" title={name}>
        {name}
      </span>
    </label>
  )
}
