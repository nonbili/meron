import { useEffect, useId, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { BookUser, KeyRound, Plus, RefreshCw, Search, Trash2 } from 'lucide-react'
import { invoke } from '../../lib/bridge'
import { errorMessage } from '../../lib/errors'
import { isGoogleMailbox } from '../../lib/googleMailbox'
import { useTranslation } from '../../lib/i18n'
import { accounts$ } from '../../states/accounts'
import { confirmAction, ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { SelectInput, TextInput } from '../field/Field'
import { Switch } from '../dialog/AccountSettingsRows'
import { PlainHttpWarning } from '../field/PlainHttpWarning'

type Account = {
  id: string
  username: string
  server: string
  google?: boolean
  books: { id: string; name: string; enabled: boolean; skipped?: number }[]
}

type Form = { server: string; email: string; discovered: boolean }

const sameLogin = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase()

// Address books synced over CardDAV (or Google Contacts), set up like synced
// calendars: pick a mail account and let discovery find its server.
export function AddressBooks({ onChange }: { onChange: () => void }) {
  const { t: translate } = useTranslation()
  const t = (key: string) => translate(`contacts.addressBooks${key}`, { defaultValue: labels[key] })
  const [accounts, setAccounts] = useState<Account[]>([])
  const [busy, setBusy] = useState(false)
  const [waitingForGoogle, setWaitingForGoogle] = useState(false)
  // Bumped to abandon a running step (a Google sign-in left in the browser):
  // its late answer still reloads the list but no longer holds the panel.
  const attempt = useRef(0)
  const [error, setError] = useState('')
  const [form, setForm] = useState<Form | null>(null)
  const [mailboxId, setMailboxId] = useState('')
  const googleConfigured = !!useValue(ui$.system)?.gmail_oauth_configured
  const activeMailboxId = useValue(ui$.selectedAccount)
  // Mail accounts that could have an address book and do not have one here yet.
  const mailboxes = useValue(accounts$).filter(
    (account) =>
      account.email &&
      account.auth_type !== 'rss' &&
      account.auth_type !== 'local' &&
      (!isGoogleMailbox(account) || googleConfigured) &&
      !accounts.some((book) => sameLogin(book.username, account.email)),
  )
  const mailbox = mailboxes.find((account) => account.id === (mailboxId || activeMailboxId)) ?? mailboxes[0]

  const reload = async () => setAccounts((await invoke<{ accounts: Account[] }>('carddav.accounts')).accounts)
  useEffect(() => {
    void reload().catch((err) => setError(String(err)))
  }, [])

  async function run(work: () => Promise<unknown>) {
    if (busy) return false
    const current = ++attempt.current
    const live = () => attempt.current === current
    setBusy(true)
    setError('')
    let ok = true
    try {
      await work()
    } catch (err) {
      ok = false
      if (live()) setError(errorMessage(err, t('Failed')))
    } finally {
      onChange()
      try {
        await reload()
      } catch (err) {
        if (live()) setError(String(err))
      }
      if (live()) {
        setBusy(false)
        setWaitingForGoogle(false)
      }
    }
    return ok && live()
  }

  /** Stop waiting for the running step; whatever it returns later is ignored. */
  function abandon() {
    attempt.current++
    setBusy(false)
    setWaitingForGoogle(false)
  }

  function connectGoogle(options: { account?: Account; email?: string } = {}) {
    void run(async () => {
      setWaitingForGoogle(true)
      const res = await invoke<{ sync_error?: string }>('carddav.connectGoogle', {
        account_id: options.account?.id,
        login_hint: options.account?.username ?? options.email,
      })
      if (res.sync_error) throw new Error(res.sync_error)
    })
  }

  function find() {
    if (!mailbox) return
    if (isGoogleMailbox(mailbox)) {
      connectGoogle({ email: mailbox.email })
      return
    }
    void run(async () => {
      let server: string | null = null
      try {
        server = (await invoke<{ server_url: string | null }>('carddav.discoverServer', { email: mailbox.email }))
          .server_url
      } catch {
        // Fall through to entering the server by hand.
      }
      setForm({ server: server ?? '', email: mailbox.email, discovered: !!server })
    })
  }

  async function remove(account: Account) {
    const confirmed = await confirmAction({
      title: t('Disconnect'),
      message: translate('contacts.addressBooksDisconnectConfirm', {
        name: account.username,
        defaultValue: 'Stop syncing {name}? Its contacts are removed from this device; the server keeps them.',
      }),
      confirmLabel: t('Disconnect'),
      tone: 'danger',
    })
    if (confirmed) void run(() => invoke('carddav.remove', { account_id: account.id }))
  }

  return (
    <div className="min-h-0 flex-1 space-y-5 overflow-y-auto p-3">
      <p className="px-1 text-xs leading-relaxed text-secondary">{t('Description')}</p>
      {waitingForGoogle && (
        <div className="flex items-center gap-2 rounded-xl border border-border/60 bg-raised/80 px-3 py-2">
          <p role="status" className="min-w-0 flex-1 text-xs text-secondary">
            {translate('accounts.oauth.waitingForSignIn', { provider: 'Google' })}
          </p>
          <Button variant="secondary" size="sm" onClick={abandon}>
            {translate('buttons.cancel')}
          </Button>
        </div>
      )}
      {error && (
        <p role="alert" className="break-words px-1 text-xs text-rose-500 select-text">
          {error}
        </p>
      )}
      {accounts.length > 0 && (
        <div className="flex flex-col gap-2">
          {accounts.map((account) => (
            <AccountCard
              key={account.id}
              account={account}
              locked={busy}
              t={t}
              translate={translate}
              onRun={run}
              onReconnect={account.google && googleConfigured ? () => connectGoogle({ account }) : undefined}
              onRemove={() => void remove(account)}
            />
          ))}
        </div>
      )}
      <section>
        <h3 className="mb-2 px-1 text-[0.75rem] font-semibold text-secondary">{t('Add')}</h3>
        <div className="flex flex-col gap-3 rounded-2xl border border-border/60 bg-raised/80 p-3.5 shadow-sm shadow-black/[0.03] dark:shadow-black/30">
          {form ? (
            <ServerForm
              key={`${form.email}:${form.server}`}
              initial={form}
              disabled={busy}
              t={t}
              // While connecting, stop and keep the form to fix it; then close.
              onCancel={() => (busy ? abandon() : setForm(null))}
              onSubmit={(input) =>
                run(async () => {
                  const result = await invoke<{ account_id: string }>('carddav.add', input)
                  await invoke('carddav.sync', { account_id: result.account_id })
                }).then((ok) => ok && setForm(null))
              }
            />
          ) : (
            <>
              {mailbox ? (
                <form
                  className="flex flex-col gap-2"
                  onSubmit={(event) => {
                    event.preventDefault()
                    find()
                  }}
                >
                  <SelectInput
                    fieldSize="md"
                    aria-label={t('Mailbox')}
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
                  <Button type="submit" leftIcon={Search} disabled={busy}>
                    {busy ? translate('settings.calendars.connecting') : t('Find')}
                  </Button>
                </form>
              ) : (
                <p className="text-xs text-secondary">{t('NoMailbox')}</p>
              )}
              <div className="border-t border-border/40 pt-3">
                <Button
                  variant="ghost"
                  size="sm"
                  leftIcon={Plus}
                  disabled={busy}
                  onClick={() => setForm({ server: '', email: '', discovered: false })}
                >
                  {t('Manual')}
                </Button>
              </div>
            </>
          )}
        </div>
      </section>
    </div>
  )
}

function ServerForm({
  initial,
  disabled,
  t,
  onCancel,
  onSubmit,
}: {
  initial: Form
  disabled: boolean
  t: (key: string) => string
  onCancel: () => void
  onSubmit: (input: { server_url: string; username: string; password: string; mail_account_id?: string }) => void
}) {
  const { t: translate } = useTranslation()
  const [server, setServer] = useState(initial.server)
  const [username, setUsername] = useState(initial.email)
  const [password, setPassword] = useState('')
  const login = username.trim()
  // A mail account on the same login usually opens the address book too, so
  // its password (which stays in the core) can stand in for a blank one.
  const mailAccount = useValue(accounts$).find(
    (account) =>
      !!login &&
      account.auth_type === 'password' &&
      !account.needs_reconnect &&
      sameLogin(account.username || account.email, login),
  )
  const canSubmit = !!server.trim() && !!login && (!!password || !!mailAccount) && !disabled

  return (
    <form
      className="flex flex-col gap-2.5"
      onSubmit={(event) => {
        event.preventDefault()
        if (!canSubmit) return
        onSubmit({
          server_url: server.trim(),
          username: login,
          password,
          mail_account_id: password ? undefined : mailAccount?.id,
        })
      }}
    >
      {initial.email && (
        <p role="status" className="text-xs text-secondary">
          {initial.discovered ? t('Found') : t('NotFound')}
        </p>
      )}
      <TextInput
        fieldSize="md"
        autoFocus={!initial.discovered}
        placeholder={translate('settings.calendars.serverPlaceholder')}
        aria-label={t('Server')}
        value={server}
        onChange={(event) => setServer(event.target.value)}
        autoCapitalize="off"
        spellCheck={false}
      />
      <TextInput
        fieldSize="md"
        placeholder={translate('settings.calendars.username')}
        aria-label={translate('settings.calendars.username')}
        value={username}
        onChange={(event) => setUsername(event.target.value)}
        autoComplete="username"
        autoCapitalize="off"
        spellCheck={false}
      />
      <TextInput
        fieldSize="md"
        type="password"
        autoFocus={initial.discovered && !mailAccount}
        placeholder={translate(mailAccount ? 'settings.calendars.passwordFromMail' : 'settings.calendars.password')}
        aria-label={translate('settings.calendars.password')}
        value={password}
        onChange={(event) => setPassword(event.target.value)}
        autoComplete="current-password"
      />
      <PlainHttpWarning server={server} mailPassword={!password && !!mailAccount} />
      <p className="text-[0.6875rem] text-secondary">{translate('settings.calendars.passwordHint')}</p>
      <div className="flex justify-end gap-2">
        <Button variant="secondary" size="sm" onClick={onCancel}>
          {translate('buttons.cancel')}
        </Button>
        <Button type="submit" size="sm" disabled={!canSubmit}>
          {disabled ? translate('settings.calendars.connecting') : t('Connect')}
        </Button>
      </div>
    </form>
  )
}

function AccountCard({
  account,
  locked,
  t,
  translate,
  onRun,
  onReconnect,
  onRemove,
}: {
  account: Account
  locked: boolean
  t: (key: string) => string
  translate: ReturnType<typeof useTranslation>['t']
  onRun: (work: () => Promise<unknown>) => Promise<boolean>
  onReconnect?: () => void
  onRemove: () => void
}) {
  const id = useId()

  return (
    <section
      aria-labelledby={`${id}-name`}
      className="overflow-hidden rounded-2xl border border-border/60 bg-raised/80 shadow-sm shadow-black/[0.03] dark:shadow-black/30"
    >
      <div className="flex items-center gap-3 px-3.5 py-2.5">
        <span aria-hidden className="grid size-8 shrink-0 place-items-center rounded-full bg-accent/10 text-accent">
          <BookUser size={16} />
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span id={`${id}-name`} className="truncate text-xs font-semibold text-primary" title={account.username}>
            {account.username}
          </span>
          <span className="truncate text-[0.6875rem] text-secondary" title={account.server}>
            {account.google ? 'Google Contacts' : account.server}
          </span>
        </span>
      </div>
      <div className="border-t border-border/40 px-1.5 py-1.5">
        {account.books.length === 0 && <p className="px-2 py-1.5 text-xs text-secondary">{t('NoBooks')}</p>}
        {account.books.map((book) => (
          <div key={book.id}>
            <div className="flex min-h-9 items-center gap-2 rounded-lg px-2 hover:bg-hover">
              <span
                className={`min-w-0 flex-1 truncate text-xs ${book.enabled ? 'text-primary' : 'text-secondary'}`}
                title={book.name}
              >
                {book.name}
              </span>
              <Switch
                checked={book.enabled}
                label={book.name}
                onChange={() => {
                  if (locked) return
                  const enabled = !book.enabled
                  void onRun(async () => {
                    await invoke('carddav.setBookEnabled', { book_id: book.id, enabled })
                    if (enabled) await invoke('carddav.sync', { account_id: account.id })
                  })
                }}
              />
            </div>
            {!!book.skipped && (
              <p className="px-2 pb-1 text-[0.6875rem] text-secondary">
                {translate('contacts.addressBooksSkipped', {
                  count: book.skipped,
                  defaultValue: '{count} unreadable vCards skipped.',
                })}
              </p>
            )}
          </div>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-1.5 border-t border-border/40 bg-hover/40 px-3.5 py-2">
        <Button
          variant="secondary"
          size="sm"
          leftIcon={RefreshCw}
          disabled={locked}
          onClick={() => void onRun(() => invoke('carddav.sync', { account_id: account.id }))}
        >
          {translate('settings.calendars.syncNow')}
        </Button>
        {onReconnect && (
          <Button variant="ghost" size="sm" leftIcon={KeyRound} disabled={locked} onClick={onReconnect}>
            {t('ReconnectGoogle')}
          </Button>
        )}
        <IconButton
          icon={Trash2}
          size="sm"
          label={t('Disconnect')}
          className="ms-auto text-rose-600 hover:text-rose-600 dark:text-rose-400"
          disabled={locked}
          onClick={onRemove}
        />
      </div>
    </section>
  )
}

const labels: Record<string, string> = {
  Description:
    'Sync contacts from a CardDAV server or Google Contacts. Add and edit them from the contacts list; changes are saved to the server.',
  Failed: 'Could not connect the address book.',
  Add: 'Add an address book',
  Mailbox: 'Mail account',
  Find: 'Find address book',
  NoMailbox: 'Add a mail account to find its address book, or enter a server.',
  Manual: 'Enter server',
  Found: 'Found an address book server. Sign in to connect it.',
  NotFound: 'No address book server was found automatically. Enter the server address.',
  NoBooks: 'No address books on this account.',
  Disconnect: 'Disconnect',
  Server: 'CardDAV server URL',
  Connect: 'Connect',
  ReconnectGoogle: 'Reconnect',
}
