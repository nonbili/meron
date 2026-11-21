import { useEffect, useRef, useState } from 'react'
import { RefreshCw, Trash2 } from 'lucide-react'

import { invoke } from '../../lib/bridge'
import { formatThreadDate } from '../../lib/date'
import { errorMessage } from '../../lib/errors'
import { useTranslation } from '../../lib/i18n'
import { confirmAction, showToast } from '../../states/ui'
import { Button } from '../button/Button'
import { TextInput } from '../field/Field'

type GoogleTasksAccount = {
  email: string
  last_error: string
  last_sync_at: number
}

type AccountsAnswer = { accounts?: GoogleTasksAccount[]; unsent?: number }

const provider = 'Google Tasks'

export function GoogleTasksAccounts({ canConnect }: { canConnect: boolean }) {
  const { t } = useTranslation()
  const [accounts, setAccounts] = useState<GoogleTasksAccount[]>([])
  const [email, setEmail] = useState('')
  const [busy, setBusy] = useState(false)
  const readVersion = useRef(0)

  useEffect(() => {
    let active = true
    const load = async () => {
      const version = ++readVersion.current
      const result = await invoke<AccountsAnswer>('googleTasks.accounts')
      if (active && version === readVersion.current && result?.accounts) setAccounts(result.accounts)
    }
    const refresh = () => void load().catch(() => {})
    refresh()
    const stop = (window as any).runtime?.EventsOn?.('googleTasks.changed', refresh)
    return () => {
      active = false
      stop?.()
    }
  }, [])

  async function run(method: string, params: Record<string, unknown>) {
    setBusy(true)
    try {
      let answer = await invoke<AccountsAnswer>(method, params)
      if (answer?.unsent) {
        const discard = await confirmAction({
          title: t('settings.calendars.unsentTitle'),
          message: t('settings.calendars.unsentConfirm', { count: answer.unsent }),
          confirmLabel: t('settings.calendars.discardUnsent'),
          tone: 'danger',
        })
        if (!discard) return
        answer = await invoke<AccountsAnswer>(method, { ...params, discard_unsent: true })
      }
      if (answer?.accounts) {
        readVersion.current++
        setAccounts(answer.accounts)
      }
      if (method === 'googleTasks.connect') setEmail('')
    } catch (error) {
      showToast(errorMessage(error, provider), 'error')
    } finally {
      setBusy(false)
    }
  }

  if (!canConnect && accounts.length === 0) return null

  return (
    <section className="flex flex-col gap-3 border-t border-border/60 pt-4">
      <h2 className="text-sm font-semibold text-primary">{provider}</h2>
      {accounts.map((account) => (
        <div
          key={account.email}
          className="flex flex-wrap items-center justify-between gap-2 rounded-xl bg-raised px-3 py-2"
        >
          <div className="min-w-0">
            <div className="truncate text-xs font-medium">{account.email}</div>
            <div className={account.last_error ? 'break-words text-xs text-rose-500' : 'text-xs text-secondary'}>
              {account.last_error
                ? t('settings.calendars.syncFailed', { error: account.last_error })
                : account.last_sync_at
                  ? t('settings.calendars.lastSynced', { time: formatThreadDate(account.last_sync_at) })
                  : t('settings.calendars.notSynced')}
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            {canConnect && account.last_error && (
              <Button
                size="sm"
                disabled={busy}
                onClick={() => void run('googleTasks.connect', { login_hint: account.email })}
              >
                {t('settings.account.reconnectButton')}
              </Button>
            )}
            <Button
              size="sm"
              disabled={busy}
              leftIcon={RefreshCw}
              onClick={() => void run('googleTasks.sync', { email: account.email })}
            >
              {t('settings.calendars.syncNow')}
            </Button>
            <Button
              size="sm"
              disabled={busy}
              leftIcon={Trash2}
              onClick={async () => {
                const remove = await confirmAction({
                  title: t('settings.account.removeAccountTitle'),
                  message: t('settings.tasks.removeGoogleConfirm', { email: account.email }),
                  confirmLabel: t('settings.calendars.remove'),
                  tone: 'danger',
                })
                if (remove) await run('googleTasks.remove', { email: account.email })
              }}
            >
              {t('settings.calendars.remove')}
            </Button>
          </div>
        </div>
      ))}
      {canConnect && (
        <form
          className="flex flex-wrap gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            void run('googleTasks.connect', { login_hint: email.trim() })
          }}
        >
          <TextInput
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            required
            placeholder={t('accounts.fields.emailAddress')}
            aria-label={t('accounts.fields.emailAddress')}
          />
          <Button type="submit" size="sm" disabled={busy || !email.trim()} aria-busy={busy}>
            {t('accounts.oauth.signInWith', { provider: 'Google' })}
          </Button>
        </form>
      )}
    </section>
  )
}
