import { useCallback, useEffect, useRef, useState } from 'react'
import { KeyRound, RefreshCw, Trash2 } from 'lucide-react'

import { invoke } from '../../lib/bridge'
import { formatThreadDate } from '../../lib/date'
import { errorMessage } from '../../lib/errors'
import { t as translate, useTranslation } from '../../lib/i18n'
import { confirmAction, showToast } from '../../states/ui'
import { Button } from '../button/Button'
import { Switch } from './AccountSettingsRows'

export type GoogleTasksAccount = {
  email: string
  last_error: string
  last_sync_at: number
}

type AccountsAnswer = { accounts?: GoogleTasksAccount[]; unsent?: number }

const provider = 'Google Tasks'

const sameEmail = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase()

/** Google Tasks syncs over its own API, not CalDAV, but shares the Google
 * account's sign-in, so it is switched on from that account's calendar card. */
export function useGoogleTasks() {
  const [accounts, setAccounts] = useState<GoogleTasksAccount[]>([])
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

  const run = useCallback(async (method: string, params: Record<string, unknown>) => {
    setBusy(true)
    try {
      let answer = await invoke<AccountsAnswer>(method, params)
      if (answer?.unsent) {
        const discard = await confirmAction({
          title: translate('settings.calendars.unsentTitle'),
          message: translate('settings.calendars.unsentConfirm', { count: answer.unsent }),
          confirmLabel: translate('settings.calendars.discardUnsent'),
          tone: 'danger',
        })
        if (!discard) return
        answer = await invoke<AccountsAnswer>(method, { ...params, discard_unsent: true })
      }
      if (answer?.accounts) {
        readVersion.current++
        setAccounts(answer.accounts)
      }
    } catch (error) {
      showToast(errorMessage(error, provider), 'error')
    } finally {
      setBusy(false)
    }
  }, [])

  const find = useCallback((email: string) => accounts.find((account) => sameEmail(account.email, email)), [accounts])

  return { accounts, busy, run, find }
}

export type GoogleTasks = ReturnType<typeof useGoogleTasks>

export function useGoogleTasksStatus(account: GoogleTasksAccount | undefined) {
  const { t } = useTranslation()
  if (!account) return ''
  return account.last_error
    ? t('settings.calendars.syncFailed', { error: account.last_error })
    : account.last_sync_at
      ? t('settings.calendars.lastSynced', { time: formatThreadDate(account.last_sync_at) })
      : t('settings.calendars.notSynced')
}

/** The Tasks group of a Google calendar account's card. The card's header
 * has the sync time; this row speaks up only when Google Tasks fails. */
export function GoogleTasksSwitch({
  tasks,
  email,
  canConnect,
}: {
  tasks: GoogleTasks
  email: string
  canConnect: boolean
}) {
  const { t } = useTranslation()
  const account = tasks.find(email)
  const status = useGoogleTasksStatus(account)
  if (!account && !canConnect) return null
  return (
    <div className="border-t border-border/40 px-1.5 py-1.5">
      <h3 className="px-2 pb-1 pt-1 text-[0.6875rem] font-semibold text-secondary">{t('tasks.title')}</h3>
      <div className="flex items-center justify-between gap-2 px-2 py-1.5 text-xs">
        <span className="flex min-w-0 flex-col">
          <span className="truncate text-primary">{provider}</span>
          {account?.last_error && <span className="break-words text-rose-500">{status}</span>}
        </span>
        {/* The card's Reconnect covers the calendars: a grant that still
            serves them but not Tasks needs Tasks to ask for itself. */}
        {account?.last_error && canConnect && (
          <Button
            size="sm"
            variant="ghost"
            leftIcon={KeyRound}
            aria-label={`${t('settings.account.reconnectButton')} · ${provider}`}
            disabled={tasks.busy}
            onClick={() => void tasks.run('googleTasks.connect', { login_hint: account.email })}
          >
            {t('settings.account.reconnectButton')}
          </Button>
        )}
        <Switch
          checked={!!account}
          label={provider}
          onChange={async () => {
            if (tasks.busy) return
            if (!account) return void tasks.run('googleTasks.connect', { login_hint: email })
            const off = await confirmAction({
              title: provider,
              message: t('settings.tasks.removeGoogleConfirm', { email: account.email }),
              confirmLabel: t('settings.calendars.remove'),
              tone: 'danger',
            })
            if (off) await tasks.run('googleTasks.remove', { email: account.email })
          }}
        />
      </div>
    </div>
  )
}

/** Google Tasks accounts left without a Google calendar account in Meron
 * (its calendar account was removed): still shown so they can be synced or
 * removed. New ones are switched on from a calendar account's card. */
export function OrphanGoogleTasksAccounts({
  tasks,
  calendarEmails,
  canConnect,
}: {
  tasks: GoogleTasks
  calendarEmails: string[]
  canConnect: boolean
}) {
  const orphans = tasks.accounts.filter((account) => !calendarEmails.some((email) => sameEmail(email, account.email)))
  if (orphans.length === 0) return null
  return (
    <section className="flex flex-col gap-3 border-t border-border/60 pt-4">
      <h2 className="text-sm font-semibold text-primary">{provider}</h2>
      {orphans.map((account) => (
        <OrphanRow key={account.email} account={account} tasks={tasks} canConnect={canConnect} />
      ))}
    </section>
  )
}

function OrphanRow({
  account,
  tasks,
  canConnect,
}: {
  account: GoogleTasksAccount
  tasks: GoogleTasks
  canConnect: boolean
}) {
  const { t } = useTranslation()
  const status = useGoogleTasksStatus(account)
  return (
    <div className="flex flex-wrap items-center justify-between gap-2 rounded-xl bg-raised px-3 py-2">
      <div className="min-w-0">
        <div className="truncate text-xs font-medium">{account.email}</div>
        <div className={account.last_error ? 'break-words text-xs text-rose-500' : 'text-xs text-secondary'}>
          {status}
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        {canConnect && account.last_error && (
          <Button
            size="sm"
            disabled={tasks.busy}
            onClick={() => void tasks.run('googleTasks.connect', { login_hint: account.email })}
          >
            {t('settings.account.reconnectButton')}
          </Button>
        )}
        <Button
          size="sm"
          disabled={tasks.busy}
          leftIcon={RefreshCw}
          onClick={() => void tasks.run('googleTasks.sync', { email: account.email })}
        >
          {t('settings.calendars.syncNow')}
        </Button>
        <Button
          size="sm"
          disabled={tasks.busy}
          leftIcon={Trash2}
          onClick={async () => {
            const remove = await confirmAction({
              title: t('settings.account.removeAccountTitle'),
              message: t('settings.tasks.removeGoogleConfirm', { email: account.email }),
              confirmLabel: t('settings.calendars.remove'),
              tone: 'danger',
            })
            if (remove) await tasks.run('googleTasks.remove', { email: account.email })
          }}
        >
          {t('settings.calendars.remove')}
        </Button>
      </div>
    </div>
  )
}
