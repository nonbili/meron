import { useValue } from '@legendapp/state/react'
import { RefreshCw } from 'lucide-react'
import { useEffect } from 'react'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { calendar$ } from '../../states/calendar'
import { caldav$, loadCalendarAccounts, syncAllCalendars } from '../../states/caldav'
import { IconButton } from '../button/IconButton'

/** Sync the calendar accounts now. Spins while they sync; a dot says the
 * last sync of one failed, and its label why (Settings has every account's
 * status). Shown once there is a calendar to sync. */
export function CalendarSyncButton() {
  const { t } = useTranslation()
  const calendars = useValue(calendar$.calendars)
  const accounts = useValue(caldav$.accounts)
  const syncing = useValue(caldav$.syncing)

  useEffect(() => {
    void loadCalendarAccounts()
    const eventsOn = (window as any).runtime?.EventsOn
    const off = typeof eventsOn === 'function' ? eventsOn('caldav.changed', () => void loadCalendarAccounts()) : null
    return () => {
      if (typeof off === 'function') off()
    }
  }, [])

  if (calendars.length === 0) return null
  const failed = accounts.find((account) => account.last_error)
  const syncNow = t('settings.calendars.syncNow')
  const label = failed
    ? `${syncNow} · ${failed.name}: ${t('settings.calendars.syncFailed', { error: failed.last_error })}`
    : syncNow

  return (
    <span className="relative inline-flex">
      <IconButton
        icon={RefreshCw}
        size="sm"
        label={label}
        disabled={syncing}
        className={clsx(syncing && '[&_svg]:animate-spin')}
        onClick={() => void syncAllCalendars()}
      />
      {failed && !syncing && (
        <span className="pointer-events-none absolute right-1 top-1 h-1.5 w-1.5 rounded-full bg-rose-500" aria-hidden />
      )}
    </span>
  )
}
