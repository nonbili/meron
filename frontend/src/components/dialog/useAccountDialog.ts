import { useEffect, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { boot } from '../../boot'
import { ui$, type SetupMode } from '../../states/ui'
import { accounts$ } from '../../states/accounts'
import { closeKanbanBoard } from '../../states/kanban'
import { errorMessage } from '../../lib/errors'

type AddAccountResult = { account?: { id?: string } }

// All of the account-setup dialog's state and backend flows: provider OAuth
// (begin + poll), IMAP/SMTP autodiscovery, and the final save. Returned to the
// dialog and its mode-specific form sections, which stay presentational.
export function useAccountDialog() {
  const mode = useValue(ui$.setupMode)
  const system = useValue(ui$.system)
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
    imap_port: '993',
    smtp_host: '',
    smtp_port: '465',
    username: '',
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
    closeKanbanBoard()
    ui$.selectedAccount.set(created.id)
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.selectedStarredItem.set('')
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

  const setMode = (newMode: SetupMode) => {
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

  async function beginOAuth() {
    try {
      setError('')
      setWaitingForGoogle(true)
      const res = await invoke<{ url: string; needs_external_browser?: boolean }>(`oauth.${oauthProvider}Begin`)
      if (res.url && !res.needs_external_browser) {
        window.location.href = res.url
      }
      if (res.url) {
        const id = setInterval(() => {
          void pollProfile(id, oauthProvider)
        }, 1000)
        ;(window as any)._oauthPollInterval = id
      } else {
        setWaitingForGoogle(false)
      }
    } catch (err) {
      setWaitingForGoogle(false)
      setError(errorMessage(err, `Failed to begin ${oauthLabel} sign in`))
    }
  }

  async function runDiscovery(email: string) {
    if (!email.includes('@') || email.endsWith('@')) return
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
      setForm((f) => ({
        ...f,
        // Don't clobber anything the user already typed.
        imap_host: f.imap_host || cfg.imap_host,
        imap_port: f.imap_host ? f.imap_port : String(cfg.imap_port),
        smtp_host: f.smtp_host || cfg.smtp_host,
        smtp_port: f.smtp_host ? f.smtp_port : String(cfg.smtp_port),
        username: f.username || cfg.username,
      }))
      setAppPasswordHint(cfg.app_password_hint ?? null)
      if (cfg.source === 'guess') {
        setAdvancedOpen(true)
        setDiscoverNote("Couldn't find settings automatically — please verify the servers below.")
      } else {
        setDiscoverNote(`Settings found${cfg.provider_name ? ` for ${cfg.provider_name}` : ''}.`)
      }
    } catch {
      setDiscoverNote('')
    } finally {
      setDiscovering(false)
    }
  }

  async function save() {
    try {
      setError('')
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
          password: form.password,
          tls: true,
        })
        createdId = added.account?.id ?? ''
      }
      ui$.setupOpen.set(false)
      await boot()
      const created = selectCreatedAccount(before, createdId)
      // When the dialog was opened from within Settings, drop straight into the
      // new account's panel; boot() has refreshed accounts$, so the one id not in
      // the pre-save snapshot is the account we just created.
      if (ui$.settingsOpen.peek()) {
        if (created) ui$.accountSettingsId.set(created.id)
      }
    } catch (err) {
      setError(errorMessage(err, 'Could not save account'))
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
        : !form.email || !form.password)

  return {
    mode,
    setMode,
    oauthConfigured,
    oauthLabel,
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
    saveDisabled,
  }
}

export type AccountDialogController = ReturnType<typeof useAccountDialog>
