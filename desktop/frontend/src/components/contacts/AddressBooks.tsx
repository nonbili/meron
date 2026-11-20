import { useEffect, useState, type FormEvent } from 'react'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'
import { ui$ } from '../../states/ui'
import { BookContacts } from './BookContacts'

type Account = {
  id: string
  username: string
  server: string
  google?: boolean
  books: { id: string; name: string; enabled: boolean; skipped?: number }[]
}

export function AddressBooks({ onChange }: { onChange: () => void }) {
  const { t: translate } = useTranslation()
  const t = (key: string) => translate(`contacts.addressBooks${key}`, { defaultValue: labels[key] })
  const [accounts, setAccounts] = useState<Account[]>([])
  const [busy, setBusy] = useState(false)
  const [editing, setEditing] = useState(false)
  const locked = busy || editing
  const [error, setError] = useState('')
  const [password, setPassword] = useState('')
  const [openBook, setOpenBook] = useState('')
  const googleConfigured = !!useValue(ui$.system)?.gmail_oauth_configured
  const field = 'w-full rounded border border-border bg-transparent px-2 py-1'
  const button = 'rounded border border-border px-2 py-1 disabled:opacity-50'

  const reload = async () => setAccounts((await invoke<{ accounts: Account[] }>('carddav.accounts')).accounts)
  useEffect(() => {
    void reload().catch((err) => setError(String(err)))
  }, [])

  async function run(work: () => Promise<unknown>) {
    if (locked) return
    setBusy(true)
    setError('')
    try {
      await work()
    } catch (err) {
      setError(String(err))
    } finally {
      onChange()
      try {
        await reload()
      } catch (err) {
        setError(String(err))
      }
      setBusy(false)
    }
  }

  function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    void run(async () => {
      const result = await invoke<{ account_id: string }>('carddav.add', {
        server_url: data.get('server'),
        username: data.get('username'),
        password,
      })
      setPassword('')
      await invoke('carddav.sync', { account_id: result.account_id })
    })
  }

  function connectGoogle(account?: Account) {
    void run(async () => {
      const res = await invoke<{ sync_error?: string }>(
        'carddav.connectGoogle',
        account ? { account_id: account.id, login_hint: account.username } : {},
      )
      if (res.sync_error) throw new Error(res.sync_error)
    })
  }

  return (
    <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-3 text-sm">
      <p className="text-muted-foreground">{t('Description')}</p>
      {error && (
        <p role="alert" className="break-words text-red-500">
          {error}
        </p>
      )}
      {busy && <p role="status">{t('Working')}</p>}
      {accounts.map((account) => (
        <section key={account.id} className="space-y-2 rounded-lg border border-border p-2">
          <div className="break-words font-medium">{account.username}</div>
          <div className="break-all text-xs text-muted-foreground">
            {account.google ? 'Google Contacts' : account.server}
          </div>
          {account.books.map((book) => (
            <div key={book.id} className="space-y-2">
              <div className="flex items-center justify-between gap-2">
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={book.enabled}
                    disabled={locked}
                    onChange={(event) => {
                      const enabled = event.currentTarget.checked
                      void run(async () => {
                        await invoke('carddav.setBookEnabled', { book_id: book.id, enabled })
                        if (enabled) await invoke('carddav.sync', { account_id: account.id })
                      })
                    }}
                  />
                  {book.name}
                </label>
                {book.enabled && (
                  <button
                    type="button"
                    className={button}
                    disabled={locked}
                    aria-expanded={openBook === book.id}
                    onClick={() => setOpenBook(openBook === book.id ? '' : book.id)}
                  >
                    {t('Manage')}
                  </button>
                )}
              </div>
              {!!book.skipped && (
                <p className="text-muted-foreground">
                  {translate('contacts.addressBooksSkipped', {
                    count: book.skipped,
                    defaultValue: '{{count}} unreadable vCards skipped.',
                  })}
                </p>
              )}
              {book.enabled && openBook === book.id && (
                <BookContacts
                  bookId={book.id}
                  onChange={() => {
                    onChange()
                    void reload().catch((err) => setError(String(err)))
                  }}
                  disabled={busy}
                  onBusy={setEditing}
                />
              )}
            </div>
          ))}
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className={button}
              disabled={locked}
              onClick={() => void run(() => invoke('carddav.sync', { account_id: account.id }))}
            >
              {t('Sync')}
            </button>
            {account.google && googleConfigured && (
              <button type="button" className={button} disabled={locked} onClick={() => connectGoogle(account)}>
                {t('ReconnectGoogle')}
              </button>
            )}
            <button
              type="button"
              className={button}
              disabled={locked}
              onClick={() => void run(() => invoke('carddav.remove', { account_id: account.id }))}
            >
              {t('Disconnect')}
            </button>
          </div>
        </section>
      ))}
      {googleConfigured && (
        <button type="button" className={button} disabled={locked} onClick={() => connectGoogle()}>
          {t('ConnectGoogle')}
        </button>
      )}
      <form onSubmit={connect} className="space-y-2">
        <input
          name="server"
          required
          className={field}
          placeholder="https://dav.example.test"
          aria-label={t('Server')}
        />
        <input
          name="username"
          required
          className={field}
          placeholder={t('Username')}
          aria-label={t('Username')}
          autoComplete="username"
        />
        <input
          type="password"
          required
          className={field}
          placeholder={t('Password')}
          aria-label={t('Password')}
          autoComplete="current-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <button type="submit" className={button} disabled={locked}>
          {t('Connect')}
        </button>
      </form>
    </div>
  )
}

const labels: Record<string, string> = {
  Description: 'Sync saved contacts and manage them in their address book. Edits are saved to the server.',
  Manage: 'Manage contacts',
  Working: 'Updating address books…',
  Sync: 'Sync now',
  Disconnect: 'Disconnect',
  Server: 'CardDAV server URL',
  Username: 'Username',
  Password: 'Password',
  Connect: 'Connect CardDAV',
  ConnectGoogle: 'Connect Google Contacts',
  ReconnectGoogle: 'Reconnect Google Contacts',
}
