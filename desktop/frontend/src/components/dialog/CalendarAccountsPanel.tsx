import { useEffect, useState } from 'react'
import { KeyRound, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { formatThreadDate } from '../../lib/date'
import { errorMessage } from '../../lib/errors'
import {
  SYNC_INTERVALS,
  addCalendarAccount,
  createCalendar,
  deleteCalendar,
  setSyncInterval,
  updateCalendar,
  type CalendarAccountCalendar,
  caldav$,
  loadCalendarAccounts,
  removeCalendarAccount,
  renameCalendarAccount,
  setCalendarEnabled,
  setCalendarPassword,
  syncCalendarAccount,
  type CalendarAccount,
} from '../../states/caldav'
import { useValue } from '@legendapp/state/react'
import { calendarColor } from '../../states/calendar'
import { Button } from '../button/Button'
import { SelectInput, TextInput } from '../field/Field'
import { ColorSwatchPicker } from '../field/ColorSwatchPicker'
import { COLOR_PRESETS, normalizeHexColor } from '../../lib/colorPresets'
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
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const canSubmit = !!server.trim() && !!username.trim() && !!password && !busy

  const submit = async () => {
    if (!canSubmit) return
    setBusy(true)
    setError('')
    try {
      await addCalendarAccount({
        server_url: server.trim(),
        username: username.trim(),
        password,
        display_name: name.trim(),
      })
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
        <TextInput
          fieldSize="md"
          placeholder={t('settings.calendars.accountNameOptional')}
          aria-label={t('settings.calendars.accountName')}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
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
      <SettingRow
        title={t('settings.calendars.accountName')}
        hint={t('settings.calendars.accountNameHint')}
        control={<AccountName account={account} />}
      />
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
      <SettingRow
        title={t('settings.calendars.syncEvery')}
        control={
          <SelectInput
            aria-label={t('settings.calendars.syncEvery')}
            value={String(account.sync_interval)}
            onChange={(event) => void setSyncInterval(account.id, Number(event.target.value))}
          >
            {SYNC_INTERVALS.map((minutes) => (
              <option key={minutes} value={minutes}>
                {minutes === 0
                  ? t('settings.calendars.syncManually')
                  : t('settings.calendars.minutes', { count: minutes })}
              </option>
            ))}
          </SelectInput>
        }
      />
      {eventCalendars.map((calendar) => (
        <CalendarRow key={calendar.id} calendar={calendar} />
      ))}
      <NewCalendar accountId={account.id} />
    </SettingsGroup>
  )
}

/** The account's name, saved when the field is left or Enter pressed. */
function AccountName({ account }: { account: CalendarAccount }) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState(account.display_name)
  useEffect(() => setDraft(account.display_name), [account.display_name])
  const save = () => {
    if (draft.trim() !== account.display_name) void renameCalendarAccount(account.id, draft.trim())
  }
  return (
    <TextInput
      className="w-48 font-semibold"
      aria-label={t('settings.calendars.accountName')}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={save}
      onKeyDown={(event) => {
        if (event.key === 'Enter') event.currentTarget.blur()
      }}
    />
  )
}

/** One calendar: shown or not, and, behind its edit button, its name and
 * colour, which change on the server too. */
function CalendarRow({ calendar }: { calendar: CalendarAccountCalendar }) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  return (
    <>
      <SettingRow
        title={calendar.read_only ? `${calendar.name} · ${t('calendar.readOnly')}` : calendar.name}
        control={
          <span className="flex items-center gap-2">
            <button
              type="button"
              className="flex h-6 w-6 items-center justify-center rounded-lg cursor-pointer hover:bg-hover"
              title={t('settings.calendars.edit')}
              aria-label={t('settings.calendars.edit')}
              aria-expanded={editing}
              onClick={() => setEditing(!editing)}
            >
              <span className="h-3 w-3 rounded-full" style={{ backgroundColor: calendarColor(calendar) }} />
            </button>
            <Switch
              checked={calendar.enabled}
              label={calendar.name}
              onChange={() => void setCalendarEnabled(calendar.id, !calendar.enabled)}
            />
          </span>
        }
      />
      {editing && <CalendarEditor calendar={calendar} onDone={() => setEditing(false)} />}
    </>
  )
}

function CalendarEditor({ calendar, onDone }: { calendar: CalendarAccountCalendar; onDone: () => void }) {
  const { t } = useTranslation()
  const [name, setName] = useState(calendar.name)
  const [error, setError] = useState('')
  const run = async (changes: { name?: string; color?: string }) => {
    setError('')
    try {
      await updateCalendar(calendar.id, changes)
      return true
    } catch (err) {
      setError(errorMessage(err, t('settings.calendars.saveFailed')))
      return false
    }
  }
  return (
    <form
      className="flex flex-col gap-2.5 px-3.5 py-2.5"
      onSubmit={(event) => {
        event.preventDefault()
        const trimmed = name.trim()
        if (!trimmed || trimmed === calendar.name) return onDone()
        void run({ name: trimmed }).then((ok) => ok && onDone())
      }}
    >
      {/* Someone else's calendar keeps its name; its colour can still be
          chosen, for this device. */}
      {!calendar.read_only && (
        <div className="flex items-center gap-2">
          <TextInput
            className="flex-1"
            aria-label={t('settings.calendars.calendarName')}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <Button type="submit" size="sm" disabled={!name.trim()}>
            {t('buttons.save')}
          </Button>
        </div>
      )}
      <ColorSwatchPicker
        label={t('settings.calendars.color')}
        value={normalizeHexColor(calendar.color) ?? calendarColor(calendar)}
        onChange={(color) => {
          if (color) void run({ color })
        }}
      />
      {error && <p className="text-xs text-rose-500 select-text">{error}</p>}
      <div>
        <Button variant="danger" size="sm" leftIcon={Trash2} onClick={() => void deleteCalendar(calendar)}>
          {t('settings.calendars.deleteCalendar')}
        </Button>
      </div>
    </form>
  )
}

/** Make a calendar on the account's server. */
function NewCalendar({ accountId }: { accountId: string }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [color, setColor] = useState<string>(COLOR_PRESETS[5].hex)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  if (!open) {
    return (
      <div className="px-3.5 py-2">
        <Button variant="ghost" size="sm" leftIcon={Plus} onClick={() => setOpen(true)}>
          {t('settings.calendars.newCalendar')}
        </Button>
      </div>
    )
  }
  const close = () => {
    setOpen(false)
    setName('')
    setError('')
  }
  return (
    <form
      className="flex flex-col gap-2.5 px-3.5 py-2.5"
      onSubmit={(event) => {
        event.preventDefault()
        if (!name.trim() || busy) return
        setBusy(true)
        setError('')
        createCalendar(accountId, name.trim(), color)
          .then(close)
          .catch((err) => setError(errorMessage(err, t('settings.calendars.createFailed'))))
          .finally(() => setBusy(false))
      }}
    >
      <TextInput
        autoFocus
        placeholder={t('settings.calendars.calendarName')}
        aria-label={t('settings.calendars.calendarName')}
        value={name}
        onChange={(event) => setName(event.target.value)}
      />
      <ColorSwatchPicker label={t('settings.calendars.color')} value={color} onChange={(c) => c && setColor(c)} />
      {error && <p className="text-xs text-rose-500 select-text">{error}</p>}
      <div className="flex justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={close} disabled={busy}>
          {t('buttons.cancel')}
        </Button>
        <Button type="submit" size="sm" disabled={!name.trim() || busy}>
          {t('settings.calendars.create')}
        </Button>
      </div>
    </form>
  )
}
