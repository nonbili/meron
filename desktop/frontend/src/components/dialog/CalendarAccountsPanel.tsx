import { useEffect, useId, useState } from 'react'
import { ChevronDown, KeyRound, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { formatThreadDate } from '../../lib/date'
import { errorMessage } from '../../lib/errors'
import { isGoogleMailbox } from '../../lib/googleMailbox'
import { accounts$ } from '../../states/accounts'
import {
  SYNC_INTERVALS,
  addCalendarAccount,
  connectGoogleCalendar,
  createCalendar,
  deleteCalendar,
  discoverCalendarServer,
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
import { showToast, ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { SelectInput, TextInput } from '../field/Field'
import { ColorSwatchPicker } from '../field/ColorSwatchPicker'
import { COLOR_PRESETS, normalizeHexColor } from '../../lib/colorPresets'
import { SettingRow, SettingsGroup, Switch } from './AccountSettingsRows'

type CalendarFormValues = { server?: string; email: string }

// Calendars synced from CalDAV servers (Nextcloud, Fastmail, iCloud, …): add
// an account, choose which of its calendars to show, see how syncing goes.
export function CalendarAccountsPanel() {
  const { t } = useTranslation()
  const accounts = useValue(caldav$.accounts)
  const [adding, setAdding] = useState<null | { initial?: CalendarFormValues }>(null)
  const [mailboxId, setMailboxId] = useState('')
  const [discovering, setDiscovering] = useState(false)
  const [connectingGoogle, setConnectingGoogle] = useState(false)
  const googleConfigured = !!useValue(ui$.system)?.gmail_oauth_configured
  const activeMailboxId = useValue(ui$.selectedAccount)
  const mailboxes = useValue(accounts$).filter(
    (account) =>
      account.email &&
      account.auth_type !== 'rss' &&
      account.auth_type !== 'local' &&
      (!isGoogleMailbox(account) || googleConfigured),
  )
  const mailbox = mailboxes.find((account) => account.id === (mailboxId || activeMailboxId)) ?? mailboxes[0]
  const busy = discovering || connectingGoogle

  const connectGoogle = async (options?: Parameters<typeof connectGoogleCalendar>[0]) => {
    setConnectingGoogle(true)
    try {
      await connectGoogleCalendar(options)
    } catch (err) {
      showToast(errorMessage(err, t('settings.calendars.addFailed')), 'error')
    } finally {
      setConnectingGoogle(false)
    }
  }

  const findCalendar = async () => {
    if (!mailbox || busy) return
    if (isGoogleMailbox(mailbox)) {
      await connectGoogle({ email: mailbox.email })
      return
    }
    setDiscovering(true)
    try {
      const server = await discoverCalendarServer(mailbox.email)
      setAdding({ initial: { email: mailbox.email, server: server ?? undefined } })
    } catch {
      setAdding({ initial: { email: mailbox.email } })
    } finally {
      setDiscovering(false)
    }
  }

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
      <div className="flex flex-wrap items-start justify-between gap-4">
        <p className="max-w-lg text-xs text-secondary">{t('settings.calendars.intro')}</p>
        <div className="flex flex-wrap gap-2">
          {googleConfigured && (
            <Button variant="secondary" size="sm" disabled={busy} onClick={() => void connectGoogle()}>
              {connectingGoogle ? t('settings.calendars.connecting') : t('settings.calendars.connectGoogle')}
            </Button>
          )}
          {!adding && (
            <Button size="sm" leftIcon={Plus} disabled={busy} onClick={() => setAdding({})}>
              {t('settings.calendars.add')}
            </Button>
          )}
        </div>
      </div>
      {mailbox && !adding && (
        <SettingsGroup title={t('settings.calendars.findForMailbox')}>
          <form
            className="flex flex-wrap items-center gap-2 p-3.5"
            onSubmit={(event) => {
              event.preventDefault()
              void findCalendar()
            }}
          >
            <SelectInput
              aria-label={t('settings.sections.mailAccounts')}
              wrapperClassName="min-w-0 grow"
              value={mailbox.id}
              disabled={busy}
              onChange={(event) => setMailboxId(event.target.value)}
            >
              {mailboxes.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.email}
                </option>
              ))}
            </SelectInput>
            <Button type="submit" size="sm" disabled={busy}>
              {discovering
                ? t('common.loading')
                : isGoogleMailbox(mailbox)
                  ? t('settings.calendars.connect')
                  : t('settings.calendars.findForMailbox')}
            </Button>
          </form>
        </SettingsGroup>
      )}
      {adding && (
        <SettingsGroup title={t('settings.calendars.add')}>
          {adding.initial && !adding.initial.server && (
            <p role="status" className="px-3.5 pt-3.5 text-xs text-secondary">
              {t('settings.calendars.discoveryNoServer')}
            </p>
          )}
          <CalendarAccountForm initial={adding.initial} onDone={() => setAdding(null)} />
        </SettingsGroup>
      )}
      {accounts.length === 0 && !adding && (
        <p className="rounded-2xl border border-dashed border-border/70 px-4 py-6 text-center text-xs text-secondary">
          {t('settings.calendars.none')}
        </p>
      )}
      {accounts.length > 0 && (
        <div className="flex flex-col gap-2">
          {accounts.map((account) => (
            <AccountCard
              key={account.id}
              account={account}
              onReconnect={
                googleConfigured
                  ? () => void connectGoogle({ accountId: account.id, email: account.username })
                  : undefined
              }
              connectingGoogle={busy}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export function CalendarAccountForm({
  onDone,
  initial,
  optional = false,
}: {
  onDone: () => void
  initial?: CalendarFormValues
  optional?: boolean
}) {
  const { t } = useTranslation()
  const [server, setServer] = useState(initial?.server ?? '')
  const [username, setUsername] = useState(initial?.email ?? '')
  const [password, setPassword] = useState('')
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const serverUrl = server.trim()
  const canSubmit = !!serverUrl && !!username.trim() && !!password && !busy

  const submit = async () => {
    if (!canSubmit || (initial?.server && !serverUrl.startsWith('https://'))) return
    setBusy(true)
    setError('')
    try {
      await addCalendarAccount({
        server_url: serverUrl,
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
    <form
      className={optional ? 'mt-4 flex flex-col gap-3' : 'flex flex-col gap-3 p-3.5'}
      onSubmit={(event) => {
        event.preventDefault()
        void submit()
      }}
    >
      <TextInput
        fieldSize="md"
        autoFocus={!initial?.server}
        placeholder={t('settings.calendars.serverPlaceholder')}
        aria-label={t('settings.calendars.server')}
        pattern={initial?.server ? '\\s*https://.+\\s*' : undefined}
        title={initial?.server ? 'https://…' : undefined}
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
        autoFocus={!!initial?.server}
        autoComplete={initial ? 'off' : undefined}
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
      {error && (
        <p role="alert" className="text-xs text-rose-500 select-text">
          {error}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={onDone} disabled={busy}>
          {t(optional ? 'buttons.skip' : 'buttons.cancel')}
        </Button>
        <Button type="submit" size="sm" disabled={!canSubmit}>
          {busy ? t('settings.calendars.connecting') : t('settings.calendars.connect')}
        </Button>
      </div>
    </form>
  )
}

function AccountCard({
  account,
  onReconnect,
  connectingGoogle,
}: {
  account: CalendarAccount
  onReconnect?: () => void
  connectingGoogle: boolean
}) {
  const { t } = useTranslation()
  const id = useId()
  const [expanded, setExpanded] = useState(false)
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
    <section className="overflow-hidden rounded-2xl border border-border/60 bg-raised/80 shadow-sm shadow-black/[0.03] dark:shadow-black/30">
      <button
        type="button"
        aria-labelledby={`${id}-name`}
        aria-describedby={`${id}-summary`}
        aria-expanded={expanded}
        aria-controls={`${id}-details`}
        className="flex w-full cursor-pointer items-center gap-3 rounded-2xl px-3.5 py-2.5 text-start hover:bg-hover focus-visible:outline-offset-[-2px]"
        onClick={() => {
          if (expanded) {
            setPassword(null)
            setPasswordError('')
          }
          setExpanded(!expanded)
        }}
      >
        <span className="flex min-w-0 flex-1 flex-col">
          <span id={`${id}-name`} className="truncate text-xs font-semibold text-primary">
            {account.display_name || account.server_url}
          </span>
          <span id={`${id}-summary`} className="flex min-w-0 items-center gap-2 text-[0.6875rem] text-secondary">
            {account.username && <span className="min-w-0 max-w-1/2 truncate">{account.username}</span>}
            {account.username && <span aria-hidden>·</span>}
            <span className={`min-w-0 truncate ${account.last_error ? 'text-rose-500' : ''}`} title={status}>
              {status}
            </span>
          </span>
        </span>
        <ChevronDown
          size={15}
          aria-hidden
          className={`shrink-0 text-secondary transition-transform ${expanded ? 'rotate-180' : ''}`}
        />
      </button>
      <div id={`${id}-details`} hidden={!expanded} className="divide-y divide-border/40 border-t border-border/40">
        <SettingRow
          title={t('settings.calendars.accountName')}
          hint={t('settings.calendars.accountNameHint')}
          control={<AccountName account={account} />}
        />
        <div className="flex flex-wrap items-center gap-1.5 px-3.5 py-2.5">
          <Button variant="secondary" size="sm" leftIcon={RefreshCw} disabled={syncing} onClick={() => void sync()}>
            {t('settings.calendars.syncNow')}
          </Button>
          {account.google ? (
            onReconnect && (
              <Button variant="ghost" size="sm" leftIcon={KeyRound} disabled={connectingGoogle} onClick={onReconnect}>
                {t('accounts.actions.reconnectAccount')}
              </Button>
            )
          ) : (
            <Button variant="ghost" size="sm" leftIcon={KeyRound} onClick={() => setPassword('')}>
              {t('settings.calendars.updatePassword')}
            </Button>
          )}
          <Button variant="danger" size="sm" leftIcon={Trash2} onClick={() => void removeCalendarAccount(account)}>
            {t('settings.calendars.remove')}
          </Button>
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
          <CalendarRow key={calendar.id} calendar={calendar} manageCollection={!account.google} />
        ))}
        {!account.google && <NewCalendar accountId={account.id} />}
      </div>
    </section>
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
function CalendarRow({ calendar, manageCollection }: { calendar: CalendarAccountCalendar; manageCollection: boolean }) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  return (
    <>
      <SettingRow
        title={calendar.read_only ? `${calendar.name} · ${t('calendar.readOnly')}` : calendar.name}
        control={
          <span className="flex items-center gap-2">
            <span
              aria-hidden
              className="h-3 w-3 shrink-0 rounded-full"
              style={{ backgroundColor: calendarColor(calendar) }}
            />
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('settings.calendars.edit')}
              aria-expanded={editing}
              onClick={() => setEditing(!editing)}
            >
              {t('buttons.edit')}
            </Button>
            <Switch
              checked={calendar.enabled}
              label={calendar.name}
              onChange={() => void setCalendarEnabled(calendar.id, !calendar.enabled)}
            />
          </span>
        }
      />
      {editing && (
        <CalendarEditor calendar={calendar} manageCollection={manageCollection} onDone={() => setEditing(false)} />
      )}
    </>
  )
}

function CalendarEditor({
  calendar,
  manageCollection,
  onDone,
}: {
  calendar: CalendarAccountCalendar
  manageCollection: boolean
  onDone: () => void
}) {
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
      {manageCollection && !calendar.read_only && (
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
      {manageCollection && (
        <div>
          <Button variant="danger" size="sm" leftIcon={Trash2} onClick={() => void deleteCalendar(calendar)}>
            {t('settings.calendars.deleteCalendar')}
          </Button>
        </div>
      )}
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
