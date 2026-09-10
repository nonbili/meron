import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { ArrowLeft, Check, Copy, KeyRound, Pencil, Plus, Trash2 } from 'lucide-react'
import { accounts$ } from '../../states/accounts'
import { confirmAction, showToast } from '../../states/ui'
import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'
import { Button } from '../button/Button'
import { TextInput } from '../field/Field'
import { NumberRow, SegmentedRow, SettingRow, SettingsGroup, ToggleRow } from './AccountSettingsRows'

type Client = {
  id: string
  name: string
  accounts: string[]
  all_accounts: boolean
  drafts: boolean
  manage_accounts: boolean
  manage_settings: boolean
  organize: boolean
  send: boolean
  delete: boolean
  send_without_confirmation: boolean
  delete_without_confirmation: boolean
}
type Status = {
  enabled: boolean
  running: boolean
  port: number
  url: string
  error: string
  clients: Client[]
  activity: { time: string; client: string; tool: string; account: string; ok: boolean }[]
}
const emptyClient = (): Client => ({
  id: '',
  name: '',
  accounts: [],
  all_accounts: false,
  drafts: false,
  manage_accounts: false,
  manage_settings: false,
  organize: false,
  send: false,
  delete: false,
  send_without_confirmation: false,
  delete_without_confirmation: false,
})

export function McpSettingsPanel() {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const [status, setStatus] = useState<Status | null>(null)
  // null while no client is being approved or edited: the form is a step, not
  // a permanent wall of switches under the list it belongs to.
  const [editor, setEditor] = useState<Client | null>(null)
  const [credential, setCredential] = useState('')
  // Held only while the field is being edited, so a poll cannot overwrite a
  // half-typed port.
  const [portDraft, setPortDraft] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const revision = useRef(0)
  const changing = useRef(false)
  const emailOf = (id: string) => accounts.find((account) => account.id === id)?.email || id

  useEffect(() => {
    let active = true
    const refresh = () => {
      if (changing.current) return Promise.resolve()
      const currentRevision = revision.current
      return invoke<Status>('mcp.status').then(
        (value) => {
          if (active && currentRevision === revision.current) setStatus(value)
        },
        (err) => {
          if (active && currentRevision === revision.current) setError(String(err))
        },
      )
    }
    void refresh()
    // Tool calls announce themselves, so the panel follows activity without a
    // request every few seconds. The slow interval only covers what no event
    // reports, such as a listener that died on its own.
    const eventsOn = (window as any).runtime?.EventsOn
    // Every tool call reports itself, and a client can make several in a row,
    // so a burst collapses into one refresh.
    let debounce: ReturnType<typeof setTimeout> | undefined
    const unsubscribe =
      typeof eventsOn === 'function'
        ? eventsOn('mcp.activity', () => {
            clearTimeout(debounce)
            debounce = setTimeout(() => void refresh(), 400)
          })
        : undefined
    const timer = setInterval(() => void refresh(), unsubscribe ? 60000 : 5000)
    return () => {
      active = false
      clearTimeout(debounce)
      clearInterval(timer)
      if (typeof unsubscribe === 'function') unsubscribe()
    }
  }, [])

  async function run(action: () => Promise<void>) {
    if (changing.current) return
    changing.current = true
    revision.current++
    setBusy(true)
    setError('')
    try {
      await action()
    } catch (err) {
      setError(String(err))
    } finally {
      changing.current = false
      setBusy(false)
    }
  }

  async function save(client: Client) {
    const result = await invoke<{ status: Status; token: string }>('mcp.clientSave', client)
    setStatus(result.status)
    if (result.token) setCredential(configurationFor(result.status.url, result.token))
    setEditor(null)
  }

  const copy = (value: string) => {
    navigator.clipboard
      ?.writeText(value)
      .then(() => showToast(t('common.copied')))
      .catch(() => undefined)
  }

  const commitPort = (status: Status) => {
    const value = Number(portDraft)
    const changed = portDraft !== null && Number.isInteger(value) && value !== status.port
    setPortDraft(null)
    if (!changed || value < 1024 || value > 65535) return
    void run(async () => {
      setStatus(await invoke<Status>('mcp.setPort', { port: value }))
    })
  }

  // The plaintext credential is shown once and never stored, so a lost or
  // leaked secret is replaced in place instead of costing the whole grant.
  const regenerate = async (client: Client) => {
    if (
      !(await confirmAction({
        title: t('mcp.regenerateTitle', { name: client.name }),
        message: t('mcp.regenerateMessage'),
        confirmLabel: t('mcp.regenerate'),
        tone: 'danger',
      }))
    ) {
      return
    }
    void run(async () => {
      const result = await invoke<{ status: Status; token: string }>('mcp.clientRegenerate', { id: client.id })
      setStatus(result.status)
      setCredential(configurationFor(result.status.url, result.token))
    })
  }

  const revoke = async (client: Client) => {
    if (
      !(await confirmAction({
        title: t('mcp.revokeTitle', { name: client.name }),
        message: t('mcp.revokeMessage'),
        confirmLabel: t('mcp.revoke'),
        tone: 'danger',
      }))
    ) {
      return
    }
    void run(async () => {
      setStatus(await invoke<Status>('mcp.clientRevoke', { id: client.id }))
      setCredential('')
      setEditor((current) => (current?.id === client.id ? null : current))
    })
  }

  if (!status) {
    return <p className="text-xs text-secondary">{t('common.loading')}</p>
  }

  {
    /* A failed enable reports the listener error twice: once as the rejected
      call, once as the stored startup error. Show it once. */
  }
  const banners = [status.error, error && !error.includes(status.error || '\u0000') ? error : '']
    .filter(Boolean)
    .map((message) => (
      <p key={message} role="alert" className="rounded-xl bg-rose-500/10 px-3.5 py-2 text-xs text-rose-500">
        {message}
      </p>
    ))

  // Approving or editing takes over the panel, the way an account does: the
  // list stays an overview, and a long account list has room to breathe.
  if (editor) {
    return (
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          void run(() => save(editor))
        }}
      >
        <div className="flex items-center gap-2">
          <button
            type="button"
            aria-label={t('buttons.back')}
            title={t('buttons.back')}
            onClick={() => setEditor(null)}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-active hover:text-primary cursor-pointer"
          >
            <ArrowLeft size={16} />
          </button>
          <h2 className="truncate text-base font-bold leading-tight tracking-tight">
            {editor.id ? t('mcp.editClient') : t('mcp.addClient')}
          </h2>
        </div>
        {banners}
        <SettingsGroup title={t('mcp.name')}>
          <div className="px-3.5 py-2.5">
            <TextInput
              required
              autoFocus
              maxLength={80}
              aria-label={t('mcp.name')}
              value={editor.name}
              onChange={(event) => setEditor({ ...editor, name: event.target.value })}
              className="w-full font-semibold"
            />
          </div>
        </SettingsGroup>
        <SettingsGroup title={t('mcp.accounts')}>
          <ToggleRow
            title={t('mcp.allAccounts')}
            checked={editor.all_accounts}
            onChange={() => setEditor({ ...editor, all_accounts: !editor.all_accounts })}
          />
          {!editor.all_accounts && (
            <>
              {accounts.map((account) => (
                <ToggleRow
                  key={account.id}
                  title={account.email || account.display_name}
                  checked={editor.accounts.includes(account.id)}
                  onChange={() =>
                    setEditor({
                      ...editor,
                      accounts: editor.accounts.includes(account.id)
                        ? editor.accounts.filter((id) => id !== account.id)
                        : [...editor.accounts, account.id],
                    })
                  }
                />
              ))}
              <div className="flex items-center justify-between gap-3 px-3.5 py-2">
                <p className="text-[0.6875rem] text-secondary">{t('mcp.newAccounts')}</p>
                <button
                  type="button"
                  onClick={() => setEditor({ ...editor, accounts: accounts.map((account) => account.id) })}
                  className="shrink-0 text-xs font-semibold text-accent hover:underline cursor-pointer"
                >
                  {t('mcp.selectAll')}
                </button>
              </div>
            </>
          )}
        </SettingsGroup>
        <SettingsGroup title={t('mcp.configurationPermissions')}>
          <ToggleRow
            title={t('mcp.manageAccounts')}
            hint={t('mcp.manageAccountsPermission')}
            checked={editor.manage_accounts}
            onChange={() => setEditor({ ...editor, manage_accounts: !editor.manage_accounts })}
          />
          <ToggleRow
            title={t('mcp.manageSettings')}
            hint={t('mcp.manageSettingsPermission')}
            checked={editor.manage_settings}
            onChange={() => setEditor({ ...editor, manage_settings: !editor.manage_settings })}
          />
        </SettingsGroup>
        <SettingsGroup title={t('mcp.permissions')}>
          <fieldset
            disabled={!editor.all_accounts && !editor.accounts.length}
            className="m-0 min-w-0 border-0 p-0 disabled:opacity-50"
          >
            <SettingRow
              title={t('mcp.read')}
              hint={t('mcp.readPermission')}
              control={<span className="text-xs font-semibold text-secondary">{t('mcp.always')}</span>}
            />
            <ToggleRow
              title={t('mcp.organize')}
              hint={t('mcp.organizePermission')}
              checked={editor.organize}
              onChange={() => setEditor({ ...editor, organize: !editor.organize })}
            />
            <ToggleRow
              title={t('chat.draft')}
              hint={t('mcp.drafts')}
              checked={editor.drafts}
              onChange={() => setEditor({ ...editor, drafts: !editor.drafts })}
            />
            <ToggleRow
              title={t('buttons.send')}
              hint={t('mcp.sendPermission')}
              checked={editor.send}
              onChange={() => setEditor({ ...editor, send: !editor.send, send_without_confirmation: false })}
            />
            {editor.send && (
              <SegmentedRow
                title={t('mcp.sendConfirmation')}
                hint={t('mcp.confirmationHint')}
                value={editor.send_without_confirmation ? 'allow' : 'ask'}
                options={[
                  { value: 'ask', label: t('mcp.ask') },
                  { value: 'allow', label: t('mcp.withoutAsking') },
                ]}
                onChange={(value) => setEditor({ ...editor, send_without_confirmation: value === 'allow' })}
              />
            )}
            <ToggleRow
              title={t('mcp.delete')}
              hint={t('mcp.deletePermission')}
              checked={editor.delete}
              onChange={() => setEditor({ ...editor, delete: !editor.delete, delete_without_confirmation: false })}
            />
            {editor.delete && (
              <SegmentedRow
                title={t('mcp.deleteConfirmation')}
                hint={t('mcp.confirmationHint')}
                value={editor.delete_without_confirmation ? 'allow' : 'ask'}
                options={[
                  { value: 'ask', label: t('mcp.ask') },
                  { value: 'allow', label: t('mcp.withoutAsking') },
                ]}
                onChange={(value) => setEditor({ ...editor, delete_without_confirmation: value === 'allow' })}
              />
            )}
          </fieldset>
        </SettingsGroup>
        <div className="flex items-center justify-end gap-2">
          <Button type="button" variant="ghost" onClick={() => setEditor(null)}>
            {t('buttons.cancel')}
          </Button>
          <Button
            type="submit"
            variant="primary"
            disabled={
              busy ||
              !editor.name.trim() ||
              (!editor.all_accounts && !editor.accounts.length && !editor.manage_accounts && !editor.manage_settings)
            }
          >
            {editor.id ? t('mcp.save') : t('mcp.approve')}
          </Button>
        </div>
      </form>
    )
  }

  return (
    <div className="flex flex-col gap-4">
      {banners}

      <SettingsGroup title={t('mcp.localServer')}>
        <ToggleRow
          title={t('mcp.enable')}
          hint={t('mcp.description')}
          checked={status.enabled}
          onChange={() =>
            void run(async () => {
              setStatus(await invoke<Status>('mcp.enable', { enabled: !status.enabled }))
            })
          }
        />
        <NumberRow
          title={t('accounts.fields.port')}
          hint={t('mcp.portHint')}
          value={portDraft ?? String(status.port)}
          min={1024}
          max={65535}
          step={1}
          suffix=""
          onChange={setPortDraft}
          onBlur={() => commitPort(status)}
        />
        {status.running && (
          <SettingRow
            title={t('mcp.endpoint')}
            control={
              <div className="flex items-center gap-1.5">
                <span className="font-mono text-[0.6875rem] text-secondary select-text">{status.url}</span>
                <button
                  type="button"
                  aria-label={t('common.copy')}
                  title={t('common.copy')}
                  onClick={() => copy(status.url)}
                  className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-active hover:text-primary cursor-pointer"
                >
                  <Copy size={13} />
                </button>
              </div>
            }
          />
        )}
        {status.enabled && !status.running && (
          <SettingRow
            title={t('mcp.notRunning')}
            control={
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    setStatus(await invoke<Status>('mcp.enable', { enabled: true }))
                  })
                }
              >
                {t('mcp.retry')}
              </Button>
            }
          />
        )}
      </SettingsGroup>

      <SettingsGroup title={t('mcp.clients')}>
        {status.clients.length === 0 && <p className="px-3.5 py-3 text-xs text-secondary">{t('mcp.noClients')}</p>}
        {status.clients.map((client) => (
          <div key={client.id} className="flex items-start gap-3 px-3.5 py-2.5">
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs font-semibold text-primary">{client.name}</p>
              <p className="mt-0.5 truncate text-[0.6875rem] text-secondary">
                {client.all_accounts ? t('mcp.allAccounts') : client.accounts.map(emailOf).join(', ')}
              </p>
              <div className="mt-1.5 flex flex-wrap gap-1">
                {(client.all_accounts || client.accounts.length > 0) && (
                  <Chip label={t('mcp.read')} unconfirmedLabel={t('mcp.withoutAsking')} />
                )}
                {client.manage_accounts && (
                  <Chip label={t('mcp.manageAccounts')} unconfirmedLabel={t('mcp.withoutAsking')} />
                )}
                {client.manage_settings && (
                  <Chip label={t('mcp.manageSettings')} unconfirmedLabel={t('mcp.withoutAsking')} />
                )}
                {(client.all_accounts || client.accounts.length > 0) && client.organize && (
                  <Chip label={t('mcp.organize')} unconfirmedLabel={t('mcp.withoutAsking')} />
                )}
                {(client.all_accounts || client.accounts.length > 0) && client.drafts && (
                  <Chip label={t('chat.draft')} unconfirmedLabel={t('mcp.withoutAsking')} />
                )}
                {(client.all_accounts || client.accounts.length > 0) && client.send && (
                  <Chip
                    label={t('buttons.send')}
                    unconfirmed={client.send_without_confirmation}
                    unconfirmedLabel={t('mcp.withoutAsking')}
                  />
                )}
                {(client.all_accounts || client.accounts.length > 0) && client.delete && (
                  <Chip
                    label={t('mcp.delete')}
                    unconfirmed={client.delete_without_confirmation}
                    unconfirmedLabel={t('mcp.withoutAsking')}
                    danger
                  />
                )}
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <button
                type="button"
                aria-label={t('buttons.edit')}
                title={t('buttons.edit')}
                disabled={busy}
                onClick={() => {
                  setEditor({ ...emptyClient(), ...client })
                  setCredential('')
                }}
                className="flex h-7 w-7 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-active hover:text-primary cursor-pointer"
              >
                <Pencil size={13} />
              </button>
              <button
                type="button"
                aria-label={t('mcp.regenerate')}
                title={t('mcp.regenerate')}
                disabled={busy}
                onClick={() => void regenerate(client)}
                className="flex h-7 w-7 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-active hover:text-primary cursor-pointer"
              >
                <KeyRound size={13} />
              </button>
              <button
                type="button"
                aria-label={t('mcp.revoke')}
                title={t('mcp.revoke')}
                disabled={busy}
                onClick={() => void revoke(client)}
                className="flex h-7 w-7 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-active hover:text-rose-500 cursor-pointer"
              >
                <Trash2 size={13} />
              </button>
            </div>
          </div>
        ))}
        {!editor && (
          <div className="px-3.5 py-2">
            <button
              type="button"
              onClick={() => {
                setEditor(emptyClient())
                setCredential('')
              }}
              className="flex items-center gap-1 text-xs font-semibold text-accent hover:underline cursor-pointer"
            >
              <Plus size={12} /> {t('mcp.addClient')}
            </button>
          </div>
        )}
      </SettingsGroup>

      {credential && (
        <SettingsGroup title={t('mcp.configuration')}>
          <div className="flex flex-col gap-2.5 px-3.5 py-3">
            <p className="text-xs leading-relaxed text-secondary">{t('mcp.credentialHint')}</p>
            <textarea
              aria-label={t('mcp.configuration')}
              readOnly
              value={credential}
              rows={11}
              onFocus={(event) => event.target.select()}
              className="w-full rounded-xl border border-border bg-chats p-2.5 font-mono text-[0.6875rem] leading-relaxed text-primary select-text"
            />
            <div className="flex items-center justify-end gap-2">
              <Button variant="ghost" onClick={() => setCredential('')}>
                {t('mcp.dismiss')}
              </Button>
              <Button variant="primary" leftIcon={Copy} onClick={() => copy(credential)}>
                {t('common.copy')}
              </Button>
            </div>
          </div>
        </SettingsGroup>
      )}

      <SettingsGroup title={t('mcp.activity')}>
        {status.activity.length === 0 ? (
          <p className="px-3.5 py-3 text-xs text-secondary">{t('mcp.noActivity')}</p>
        ) : (
          <div className="max-h-56 divide-y divide-border/40 overflow-y-auto">
            {status.activity
              .slice()
              .reverse()
              .map((entry, index) => (
                <div key={index} className="flex items-center gap-2.5 px-3.5 py-2 select-text">
                  <span
                    aria-label={entry.ok ? t('mcp.success') : t('mcp.failed')}
                    title={entry.ok ? t('mcp.success') : t('mcp.failed')}
                    className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full ${
                      entry.ok ? 'bg-emerald-500/15 text-emerald-500' : 'bg-rose-500/15 text-rose-500'
                    }`}
                  >
                    {entry.ok ? <Check size={10} /> : <span className="text-[0.625rem] font-bold">!</span>}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-xs text-primary">
                      <span className="font-mono">{entry.tool}</span> · {entry.client}
                    </p>
                    <p className="truncate text-[0.6875rem] text-secondary">{emailOf(entry.account)}</p>
                  </div>
                  <span className="shrink-0 text-[0.6875rem] text-secondary">
                    {new Date(entry.time).toLocaleTimeString()}
                  </span>
                </div>
              ))}
          </div>
        )}
      </SettingsGroup>
    </div>
  )
}

function configurationFor(url: string, token: string) {
  return JSON.stringify(
    { mcpServers: { meron: { type: 'http', url, headers: { Authorization: `Bearer ${token}` } } } },
    null,
    2,
  )
}

// One granted permission. Write permissions that skip the in-app prompt are
// marked, so a glance at the list shows which clients act without asking.
function Chip({
  label,
  unconfirmed,
  danger,
  unconfirmedLabel,
}: {
  label: string
  unconfirmed?: boolean
  danger?: boolean
  unconfirmedLabel: string
}) {
  const tone = unconfirmed
    ? 'bg-amber-500/15 text-amber-600 dark:text-amber-400'
    : danger
      ? 'bg-rose-500/10 text-rose-500'
      : 'bg-active text-secondary'
  return (
    <span className={`rounded-md px-1.5 py-0.5 text-[0.625rem] font-semibold ${tone}`}>
      {unconfirmed ? `${label} · ${unconfirmedLabel}` : label}
    </span>
  )
}
