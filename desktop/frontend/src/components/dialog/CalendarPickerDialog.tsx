import { useId, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { caldav$, setCalendarEnabled, syncCalendarAccount } from '../../states/caldav'
import { calendarColor } from '../../states/calendar'
import { Button } from '../button/Button'
import { Avatar } from '../avatar/Avatar'
import { Checkbox } from '../field/Checkbox'
import { Dialog } from './Dialog'

export function CalendarPickerDialog({ accountId, onClose }: { accountId: string; onClose: () => void }) {
  const { t } = useTranslation()
  const titleId = useId()
  const account = useValue(caldav$.accounts).find(({ id }) => id === accountId)
  const name = account?.name ?? ''
  const [refreshing, setRefreshing] = useState(false)
  const busy = refreshing
  const calendars = account?.calendars.filter((calendar) => calendar.events) ?? []

  return (
    <Dialog aria-labelledby={titleId} className="max-w-md p-6" onClose={onClose} dismissible={!busy}>
      <h2 id={titleId} className="text-base font-bold">
        {t('settings.calendars.chooseToSync')}
      </h2>
      {account && (
        <div className="mt-3 flex items-center gap-2.5">
          <span aria-hidden className="shrink-0">
            <Avatar name={name} src={account.avatar_url} size={32} />
          </span>
          <div className="min-w-0 break-words text-xs">
            <p className="font-semibold">{name}</p>
            {name !== account.username && <p className="text-secondary">{account.username}</p>}
          </div>
        </div>
      )}
      {(account?.discovery_warning || account?.last_error) && (
        <p role="status" className="mt-3 text-xs text-secondary">
          {account.discovery_warning || t('settings.calendars.syncFailed', { error: account.last_error })}
        </p>
      )}
      {calendars.length ? (
        <div className="mt-4 max-h-[50vh] divide-y divide-border/40 overflow-y-auto rounded-xl border border-border/60">
          {calendars.map((calendar) => (
            <label key={calendar.id} className="flex min-h-11 cursor-pointer items-center gap-3 px-3 py-2 text-sm">
              <Checkbox
                checked={calendar.enabled}
                disabled={busy}
                onChange={(event) => void setCalendarEnabled(calendar.id, event.target.checked)}
              />
              <span
                aria-hidden
                className="size-3 shrink-0 rounded-full"
                style={{ background: calendarColor(calendar) }}
              />
              <span className="min-w-0 break-words">
                {calendar.name}
                {calendar.read_only && <span className="text-secondary"> · {t('calendar.readOnly')}</span>}
              </span>
            </label>
          ))}
        </div>
      ) : (
        <p className="mt-4 text-sm text-secondary">{t('settings.calendars.noEventCalendars')}</p>
      )}
      <div className="mt-5 flex flex-wrap justify-end gap-2">
        {account?.google && (
          <Button
            variant="secondary"
            disabled={busy}
            onClick={async () => {
              setRefreshing(true)
              try {
                await syncCalendarAccount(accountId)
              } finally {
                setRefreshing(false)
              }
            }}
          >
            {t(refreshing ? 'common.loading' : 'settings.calendars.syncNow')}
          </Button>
        )}
        <Button onClick={onClose} disabled={busy}>
          {t('buttons.done')}
        </Button>
      </div>
    </Dialog>
  )
}
