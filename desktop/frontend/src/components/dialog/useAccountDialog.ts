import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { boot } from '../../boot'
import { ui$, type SetupMode } from '../../states/ui'
import { accounts$ } from '../../states/accounts'
import { openMailAccount } from '../../states/kanban'
import { nextRssAccountDisplayName } from '../../states/feeds'
import { errorMessage } from '../../lib/errors'
import { securityForPort, serverSelectionAfterDiscovery, type MailSecurity } from './accountSecurity'
import {
  certificateProbePayload,
  shouldPromptForCertificate,
  untrustedCertificateProtocol,
  type CertificateInfo,
  type CertificateProtocol,
  type CertificatePrompt,
} from '../../lib/certificateTrust'

type AddAccountResult = { account?: { id?: string } }

// All of the account-setup dialog's state and backend flows: provider OAuth
// (begin + poll), IMAP/SMTP autodiscovery, and the final save. Returned to the
// dialog and its mode-specific form sections, which stay presentational.
export function useAccountDialog() {
  const mode = useValue(ui$.setupMode)
  const system = useValue(ui$.system)
  const reconnectAccountId = useValue(ui$.reconnectAccountId)
  const accounts = useValue(accounts$)
  const reconnectAccount = accounts.find((account) => account.id === reconnectAccountId) ?? null
  // The same dialog serves two jobs on an existing account. A *reconnect* is
  // repairing a missing keychain credential, so the password is the whole point
  // and must be retyped. *Editing* server settings is the ordinary case — the
  // credential is intact, the UI never holds it, and asking for it again to
  // change a port would be busywork the user cannot always satisfy.
  const editing = !!reconnectAccount && reconnectAccount.needs_reconnect !== true
  const gmailConfigured = !!system?.gmail_oauth_configured
  const outlookConfigured = !!system?.outlook_oauth_configured

  // OAuth providers (Gmail, Outlook) share one sign-in flow; these resolve the
  // active provider's command names, labels, and config gate.
  const oauthProvider: 'gmail' | 'outlook' = mode === 'outlook' ? 'outlook' : 'gmail'
  const oauthConfigured = mode === 'outlook' ? outlookConfigured : gmailConfigured
  const oauthLabel = mode === 'outlook' ? 'Microsoft' : 'Google'

  const [form, setForm] = useState({
    email: '',
    display_name: '',
    sender_name: '',
    imap_host: '',
    imap_host_touched: false,
    imap_port: '993',
    imap_port_touched: false,
    imap_security: 'tls' as MailSecurity,
    imap_security_touched: false,
    smtp_host: '',
    smtp_host_touched: false,
    smtp_port: '465',
    smtp_port_touched: false,
    smtp_security: 'tls' as MailSecurity,
    smtp_security_touched: false,
    username: '',
    username_touched: false,
    password: '',
    auth_code: '',
    feed_url: '',
  })
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [discovering, setDiscovering] = useState(false)
  const [discoverNote, setDiscoverNote] = useState('')
  const [appPasswordHint, setAppPasswordHint] = useState<{ provider: string; url: string } | null>(null)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [waitingForGoogle, setWaitingForGoogle] = useState(false)
  const [certPrompt, setCertPrompt] = useState<CertificatePrompt | null>(null)
  // Pins accepted during the save in flight. A bridge can refuse on IMAP and
  // then again on submission, and the second prompt must not drop the first.
  const pins = useRef<{ imap?: string; smtp?: string }>({})
  const autoBeginOAuthKeyRef = useRef('')
  const rssAutoNameRef = useRef('')
  const discoveryGenerationRef = useRef(0)
  const formRef = useRef(form)
  formRef.current = form
  const [exchangedTokens, setExchangedTokens] = useState<null | {
    access_token: string
    refresh_token: string
    expires_in: number
  }>(null)

  const selectCreatedAccount = (before: Set<string>, createdId?: string) => {
    const accounts = accounts$.peek()
    const created =
      (createdId ? accounts.find((acc) => acc.id === createdId) : null) ?? accounts.find((acc) => !before.has(acc.id))
    if (!created) return null
    openMailAccount(created.id)
    ui$.selectedThread.set('')
    return created
  }

  useEffect(() => {
    return () => {
      if ((window as any)._oauthPollInterval) {
        clearInterval((window as any)._oauthPollInterval)
        ;(window as any)._oauthPollInterval = null
      }
    }
  }, [])

  useEffect(() => {
    if (!reconnectAccount) return
    const imapPort = reconnectAccount.imap_port || 993
    const imapSecurity: MailSecurity = reconnectAccount.starttls ? 'starttls' : reconnectAccount.tls ? 'tls' : 'none'
    const smtpPort = reconnectAccount.smtp_port || 465
    const smtpSecurity: MailSecurity = reconnectAccount.smtp_starttls
      ? 'starttls'
      : reconnectAccount.smtp_tls === false
        ? 'none'
        : 'tls'
    setForm({
      email: reconnectAccount.email,
      display_name: reconnectAccount.display_name || '',
      sender_name: reconnectAccount.sender_name || '',
      imap_host: reconnectAccount.imap_host || '',
      imap_host_touched: !!reconnectAccount.imap_host,
      imap_port: String(imapPort),
      imap_port_touched: reconnectAccount.imap_port > 0,
      imap_security: imapSecurity,
      imap_security_touched: imapSecurity !== securityForPort(imapPort),
      smtp_host: reconnectAccount.smtp_host || '',
      smtp_host_touched: !!reconnectAccount.smtp_host,
      smtp_port: String(smtpPort),
      smtp_port_touched: reconnectAccount.smtp_port > 0,
      smtp_security: smtpSecurity,
      smtp_security_touched: smtpSecurity !== securityForPort(smtpPort),
      // Not always the address — a server can use a separate login, and
      // resending the account must preserve it.
      username: reconnectAccount.username || reconnectAccount.email,
      username_touched: true,
      password: '',
      auth_code: '',
      feed_url: reconnectAccount.feed_url || '',
    })
    setError('')
    setExchangedTokens(null)
    setAppPasswordHint(null)
    setDiscoverNote('')
    setAdvancedOpen(reconnectAccount.auth_type === 'password')
  }, [reconnectAccount])

  // Prefill a default name for RSS accounts so creating one is a single click;
  // the feed URL stays optional (feeds can be added from the account afterwards).
  useEffect(() => {
    if (reconnectAccount) return
    if (mode === 'rss') {
      setForm((f) => {
        if (f.display_name) return f
        const display_name = nextRssAccountDisplayName(accounts)
        rssAutoNameRef.current = display_name
        return { ...f, display_name }
      })
      return
    }
    // Leaving the RSS tab: drop the name we filled in, so it doesn't carry over
    // into the mail forms. A name the user typed themselves stays put.
    setForm((f) => (f.display_name && f.display_name === rssAutoNameRef.current ? { ...f, display_name: '' } : f))
    rssAutoNameRef.current = ''
  }, [mode, reconnectAccount, accounts])

  const setMode = (newMode: SetupMode) => {
    ui$.reconnectAccountId.set('')
    ui$.setupMode.set(newMode)
    setAdvancedOpen(false)
    setAppPasswordHint(null)
  }

  async function pollProfile(intervalId: any, provider: 'gmail' | 'outlook') {
    const addCommand = provider === 'outlook' ? 'account.addOutlookOAuth' : 'account.addGmailOAuth'
    const providerLabel = provider === 'outlook' ? 'Microsoft' : 'Google'
    try {
      const res = await invoke<{
        exchanged: boolean
        profile?: {
          email: string
          display_name: string
          avatar_url: string
          access_token: string
          refresh_token: string
          expires_in: number
          auth_code: string
        }
      }>(`oauth.${provider}PollProfile`)

      if (res.exchanged && res.profile) {
        const before = new Set(accounts$.peek().map((acc) => acc.id))
        clearInterval(intervalId)
        ;(window as any)._oauthPollInterval = null
        setWaitingForGoogle(false)
        setLoading(true)
        setForm((f) => ({
          ...f,
          email: res.profile!.email,
          display_name: res.profile!.display_name,
          auth_code: res.profile!.auth_code,
        }))
        setExchangedTokens({
          access_token: res.profile!.access_token,
          refresh_token: res.profile!.refresh_token,
          expires_in: res.profile!.expires_in,
        })
        const added = await invoke<AddAccountResult>(addCommand, {
          email: res.profile.email,
          display_name: res.profile.display_name,
          sender_name: res.profile.display_name,
          avatar_url: res.profile.avatar_url,
          auth_code: res.profile.auth_code,
          access_token: res.profile.access_token,
          refresh_token: res.profile.refresh_token,
          expires_in: res.profile.expires_in,
        })
        ui$.reconnectAccountId.set('')
        ui$.setupOpen.set(false)
        await boot()
        selectCreatedAccount(before, added.account?.id)
      }
    } catch (err) {
      console.error('Failed to poll profile', err)
      setWaitingForGoogle(false)
      setError(errorMessage(err, `${providerLabel} sign-in completed, but account save failed`))
    } finally {
      setLoading(false)
    }
  }

  async function beginOAuth(provider: 'gmail' | 'outlook' = oauthProvider) {
    const providerLabel = provider === 'outlook' ? 'Microsoft' : 'Google'
    try {
      setError('')
      ui$.setupMode.set(provider)
      setWaitingForGoogle(true)
      const res = await invoke<{ url: string; needs_external_browser?: boolean }>(`oauth.${provider}Begin`)
      if (res.url && !res.needs_external_browser) {
        window.location.href = res.url
      }
      if (res.url) {
        const id = setInterval(() => {
          void pollProfile(id, provider)
        }, 1000)
        ;(window as any)._oauthPollInterval = id
      } else {
        setWaitingForGoogle(false)
      }
    } catch (err) {
      setWaitingForGoogle(false)
      setError(errorMessage(err, `Failed to begin ${providerLabel} sign in`))
    }
  }

  useEffect(() => {
    if (!reconnectAccount || (mode !== 'gmail' && mode !== 'outlook') || !oauthConfigured || waitingForGoogle) return
    const key = `${reconnectAccount.id}:${mode}`
    if (autoBeginOAuthKeyRef.current === key) return
    autoBeginOAuthKeyRef.current = key
    void beginOAuth()
  }, [reconnectAccount, mode, oauthConfigured, waitingForGoogle])

  async function runDiscovery(email: string) {
    if (!email.includes('@') || email.endsWith('@')) return
    const requestGeneration = ++discoveryGenerationRef.current
    setDiscoverNote('')
    setAppPasswordHint(null)
    setDiscovering(true)
    try {
      const cfg = await invoke<{
        imap_host: string
        imap_port: number
        smtp_host: string
        smtp_port: number
        username: string
        provider_name?: string
        source: string
        app_password_hint?: { provider: string; url: string }
      }>('account.autodiscover', { email })
      if (
        requestGeneration !== discoveryGenerationRef.current ||
        formRef.current.email.trim().toLowerCase() !== email.trim().toLowerCase()
      ) {
        return
      }
      setForm((f) => {
        const imap = serverSelectionAfterDiscovery(
          f.imap_port,
          f.imap_security,
          f.imap_security_touched,
          f.imap_host_touched,
          f.imap_port_touched,
          cfg.imap_host ? cfg.imap_port : 0,
        )
        const smtp = serverSelectionAfterDiscovery(
          f.smtp_port,
          f.smtp_security,
          f.smtp_security_touched,
          f.smtp_host_touched,
          f.smtp_port_touched,
          cfg.smtp_host ? cfg.smtp_port : 0,
        )
        return {
          ...f,
          imap_host: f.imap_host_touched || !cfg.imap_host ? f.imap_host : cfg.imap_host,
          imap_port: imap.port,
          imap_security: imap.security,
          smtp_host: f.smtp_host_touched || !cfg.smtp_host ? f.smtp_host : cfg.smtp_host,
          smtp_port: smtp.port,
          smtp_security: smtp.security,
          username: f.username_touched || !cfg.username ? f.username : cfg.username,
        }
      })
      setAppPasswordHint(cfg.app_password_hint ?? null)
      if (cfg.source === 'guess') {
        setAdvancedOpen(true)
        setDiscoverNote("Couldn't find settings automatically — please verify the servers below.")
      } else {
        setDiscoverNote(`Settings found${cfg.provider_name ? ` for ${cfg.provider_name}` : ''}.`)
      }
    } catch {
      if (requestGeneration === discoveryGenerationRef.current) setDiscoverNote('')
    } finally {
      if (requestGeneration === discoveryGenerationRef.current) setDiscovering(false)
    }
  }

  // A save validates both servers — IMAP first, then the submission server's
  // certificate — so either can be the one that was refused.
  function serverTarget(protocol: CertificateProtocol) {
    return protocol === 'smtp'
      ? {
          host: form.smtp_host.trim(),
          port: Number(form.smtp_port) || 465,
          starttls: form.smtp_security === 'starttls',
        }
      : {
          host: form.imap_host.trim(),
          port: Number(form.imap_port) || 993,
          starttls: form.imap_security === 'starttls',
        }
  }

  // Ask the core what certificate that server actually presented, so the user
  // has something to look at before deciding to pin it. Returns null when even
  // the probe cannot reach the server — then the original error stands.
  async function probeCertificate(protocol: CertificateProtocol): Promise<CertificatePrompt | null> {
    const target = serverTarget(protocol)
    try {
      const res = await invoke<{ certificate: CertificateInfo }>(
        'account.probeCert',
        certificateProbePayload({ ...target, protocol }, reconnectAccount?.proxy),
      )
      if (!res?.certificate?.fingerprint) return null
      return { host: target.host, port: target.port, protocol, certificate: res.certificate }
    } catch {
      return null
    }
  }

  // Pin the certificate the user just accepted and retry the save with it. The
  // other server's pin, if one was accepted earlier in this same save, rides
  // along — a bridge can present a certificate on each port.
  async function trustCertificate() {
    const accepted = certPrompt
    setCertPrompt(null)
    if (!accepted) return
    await submit(
      accepted.protocol === 'imap' ? accepted.certificate.fingerprint : pins.current.imap,
      accepted.protocol === 'smtp' ? accepted.certificate.fingerprint : pins.current.smtp,
    )
  }

  function dismissCertPrompt() {
    setCertPrompt(null)
  }

  async function save() {
    await submit()
  }

  async function submit(certPin?: string, smtpCertPin?: string) {
    pins.current = { imap: certPin, smtp: smtpCertPin }
    try {
      setError('')
      setCertPrompt(null)
      setLoading(true)
      // Snapshot existing ids so we can jump to the freshly added account below.
      const before = new Set(accounts$.peek().map((acc) => acc.id))
      let createdId = ''
      if (mode === 'gmail' || mode === 'outlook') {
        const added = await invoke<AddAccountResult>(
          mode === 'outlook' ? 'account.addOutlookOAuth' : 'account.addGmailOAuth',
          {
            email: form.email,
            display_name: form.display_name,
            sender_name: form.display_name,
            auth_code: form.auth_code,
            access_token: exchangedTokens?.access_token || null,
            refresh_token: exchangedTokens?.refresh_token || null,
            expires_in: exchangedTokens?.expires_in || null,
          },
        )
        createdId = added.account?.id ?? ''
      } else if (mode === 'rss') {
        const added = await invoke<AddAccountResult>('account.addRSS', {
          feed_url: form.feed_url,
          display_name: form.display_name,
        })
        createdId = added.account?.id ?? ''
      } else {
        const added = await invoke<AddAccountResult>('account.addPassword', {
          email: form.email,
          display_name: form.display_name,
          sender_name: form.sender_name,
          imap_host: form.imap_host,
          imap_port: Number(form.imap_port),
          smtp_host: form.smtp_host,
          smtp_port: Number(form.smtp_port),
          username: form.username || form.email,
          // Omitting the key entirely tells the core to keep the stored
          // password; sending "" would blank it. Only an explicitly typed
          // password replaces what the keychain already holds.
          ...(editing && !form.password ? {} : { password: form.password }),
          tls: form.imap_security === 'tls',
          starttls: form.imap_security === 'starttls',
          smtp_tls: form.smtp_security === 'tls',
          smtp_starttls: form.smtp_security === 'starttls',
          // A reconnect keeps the pins the account already carries unless the
          // user has just accepted a new certificate.
          cert_pin: certPin ?? reconnectAccount?.cert_pin ?? '',
          smtp_cert_pin: smtpCertPin ?? reconnectAccount?.smtp_cert_pin ?? '',
        })
        createdId = added.account?.id ?? ''
      }
      ui$.setupOpen.set(false)
      ui$.reconnectAccountId.set('')
      await boot()
      const created = selectCreatedAccount(before, createdId)
      // When the dialog was opened from within Settings, drop straight into the
      // new account's panel; boot() has refreshed accounts$, so the one id not in
      // the pre-save snapshot is the account we just created.
      if (ui$.settingsOpen.peek()) {
        if (created) ui$.accountSettingsId.set(created.id)
      }
    } catch (err) {
      const message = errorMessage(err, 'Could not save account')
      // A server whose certificate does not validate (a local bridge with a
      // self-signed one) is unreachable until its exact certificate is pinned.
      // Offer that instead of dead-ending on the handshake error. Compare the
      // probed certificate with the effective pin on this failed request: a
      // stale reconnect pin is replaceable, while retrying the same pin cannot
      // produce an endless prompt loop.
      const protocol = untrustedCertificateProtocol(message)
      if (protocol) {
        const attemptedPin =
          protocol === 'smtp'
            ? (smtpCertPin ?? reconnectAccount?.smtp_cert_pin)
            : (certPin ?? reconnectAccount?.cert_pin)
        const prompt = await probeCertificate(protocol)
        if (prompt && shouldPromptForCertificate(prompt.certificate.fingerprint, attemptedPin)) {
          setCertPrompt(prompt)
          return
        }
      }
      setError(message)
      if (mode === 'custom') setAdvancedOpen(true)
    } finally {
      setLoading(false)
    }
  }

  const saveDisabled =
    loading ||
    (mode === 'gmail' || mode === 'outlook'
      ? !exchangedTokens && !form.auth_code
      : mode === 'rss'
        ? !form.display_name
        : !form.email || (!form.password && !editing))

  return {
    mode,
    setMode,
    oauthConfigured,
    oauthLabel,
    gmailConfigured,
    outlookConfigured,
    form,
    setForm,
    error,
    loading,
    discovering,
    discoverNote,
    appPasswordHint,
    advancedOpen,
    setAdvancedOpen,
    waitingForGoogle,
    runDiscovery,
    beginOAuth,
    save,
    certPrompt,
    trustCertificate,
    dismissCertPrompt,
    saveDisabled,
    reconnectAccount,
    editing,
  }
}

export type AccountDialogController = ReturnType<typeof useAccountDialog>
