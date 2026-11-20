import { useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { errorMessage } from '../../lib/errors'
import { useTranslation } from '../../lib/i18n'
import { connectGoogleCalendar } from '../../states/caldav'
import { ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { Dialog } from './Dialog'

export function GoogleCalendarSetupDialog() {
  const email = useValue(ui$.googleCalendarSetupEmail)
  return email ? <GoogleCalendarSetupPrompt key={email} email={email} /> : null
}

function GoogleCalendarSetupPrompt({ email }: { email: string }) {
  const { t } = useTranslation()
  const [connecting, setConnecting] = useState(false)
  const [error, setError] = useState('')

  const close = () => {
    if (ui$.googleCalendarSetupEmail.peek() === email) ui$.googleCalendarSetupEmail.set('')
  }

  const connect = async () => {
    setConnecting(true)
    setError('')
    try {
      await connectGoogleCalendar({ email })
      close()
    } catch (err) {
      setError(errorMessage(err, t('settings.calendars.addFailed')))
    } finally {
      setConnecting(false)
    }
  }

  return (
    <Dialog
      onClose={close}
      closeOnBackdrop={false}
      aria-labelledby="google-calendar-setup-title"
      aria-describedby="google-calendar-setup-description"
      className="max-w-md p-6"
    >
      <h2 id="google-calendar-setup-title" className="text-base font-bold">
        {t('settings.calendars.connectGoogle')}
      </h2>
      <p className="mt-1 break-all text-xs text-secondary">{email}</p>
      <p id="google-calendar-setup-description" className="mt-4 text-sm leading-relaxed text-secondary">
        {t('accounts.setup.googleCalendarAccess')}
      </p>
      {connecting && (
        <p role="status" className="mt-3 text-xs text-secondary">
          {t('accounts.oauth.waitingForSignIn', { provider: 'Google' })}
        </p>
      )}
      {error && (
        <p role="alert" className="mt-3 text-xs text-rose-600 dark:text-rose-400">
          {error}
        </p>
      )}
      <div className="mt-5 flex flex-wrap justify-end gap-2">
        <Button variant="ghost" onClick={close}>
          {connecting ? t('buttons.close') : t('buttons.skip')}
        </Button>
        <Button onClick={() => void connect()} disabled={connecting}>
          {t('settings.calendars.connectGoogle')}
        </Button>
      </div>
    </Dialog>
  )
}
