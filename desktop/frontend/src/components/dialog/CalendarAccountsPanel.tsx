import { useEffect, useId, useRef, useState } from 'react'
import { ChevronDown, KeyRound, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { formatThreadDate } from '../../lib/date'
import { errorMessage } from '../../lib/errors'
import { isGoogleMailbox } from '../../lib/googleMailbox'
import { accounts$ } from '../../states/accounts'
import { settings$ } from '../../states/settings'
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
  setCalendarTasksEnabled,
  setCalendarPassword,
  syncCalendarAccount,
  type CalendarAccount,
} from '../../states/caldav'
import { useValue } from '@legendapp/state/react'
import { calendarColor } from '../../states/calendar'
import { showToast, ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { clsx } from '../../lib/utils'
import { Avatar } from '../avatar/Avatar'
import { SelectInput, TextInput } from '../field/Field'
import { ColorSwatchPicker } from '../field/ColorSwatchPicker'
import { COLOR_PRESETS } from '../../lib/colorPresets'
import { SettingRow, SettingsGroup, Switch } from './AccountSettingsRows'
import { CalendarPickerDialog } from './CalendarPickerDialog'
import { CalendarColorEditor } from '../calendar/CalendarColorEditor'
import { PlainHttpWarning } from '../field/PlainHttpWarning'
import { GoogleTasksSwitch, OrphanGoogleTasksAccounts, useGoogleTasks, type GoogleTasks } from './GoogleTasksAccounts'

type CalendarFormValues = { server?: string; email: string }

// Calendars synced from CalDAV servers (Nextcloud, Fastmail, iCloud, …): add
// an account, choose which of its calendars to show, see how syncing goes.
export function CalendarAccountsPanel() {
  const { t } = useTranslation()
  const accounts = useValue(caldav$.accounts)
  const [adding, setAdding] = useState<null | { initial?: CalendarFormValues }>(null)
  const [newAccountId, setNewAccountId] = useState<string>()
  const [pickingAccountId, setPickingAccountId] = useState<string>()
  const [mailboxId, setMailboxId] = useState('')
  const [discovering, setDiscovering] = useState(false)
  const [connectingGoogle, setConnectingGoogle] = useState(false)
  // The address typed to add a Google account with no mailbox here; null
  // while that form is closed.
  const [googleEmail, setGoogleEmail] = useState<string | null>(null)
  // Bumped to abandon a Google sign-in left in the browser: its late answer
  // still lists the account but no longer holds the panel.
  const googleAttempt = useRef(0)
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
  const googleAccount =
    mailbox && isGoogleMailbox(mailbox)
      ? accounts.find(
          (account) => account.google && account.username.toLowerCase() === mailbox.email.trim().toLowerCase(),
        )
      : undefined
  const busy = discovering || connectingGoogle
  const googleTasks = useGoogleTasks()
  const tasksEnabled = useValue(settings$.tasksEnabled)

  const revealNewAccount = () => {
    const created = caldav$.accounts.peek().find((account) => !accounts.some(({ id }) => id === account.id))
    if (created) setNewAccountId(created.id)
  }

  const connectGoogle = async (options: Parameters<typeof connectGoogleCalendar>[0]) => {
    const attempt = ++googleAttempt.current
    const live = () => googleAttempt.current === attempt
    setConnectingGoogle(true)
    try {
      const id = await connectGoogleCalendar(options)
      // A reconnect renews the grant Google Tasks shares: retry its sync too.
      if (live() && options.accountId && settings$.tasksEnabled.peek() && googleTasks.find(options.email)) {
        void googleTasks.run('googleTasks.sync', { email: options.email })
      }
      if (live() && id && !options.accountId) {
        setNewAccountId(id)
        setPickingAccountId(id)
      }
    } catch (err) {
      if (live()) showToast(errorMessage(err, t('settings.calendars.addFailed')), 'error')
    } finally {
      if (live()) setConnectingGoogle(false)
    }
  }

  const cancelGoogle = () => {
    googleAttempt.current++
    setConnectingGoogle(false)
  }

  const findCalendar = async () => {
    if (!mailbox || busy) return
    if (googleAccount) {
      setPickingAccountId(googleAccount.id)
      return
    }
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
        {!adding && googleEmail === null && (
          <div className="flex flex-wrap gap-2">
            {/* A Google account needs no mailbox here: sign in to add its
                calendars, then switch on its tasks in the card. */}
            {googleConfigured && (
              <Button size="sm" variant="secondary" disabled={busy} onClick={() => setGoogleEmail('')}>
                {t('accounts.oauth.signInWith', { provider: 'Google' })}
              </Button>
            )}
            <Button size="sm" leftIcon={Plus} disabled={busy} onClick={() => setAdding({})}>
              {t('settings.calendars.add')}
            </Button>
          </div>
        )}
      </div>
      {googleEmail !== null && (
        <SettingsGroup title={t('accounts.oauth.signInWith', { provider: 'Google' })}>
          {/* The address comes first so the sign-in can keep the access its
              other services (Contacts, Tasks) already have. */}
          <form
            className="flex flex-wrap items-center gap-2 p-3.5"
            onSubmit={(event) => {
              event.preventDefault()
              // One sign-in at a time: a second would orphan the first's answer.
              if (busy) return
              const email = googleEmail.trim()
              setGoogleEmail(null)
              void connectGoogle({ email })
            }}
          >
            <TextInput
              autoFocus
              type="email"
              required
              className="min-w-0 grow"
              value={googleEmail}
              onChange={(event) => setGoogleEmail(event.target.value)}
              placeholder={t('accounts.fields.emailAddress')}
              aria-label={t('accounts.fields.emailAddress')}
            />
            <Button variant="secondary" size="sm" onClick={() => setGoogleEmail(null)}>
              {t('buttons.cancel')}
            </Button>
            <Button type="submit" size="sm" disabled={busy || !googleEmail.trim()}>
              {t('accounts.oauth.signInWith', { provider: 'Google' })}
            </Button>
          </form>
        </SettingsGroup>
      )}
      {connectingGoogle && (
        <div className="flex items-center gap-3 rounded-2xl border border-border/60 bg-raised/80 px-3.5 py-2.5">
          <p role="status" className="min-w-0 flex-1 text-xs text-secondary">
            {t('accounts.oauth.waitingForSignIn', { provider: 'Google' })}
          </p>
          <Button variant="secondary" size="sm" onClick={cancelGoogle}>
            {t('buttons.cancel')}
          </Button>
        </div>
      )}
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
                : connectingGoogle && isGoogleMailbox(mailbox)
                  ? t('settings.calendars.connecting')
                  : t(googleAccount ? 'settings.calendars.choose' : 'settings.calendars.find')}
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
          <CalendarAccountForm
            initial={adding.initial}
            onDone={() => {
              revealNewAccount()
              setAdding(null)
            }}
          />
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
              newlyAdded={account.id === newAccountId}
              onReconnect={
                googleConfigured
                  ? () => void connectGoogle({ accountId: account.id, email: account.username })
                  : undefined
              }
              connectingGoogle={busy}
              googleTasks={googleTasks}
              googleConfigured={googleConfigured}
            />
          ))}
        </div>
      )}
      {pickingAccountId && (
        <CalendarPickerDialog accountId={pickingAccountId} onClose={() => setPickingAccountId(undefined)} />
      )}
      {tasksEnabled && (
        <OrphanGoogleTasksAccounts
          tasks={googleTasks}
          calendarEmails={accounts.filter((account) => account.google).map((account) => account.username)}
          canConnect={googleConfigured}
        />
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
  // Bumped when a connect is cancelled: a server that does not answer must
  // not hold the form, and its late answer is ignored.
  const attempt = useRef(0)
  const accounts = useValue(accounts$)
  const serverUrl = server.trim()
  const login = username.trim().toLowerCase()
  // A mail account on the same login usually opens the calendars too, so its
  // password (which stays in the core) can stand in for a blank one.
  const mailAccount = login
    ? accounts.find(
        (account) =>
          account.auth_type === 'password' &&
          !account.needs_reconnect &&
          (account.username || account.email).trim().toLowerCase() === login,
      )
    : undefined
  const canSubmit = !!serverUrl && !!login && (!!password || !!mailAccount) && !busy

  const submit = async () => {
    if (!canSubmit || (initial?.server && !serverUrl.startsWith('https://'))) return
    const current = ++attempt.current
    const live = () => attempt.current === current
    setBusy(true)
    setError('')
    try {
      await addCalendarAccount({
        server_url: serverUrl,
        username: username.trim(),
        password,
        mail_account_id: password ? undefined : mailAccount?.id,
        display_name: name.trim(),
      })
      if (live()) onDone()
    } catch (err) {
      if (live()) setError(errorMessage(err, t('settings.calendars.addFailed')))
    } finally {
      if (live()) setBusy(false)
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
        autoFocus={!!initial?.server && !mailAccount}
        autoComplete={initial ? 'off' : undefined}
        placeholder={t(mailAccount ? 'settings.calendars.passwordFromMail' : 'settings.calendars.password')}
        aria-label={t('settings.calendars.password')}
        value={password}
        onChange={(event) => setPassword(event.target.value)}
      />
      <PlainHttpWarning server={server} mailPassword={!password && !!mailAccount} />
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
        <Button
          variant="secondary"
          size="sm"
          // While connecting, stop and keep the form to fix it; then close.
          onClick={() => {
            if (!busy) return onDone()
            attempt.current++
            setBusy(false)
          }}
        >
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
  newlyAdded,
  onReconnect,
  connectingGoogle,
  googleTasks,
  googleConfigured,
}: {
  account: CalendarAccount
  newlyAdded: boolean
  onReconnect?: () => void
  connectingGoogle: boolean
  googleTasks: GoogleTasks
  googleConfigured: boolean
}) {
  const { t } = useTranslation()
  const id = useId()
  const name = account.name
  const tasksEnabled = useValue(settings$.tasksEnabled)
  // Google Tasks rides on a Google account's card; while Tasks is off it rests.
  const tasksAccount = account.google && tasksEnabled ? googleTasks.find(account.username) : undefined
  const tasksError = tasksAccount?.last_error ?? ''
  const [expanded, setExpanded] = useState(!!account.last_error || !!tasksError || newlyAdded)
  useEffect(() => {
    if (account.last_error || tasksError || newlyAdded) setExpanded(true)
  }, [account.last_error, tasksError, newlyAdded])
  const [syncing, setSyncing] = useState(false)
  const [password, setPassword] = useState<string | null>(null)
  const [passwordError, setPasswordError] = useState('')
  const eventCalendars = account.calendars.filter((calendar) => calendar.events)
  const taskCalendars = tasksEnabled
    ? account.calendars.filter((calendar) => calendar.tasks || calendar.tasks_enabled)
    : []

  const status = account.last_error
    ? t('settings.calendars.syncFailed', { error: account.last_error })
    : account.last_sync_at
      ? t('settings.calendars.lastSynced', { time: formatThreadDate(account.last_sync_at) })
      : t('settings.calendars.notSynced')
  // Collapsed, a Google Tasks failure shows here too; open, its own row has it.
  const tasksFailure = !account.last_error && !expanded && tasksError
  const summary = tasksFailure ? `Google Tasks · ${t('settings.calendars.syncFailed', { error: tasksError })}` : status

  const sync = async () => {
    setSyncing(true)
    await Promise.all([
      syncCalendarAccount(account.id),
      tasksAccount && googleTasks.run('googleTasks.sync', { email: tasksAccount.email }),
    ])
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
        aria-describedby={`${id}-summary${expanded && account.last_error ? ` ${id}-error` : ''}`}
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
        {account.google && (
          <span aria-hidden className="shrink-0">
            <Avatar name={name} src={account.avatar_url} size={32} />
          </span>
        )}
        <span className="flex min-w-0 flex-1 flex-col">
          <span id={`${id}-name`} className="truncate text-xs font-semibold text-primary">
            {name}
          </span>
          <span id={`${id}-summary`} className="flex min-w-0 items-center gap-2 text-[0.6875rem] text-secondary">
            {account.username && <span className="min-w-0 max-w-1/2 truncate">{account.username}</span>}
            {account.username && !(expanded && account.last_error) && <span aria-hidden>·</span>}
            <span
              className={`min-w-0 truncate ${account.last_error || tasksFailure ? 'text-rose-500' : ''}`}
              title={summary}
              hidden={expanded && !!account.last_error}
            >
              {summary}
            </span>
          </span>
        </span>
        <ChevronDown
          size={15}
          aria-hidden
          className={`shrink-0 text-secondary transition-transform ${expanded ? 'rotate-180' : ''}`}
        />
      </button>
      <div id={`${id}-details`} hidden={!expanded} className="border-t border-border/40">
        {account.last_error && (
          <p id={`${id}-error`} className="break-words border-b border-border/40 px-3.5 py-2 text-xs text-rose-500">
            {status}
          </p>
        )}
        <div className="px-1.5 py-1.5">
          <h3 className="px-2 pb-1 pt-1 text-[0.6875rem] font-semibold text-secondary">{t('calendar.calendars')}</h3>
          {eventCalendars.length ? (
            eventCalendars.map((calendar) => (
              <CalendarRow key={calendar.id} calendar={calendar} manageCollection={!account.google} />
            ))
          ) : (
            <p className="px-2 py-1.5 text-xs text-secondary">{t('settings.calendars.noEventCalendars')}</p>
          )}
          {!account.google && <NewCalendar accountId={account.id} />}
        </div>
        {taskCalendars.length > 0 && (
          <div className="border-t border-border/40 px-1.5 py-1.5">
            <h3 className="px-2 pb-1 pt-1 text-[0.6875rem] font-semibold text-secondary">{t('tasks.title')}</h3>
            {taskCalendars.map((calendar) => (
              <div key={calendar.id} className="flex items-center justify-between gap-2 px-2 py-1.5 text-xs">
                <span className="truncate text-primary">{calendar.name}</span>
                <Switch
                  checked={!!calendar.tasks_enabled}
                  label={`${t('tasks.title')} · ${calendar.name}`}
                  onChange={() => void setCalendarTasksEnabled(calendar.id, !calendar.tasks_enabled)}
                />
              </div>
            ))}
          </div>
        )}
        {account.google && tasksEnabled && (
          <GoogleTasksSwitch tasks={googleTasks} email={account.username} canConnect={googleConfigured} />
        )}
        <div className="divide-y divide-border/40 border-t border-border/40">
          <SettingRow
            title={t('settings.calendars.accountName')}
            hint={t('settings.calendars.accountNameHint')}
            control={<AccountName accountId={account.id} name={name} />}
          />
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
        </div>
        {password !== null && (
          <form
            className="flex flex-col gap-2 border-t border-border/40 px-3.5 py-2.5"
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
        <div className="flex flex-wrap items-center gap-1.5 border-t border-border/40 bg-hover/40 px-3.5 py-2">
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
          <Button
            variant="ghost"
            size="sm"
            leftIcon={Trash2}
            className="ms-auto text-rose-600 hover:text-rose-600 dark:text-rose-400"
            onClick={() => void removeCalendarAccount(account)}
          >
            {t('settings.calendars.remove')}
          </Button>
        </div>
      </div>
    </section>
  )
}

/** The account's name, saved when the field is left or Enter pressed. */
function AccountName({ accountId, name }: { accountId: string; name: string }) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState(name)
  useEffect(() => setDraft(name), [name])
  const save = () => {
    if (draft.trim() !== name) void renameCalendarAccount(accountId, draft.trim())
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

/** One calendar: shown or not, its colour behind its dot, and, for a
 * CalDAV server's, its name and deletion behind the edit button. */
function CalendarRow({ calendar, manageCollection }: { calendar: CalendarAccountCalendar; manageCollection: boolean }) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  return (
    <CalendarColorEditor calendarId={calendar.id} color={calendarColor(calendar)} dimmed={!calendar.enabled}>
      {(dot) => (
        <>
          <div className="flex min-h-9 items-center gap-2 rounded-lg px-1.5 hover:bg-hover">
            {dot}
            <span
              className={clsx('min-w-0 flex-1 truncate text-xs', calendar.enabled ? 'text-primary' : 'text-secondary')}
              title={calendar.name}
            >
              {calendar.name}
            </span>
            {calendar.read_only && (
              <span className="shrink-0 rounded-md bg-active px-1.5 py-0.5 text-[0.625rem] font-medium text-secondary">
                {t('calendar.readOnly')}
              </span>
            )}
            {manageCollection && (
              <IconButton
                icon={Pencil}
                size="sm"
                label={t('settings.calendars.edit')}
                aria-expanded={editing}
                active={editing}
                onClick={() => setEditing(!editing)}
              />
            )}
            <Switch
              checked={calendar.enabled}
              label={calendar.name}
              onChange={() => void setCalendarEnabled(calendar.id, !calendar.enabled)}
            />
          </div>
          {editing && <CalendarEditor calendar={calendar} onDone={() => setEditing(false)} />}
        </>
      )}
    </CalendarColorEditor>
  )
}

/** A CalDAV calendar's name, and deleting it, both on its server. */
function CalendarEditor({ calendar, onDone }: { calendar: CalendarAccountCalendar; onDone: () => void }) {
  const { t } = useTranslation()
  const [name, setName] = useState(calendar.name)
  const [error, setError] = useState('')
  return (
    <form
      className="flex flex-col gap-2.5 px-2 pb-2.5 pt-1"
      onSubmit={(event) => {
        event.preventDefault()
        const trimmed = name.trim()
        if (!trimmed || trimmed === calendar.name) return onDone()
        setError('')
        updateCalendar(calendar.id, { name: trimmed })
          .then(onDone)
          .catch((err) => setError(errorMessage(err, t('settings.calendars.saveFailed'))))
      }}
    >
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
      <div className="pt-0.5">
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
      className="flex flex-col gap-2.5 px-2 pb-2.5 pt-1"
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
