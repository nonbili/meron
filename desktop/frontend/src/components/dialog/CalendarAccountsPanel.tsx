import { useEffect, useState } from 'react'
import { KeyRound, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { formatThreadDate } from '../../lib/date'
import { errorMessage } from '../../lib/errors'
import {
  addCalendarAccount,
  caldav$,
  loadCalendarAccounts,
  removeCalendarAccount,
  setCalendarEnabled,
  setCalendarPassword,
  syncCalendarAccount,
  type CalendarAccount,
} from '../../states/caldav'
import { useValue } from '@legendapp/state/react'
import { Button } from '../button/Button'
import { TextInput } from '../field/Field'
import { SettingRow, SettingsGroup, Switch } from './AccountSettingsRows'

// Calendars synced from CalDAV servers (Nextcloud, Fastmail, iCloud, …): add
// an account, choose which of its calendars to show, see how syncing goes.
export function CalendarAccountsPanel() {
  const { t } = useTranslation()
  const accounts = useValue(caldav$.accounts)
  const [adding, setAdding] = useState(false)

  useEffect(() => {
    void loadCalendarAccounts()
    const eventsOn = (window as any).runtime?.EventsOn
    const off = typeof eventsOn === 'function' ? eventsOn('caldav.changed', () => void loadCalendarAccounts()) : null
    return () => {
      if (typeof off === 'function') off()
    }
  }, [])

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-start justify-between gap-4">
        <p className="max-w-lg text-xs text-secondary">{t('settings.calendars.intro')}</p>
        {!adding && (
          <Button size="sm" leftIcon={Plus} onClick={() => setAdding(true)}>
            {t('settings.calendars.add')}
          </Button>
        )}
      </div>
      {adding && <AddAccountForm onDone={() => setAdding(false)} />}
      {accounts.length === 0 && !adding && (
        <p className="rounded-2xl border border-dashed border-border/70 px-4 py-6 text-center text-xs text-secondary">
          {t('settings.calendars.none')}
        </p>
      )}
      {accounts.map((account) => (
        <AccountCard key={account.id} account={account} />
      ))}
    </div>
  )
}

function AddAccountForm({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  const [server, setServer] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const canSubmit = !!server.trim() && !!username.trim() && !!password && !busy

  const submit = async () => {
    if (!canSubmit) return
    setBusy(true)
    setError('')
    try {
      await addCalendarAccount({ server_url: server.trim(), username: username.trim(), password })
      onDone()
    } catch (err) {
      setError(errorMessage(err, t('settings.calendars.addFailed')))
    } finally {
      setBusy(false)
    }
  }

  return (
    <SettingsGroup title={t('settings.calendars.add')}>
      <form
        className="flex flex-col gap-3 p-3.5"
        onSubmit={(event) => {
          event.preventDefault()
          void submit()
        }}
      >
        <TextInput
          fieldSize="md"
          autoFocus
          placeholder={t('settings.calendars.serverPlaceholder')}
          aria-label={t('settings.calendars.server')}
          value={server}
          onChange={(event) => setServer(event.target.value)}
          autoCapitalize="off"
          spellCheck={false}
        />
        <TextInput
          fieldSize="md"
          placeholder={t('settings.calendars.username')}
          aria-label={t('settings.calendars.username')}
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          autoCapitalize="off"
          spellCheck={false}
        />
        <TextInput
          fieldSize="md"
          type="password"
          placeholder={t('settings.calendars.password')}
          aria-label={t('settings.calendars.password')}
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <p className="text-[0.6875rem] text-secondary">{t('settings.calendars.passwordHint')}</p>
        {error && <p className="text-xs text-rose-500 select-text">{error}</p>}
        <div className="flex justify-end gap-2">
          <Button variant="secondary" size="sm" onClick={onDone} disabled={busy}>
            {t('buttons.cancel')}
          </Button>
          <Button type="submit" size="sm" disabled={!canSubmit}>
            {busy ? t('settings.calendars.connecting') : t('settings.calendars.connect')}
          </Button>
        </div>
      </form>
    </SettingsGroup>
  )
}

function AccountCard({ account }: { account: CalendarAccount }) {
  const { t } = useTranslation()
  const [syncing, setSyncing] = useState(false)
  const [password, setPassword] = useState<string | null>(null)
  const [passwordError, setPasswordError] = useState('')
  const eventCalendars = account.calendars.filter((calendar) => calendar.events)

  const status = account.last_error
    ? t('settings.calendars.syncFailed', { error: account.last_error })
    : account.last_sync_at
      ? t('settings.calendars.lastSynced', { time: formatThreadDate(account.last_sync_at) })
      : t('settings.calendars.notSynced')

  const sync = async () => {
    setSyncing(true)
    await syncCalendarAccount(account.id)
    setSyncing(false)
  }

  const savePassword = async () => {
    if (!password) return
    setPasswordError('')
    try {
      await setCalendarPassword(account.id, password)
      setPassword(null)
    } catch (err) {
      setPasswordError(errorMessage(err, t('settings.calendars.addFailed')))
    }
  }

  return (
    <SettingsGroup title={account.display_name || account.server_url}>
      <div className="flex items-center justify-between gap-4 px-3.5 py-2.5">
        <div className="flex min-w-0 flex-col">
          <span className="truncate text-xs text-primary">{account.username}</span>
          <span
            className={`truncate text-[0.6875rem] ${account.last_error ? 'text-rose-500' : 'text-secondary'}`}
            title={account.last_error || account.server_url}
          >
            {status}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button variant="secondary" size="sm" leftIcon={RefreshCw} disabled={syncing} onClick={() => void sync()}>
            {t('settings.calendars.syncNow')}
          </Button>
          <Button variant="ghost" size="sm" leftIcon={KeyRound} onClick={() => setPassword('')}>
            {t('settings.calendars.updatePassword')}
          </Button>
          <Button variant="danger" size="sm" leftIcon={Trash2} onClick={() => void removeCalendarAccount(account)}>
            {t('settings.calendars.remove')}
          </Button>
        </div>
      </div>
      {password !== null && (
        <form
          className="flex flex-col gap-2 px-3.5 py-2.5"
          onSubmit={(event) => {
            event.preventDefault()
            void savePassword()
          }}
        >
          <div className="flex items-center gap-2">
            <TextInput
              autoFocus
              type="password"
              className="flex-1"
              placeholder={t('settings.calendars.password')}
              aria-label={t('settings.calendars.password')}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
            <Button variant="secondary" size="sm" onClick={() => setPassword(null)}>
              {t('buttons.cancel')}
            </Button>
            <Button type="submit" size="sm" disabled={!password}>
              {t('buttons.save')}
            </Button>
          </div>
          {passwordError && <p className="text-xs text-rose-500 select-text">{passwordError}</p>}
        </form>
      )}
      {eventCalendars.map((calendar) => (
        <SettingRow
          key={calendar.id}
          title={calendar.read_only ? `${calendar.name} · ${t('calendar.readOnly')}` : calendar.name}
          control={
            <span className="flex items-center gap-3">
              {calendar.color && (
                <span className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: calendar.color }} aria-hidden />
              )}
              <Switch
                checked={calendar.enabled}
                label={calendar.name}
                onChange={() => void setCalendarEnabled(calendar.id, !calendar.enabled)}
              />
            </span>
          }
        />
      ))}
    </SettingsGroup>
  )
}
