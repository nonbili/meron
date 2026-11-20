import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { ui$ } from '../../states/ui'
import { CalendarAccountForm } from './CalendarAccountsPanel'
import { Dialog } from './Dialog'

export function CaldavSetupDialog() {
  const setup = useValue(ui$.caldavSetup)
  const { t } = useTranslation()
  if (!setup) return null

  const close = () => {
    if (ui$.caldavSetup.peek() === setup) ui$.caldavSetup.set(null)
  }

  return (
    <Dialog
      onClose={close}
      closeOnBackdrop={false}
      aria-labelledby="caldav-setup-title"
      aria-describedby="caldav-setup-description"
      className="max-w-md p-6"
    >
      <h2 id="caldav-setup-title" className="text-base font-bold">
        {t('accounts.setup.connectCalendar')}
      </h2>
      <p id="caldav-setup-description" className="mt-2 text-sm leading-relaxed text-secondary">
        {t('accounts.setup.caldavFound')}
      </p>
      <CalendarAccountForm key={`${setup.email}:${setup.server}`} initial={setup} optional onDone={close} />
    </Dialog>
  )
}
