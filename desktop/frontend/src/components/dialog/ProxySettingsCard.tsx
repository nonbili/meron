import { useEffect, useState } from 'react'
import { Network } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { settings$, isProxyUsable, type ProxyMode, type ProxySettings } from '../../states/settings'
import { setAccountProxy } from '../../states/accounts'
import type { Account, AccountProxy } from '../../types'
import { NumberRow, SelectRow, SettingsGroup, TextRow } from './AccountSettingsRows'

/** The endpoint fields, shared by the app-wide proxy and the per-account one. */
type Endpoint = {
  host: string
  port: number
  username: string
  password: string
}

function EndpointRows({ value, onChange }: { value: Endpoint; onChange: (next: Endpoint) => void }) {
  const { t } = useTranslation()
  return (
    <>
      <TextRow
        title={t('settings.network.host')}
        value={value.host}
        placeholder="127.0.0.1"
        onChange={(host) => onChange({ ...value, host })}
      />
      <NumberRow
        title={t('accounts.fields.port')}
        // 0 is the "unset" sentinel the core reads as "no proxy"; show it blank
        // rather than as a port nobody can connect to.
        value={value.port ? String(value.port) : ''}
        min={1}
        max={65535}
        step={1}
        suffix=""
        onChange={(raw) => {
          const port = Number.parseInt(raw, 10)
          onChange({ ...value, port: Number.isFinite(port) && port > 0 ? Math.min(port, 65535) : 0 })
        }}
      />
      <TextRow
        title={t('accounts.fields.username')}
        value={value.username}
        placeholder={t('settings.network.optional')}
        onChange={(username) => onChange({ ...value, username })}
      />
      <TextRow
        title={t('accounts.fields.password')}
        type="password"
        value={value.password}
        placeholder={t('settings.network.optional')}
        onChange={(password) => onChange({ ...value, password })}
      />
    </>
  )
}

/**
 * The app-wide proxy. Covers mail connections (IMAP and SMTP) plus feed
 * fetches, avatar downloads and OAuth token calls. Accounts follow it unless
 * they carry their own override.
 */
export function ProxySettingsSection() {
  const { t } = useTranslation()
  const proxy = useValue(settings$.proxy)
  const incomplete = proxy.mode !== 'off' && !isProxyUsable(proxy)

  return (
    <SettingsGroup title={t('settings.sections.network')}>
      <SelectRow
        icon={<Network size={15} />}
        title={t('settings.network.proxy')}
        hint={t('settings.network.proxyHint')}
        value={proxy.mode}
        options={[
          { value: 'off', label: t('settings.network.modeOff') },
          { value: 'http', label: t('settings.network.modeHttp') },
          { value: 'socks5', label: t('settings.network.modeSocks5') },
        ]}
        onChange={(mode) => settings$.proxy.mode.set(mode as ProxyMode)}
      />
      {proxy.mode !== 'off' && (
        <EndpointRows
          value={proxy}
          onChange={(next) => settings$.proxy.set({ ...next, mode: proxy.mode } as ProxySettings)}
        />
      )}
      {incomplete && <p className="px-3.5 py-2 text-[0.6875rem] text-secondary">{t('settings.network.incomplete')}</p>}
    </SettingsGroup>
  )
}

const EMPTY_ENDPOINT: Endpoint = { host: '', port: 0, username: '', password: '' }

function toEndpoint(proxy: AccountProxy | undefined): Endpoint {
  return {
    host: proxy?.host ?? '',
    port: proxy?.port ?? 0,
    username: proxy?.username ?? '',
    password: proxy?.password ?? '',
  }
}

/**
 * Per-account override. New connections pick the change up; sessions already
 * open keep their current socket until they reconnect.
 */
export function AccountProxyCard({ account }: { account: Account }) {
  const { t } = useTranslation()
  const [mode, setMode] = useState<AccountProxy['mode']>('global')
  const [endpoint, setEndpoint] = useState<Endpoint>(EMPTY_ENDPOINT)

  // Seed when switching accounts only, so a half-typed host isn't overwritten
  // by the account list refreshing after a save.
  useEffect(() => {
    setMode(account.proxy?.mode ?? 'global')
    setEndpoint(toEndpoint(account.proxy))
  }, [account.id])

  const save = (nextMode: AccountProxy['mode'], nextEndpoint: Endpoint) => {
    setMode(nextMode)
    setEndpoint(nextEndpoint)
    const payload: AccountProxy =
      nextMode === 'global' || nextMode === 'direct' ? { mode: nextMode } : { mode: nextMode, ...nextEndpoint }
    void setAccountProxy(account.id, payload)
  }

  const custom = mode === 'http' || mode === 'socks5'
  const incomplete = custom && (!endpoint.host.trim() || !endpoint.port)

  return (
    <SettingsGroup title={t('settings.sections.network')}>
      <SelectRow
        icon={<Network size={15} />}
        title={t('settings.network.proxy')}
        hint={t('settings.network.accountProxyHint')}
        value={mode}
        options={[
          { value: 'global', label: t('settings.network.modeGlobal') },
          { value: 'direct', label: t('settings.network.modeDirect') },
          { value: 'http', label: t('settings.network.modeHttp') },
          { value: 'socks5', label: t('settings.network.modeSocks5') },
        ]}
        onChange={(next) => save(next as AccountProxy['mode'], endpoint)}
      />
      {custom && <EndpointRows value={endpoint} onChange={(next) => save(mode, next)} />}
      {incomplete && <p className="px-3.5 py-2 text-[0.6875rem] text-secondary">{t('settings.network.incomplete')}</p>}
    </SettingsGroup>
  )
}
