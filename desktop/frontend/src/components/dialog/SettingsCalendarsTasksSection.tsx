import { useTranslation } from '../../lib/i18n'
import { CalendarDays, SquareCheckBig, Bell } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { settings$, setCalendarEnabled, setTasksEnabled } from '../../states/settings'
import { SettingsGroup, ToggleRow, SelectRow } from './AccountSettingsRows'
import { CalendarAccountsPanel } from './CalendarAccountsPanel'
import { ALERT_MINUTES, ALL_DAY_ALERT_DAYS, NO_ALERT, alertLabel } from '../../lib/eventAlerts'

// Calendar and Tasks: whether they are on, their defaults, and the accounts
// they sync with (CalDAV calendars and task lists, Google Tasks).
export function SettingsCalendarsTasksSection() {
  const { t } = useTranslation()
  const tasksEnabled = useValue(settings$.tasksEnabled)
  const calendarEnabled = useValue(settings$.calendarEnabled)
  const reminderLeadMinutes = useValue(settings$.reminderLeadMinutes)
  const reminderAllDayDays = useValue(settings$.reminderAllDayDays)

  return (
    <div className="flex flex-col gap-4">
      <SettingsGroup title={t('settings.sections.general')}>
        <ToggleRow
          icon={<CalendarDays size={15} />}
          title={t('settings.calendar.enable')}
          hint={t('settings.calendar.enableHint')}
          checked={calendarEnabled}
          onChange={() => setCalendarEnabled(!calendarEnabled)}
        />
        <ToggleRow
          icon={<SquareCheckBig size={15} />}
          title={t('settings.tasks.enable')}
          hint={t('settings.tasks.enableHint')}
          checked={tasksEnabled}
          onChange={() => setTasksEnabled(!tasksEnabled)}
        />
        {calendarEnabled && (
          <SelectRow
            icon={<Bell size={15} />}
            title={t('settings.calendar.reminderLead')}
            hint={t('settings.calendar.reminderLeadHint')}
            value={String(reminderLeadMinutes)}
            options={[NO_ALERT, ...ALERT_MINUTES].map((minutes) => ({
              value: String(minutes),
              label: alertLabel(minutes),
            }))}
            onChange={(value) => settings$.reminderLeadMinutes.set(Number(value))}
          />
        )}
        {calendarEnabled && (
          <SelectRow
            icon={<Bell size={15} />}
            title={t('settings.calendar.allDayAlert')}
            hint={t('settings.calendar.allDayAlertHint')}
            value={String(reminderAllDayDays)}
            options={[NO_ALERT, ...ALL_DAY_ALERT_DAYS].map((days) => ({
              value: String(days),
              label: alertLabel(days, true),
            }))}
            onChange={(value) => settings$.reminderAllDayDays.set(Number(value))}
          />
        )}
      </SettingsGroup>

      {(calendarEnabled || tasksEnabled) && (
        <section data-setting-title={t('settings.calendars.accounts')} className="mt-2">
          <h3 className="mb-2 px-1 text-[0.75rem] font-semibold text-secondary">{t('settings.calendars.accounts')}</h3>
          <CalendarAccountsPanel />
        </section>
      )}
    </div>
  )
}
