import { useEffect, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { Upload, Download, Trash2, Camera, KeyRound, Server } from 'lucide-react'
import { importOpml, exportOpml } from '../../states/feeds'
import { BackupPassphraseDialog } from './BackupPassphraseDialog'
import { showToast, ui$, type SetupMode } from '../../states/ui'
import { deleteAccount, isLocalAccountId, LOCAL_ACCOUNT_ID } from '../../states/accounts'
import { loadFolders } from '../../states/mailFolders'
import type { Account } from '../../types'
import { AccountAvatar } from '../avatar/AccountAvatar'
import { SettingRow, SettingsGroup } from './AccountSettingsRows'
import { AccountProxyCard } from './ProxySettingsCard'
import { AccountSignatureCard } from './SignatureSettingsCard'
import { AccountProfileGroup } from './AccountProfileGroup'
import { useAccountAvatar } from './useAccountAvatar'
import { RemoveImageBadge } from './RemoveImageBadge'
import { AccountAliasesCard } from './AccountAliasesCard'
import { AccountTogglesSection } from './AccountTogglesSection'
import { AccountWallpaperCard } from './AccountWallpaperCard'
import { AvatarCropDialog } from './AvatarCropDialog'
import { pickImageFile } from '../../lib/nativeFilePicker'
import { invoke } from '../../lib/bridge'
import { AccountEncryptionCard } from '../crypto/AccountEncryptionCard'
import { formatBytes } from './SettingsMaintenanceRows'

export function isRssAccount(account: Account) {
  return account.provider === 'rss' || account.auth_type === 'rss'
}

// The account's servers as one line, so the settings row shows what is
// configured without opening the editor. Mirrors the security labels the setup
// form offers (TLS / STARTTLS / None).
function serverSummary(account: Account, t: ReturnType<typeof useTranslation>['t']) {
  // TLS/STARTTLS are protocol names and stay verbatim; only "none" is prose.
  const security = (tls?: boolean, starttls?: boolean) =>
    starttls ? 'STARTTLS' : tls === false ? t('accounts.security.none') : 'TLS'
  const imap = `${account.imap_host || '—'}:${account.imap_port || 993} (${security(account.tls, account.starttls)})`
  const smtp = `${account.smtp_host || '—'}:${account.smtp_port || 465} (${security(account.smtp_tls, account.smtp_starttls)})`
  return `IMAP ${imap} · SMTP ${smtp}`
}

function reconnectMode(account: Account): SetupMode {
  if (account.auth_type === 'outlook_oauth' || account.provider === 'outlook') return 'outlook'
  if (account.auth_type === 'gmail_oauth' || account.provider === 'gmail') return 'gmail'
  return 'custom'
}

export function accountMeta(account: Account, t: ReturnType<typeof useTranslation>['t']) {
  const isRSS = isRssAccount(account)
  const isLocal = isLocalAccountId(account.id)
  const displayName =
    account.display_name ||
    (isRSS ? t('accounts.rssFeeds') : isLocal ? t('accounts.local.name') : account.email.split('@')[0])
  const subtitle = isRSS ? t('accounts.rssAtomFeeds') : isLocal ? t('accounts.local.subtitle') : account.email
  return { isRSS, isLocal, displayName, subtitle }
}

function OpmlGroup({ account }: { account: string }) {
  const { t } = useTranslation()
  return (
    <SettingsGroup title={t('settings.sections.subscriptions')}>
      <SettingRow
        title={t('settings.feeds.opmlFile')}
        hint={t('settings.feeds.opmlHint')}
        control={
          <div className="flex items-center gap-2">
            <button
              onClick={() => importOpml(account)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              <Upload size={12} />
              {t('common.import')}
            </button>
            <button
              onClick={() => exportOpml(account)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              <Download size={12} />
              {t('common.export')}
            </button>
          </div>
        }
      />
    </SettingsGroup>
  )
}

export function AccountPanel({ account }: { account: Account }) {
  const { t } = useTranslation()
  const { isRSS, isLocal, displayName, subtitle } = accountMeta(account, t)
  const [avatarFile, setAvatarFile] = useState<File | null>(null)
  const { avatarBusy, persistAvatarFile, persistAvatarUrl } = useAccountAvatar(account.id)

  const reconnectAccount = () => {
    ui$.reconnectAccountId.set(account.id)
    ui$.setupMode.set(reconnectMode(account))
    ui$.setupOpen.set(true)
  }

  const pickAvatarFile = async () => {
    try {
      setAvatarFile(await pickImageFile(t('settings.account.chooseAvatarImage')))
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('settings.account.chooseAvatarFailed'), 'error')
    }
  }

  const saveCroppedAvatar = async (file: File) => {
    if (await persistAvatarFile(file)) {
      setAvatarFile(null)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3 min-w-0">
        <div className="relative shrink-0 group">
          <button
            type="button"
            title={t('settings.account.changeAvatar')}
            disabled={avatarBusy}
            onClick={() => void pickAvatarFile()}
            className="relative block rounded-2xl disabled:cursor-default cursor-pointer"
          >
            <AccountAvatar
              accountId={account.id}
              name={displayName}
              src={account.avatar_url}
              size={40}
              className="!rounded-2xl"
            />
            <span className="absolute inset-0 flex items-center justify-center rounded-2xl bg-black/45 opacity-0 group-hover:opacity-100 transition-opacity">
              <Camera size={15} className="text-white" />
            </span>
          </button>
          {account.avatar_url && (
            <RemoveImageBadge
              label={t('settings.account.removeAvatar')}
              disabled={avatarBusy}
              onRemove={() => void persistAvatarUrl('')}
            />
          )}
        </div>
        {avatarFile && (
          <AvatarCropDialog
            file={avatarFile}
            busy={avatarBusy}
            onCancel={() => setAvatarFile(null)}
            onSave={saveCroppedAvatar}
          />
        )}
        <div className="min-w-0 flex-1">
          <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight truncate">{displayName}</h2>
          <p className="text-[0.65625rem] text-secondary mt-0.5 font-medium truncate">{subtitle}</p>
        </div>
      </div>

      {!isRSS && account.auth_type === 'password' && !account.needs_reconnect && (
        <SettingsGroup title={t('settings.account.serverTitle', { defaultValue: 'Server settings' })}>
          <SettingRow
            title={t('settings.account.serverAccount', { defaultValue: 'Incoming and outgoing servers' })}
            hint={serverSummary(account, t)}
            control={
              <button
                type="button"
                onClick={reconnectAccount}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-border text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
              >
                <Server size={12} />
                {t('settings.account.serverEdit', { defaultValue: 'Edit' })}
              </button>
            }
          />
        </SettingsGroup>
      )}

      {isLocal && <LocalStorageGroup />}

      {account.needs_reconnect && !isRSS && (
        <SettingsGroup title={t('settings.account.reconnectTitle', { defaultValue: 'Reconnect' })}>
          <SettingRow
            title={t('settings.account.reconnectAccount', { defaultValue: 'Reconnect account' })}
            hint={t('settings.account.reconnectHint', {
              defaultValue: 'Restore the missing keychain credential for this account.',
            })}
            control={
              <button
                type="button"
                onClick={reconnectAccount}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-accent hover:bg-accent-hover text-accent-label hover:text-accent-hover-label font-bold text-[0.625rem] cursor-pointer transition-colors"
              >
                <KeyRound size={12} />
                {t('settings.account.reconnectButton', { defaultValue: 'Reconnect' })}
              </button>
            }
          />
        </SettingsGroup>
      )}

      <AccountProfileGroup account={account} isRSS={isRSS || isLocal} />
      <SettingsGroup title={t('settings.pages.appearance')}>
        <AccountWallpaperCard account={account} />
      </SettingsGroup>
      <AccountTogglesSection account={account} isRSS={isRSS} isLocal={isLocal} />
      {!isRSS && !isLocal && <AccountEncryptionCard account={account} />}
      {!isRSS && !isLocal && <AccountProxyCard account={account} />}
      {!isRSS && !isLocal && <AccountAliasesCard account={account} />}
      {!isRSS && !isLocal && <AccountSignatureCard account={account} />}
      {isRSS && <OpmlGroup account={account.id} />}

      <button
        type="button"
        onClick={() => void deleteAccount(account.id)}
        className="mt-1 self-start flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs font-semibold text-secondary hover:text-rose-500 transition-colors cursor-pointer"
      >
        <Trash2 size={12} />
        {isLocal ? t('accounts.local.removeConfirm') : t('settings.account.removeAccount')}
      </button>
    </div>
  )
}

type LocalStats = { messages: number; message_bytes: number; file_bytes: number; path: string }

// What the local folders keep, and the warning that matters most: this is the
// only copy of that mail, and the settings backup does not include it. The
// backup here is a consistent snapshot; copying archive.db while Meron runs
// would miss what is still in its WAL.
function LocalStorageGroup() {
  const { t } = useTranslation()
  const [stats, setStats] = useState<LocalStats | null>(null)
  const [prompt, setPrompt] = useState<'localExport' | 'restore' | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  // The backup a wrong passphrase was tried on, so a retry skips the file dialog.
  const [restorePath, setRestorePath] = useState('')
  const loadStats = () =>
    invoke<LocalStats>('account.localStats')
      .then(setStats)
      .catch(() => {})
  const closePrompt = () => {
    setPrompt(null)
    setError('')
    setRestorePath('')
  }
  const submit = async (passphrase: string) => {
    setBusy(true)
    setError('')
    try {
      if (prompt === 'localExport') {
        const result = await invoke<{ saved: boolean }>('account.localBackup', { passphrase })
        if (result.saved) showToast(t('settings.backup.exported'), 'success')
        closePrompt()
        return
      }
      const result = await invoke<{ restored?: number; wrongPassphrase?: boolean; cancelled?: boolean; path?: string }>(
        'account.localRestore',
        { passphrase, path: restorePath },
      )
      if (result.cancelled) return
      if (result.wrongPassphrase) {
        setRestorePath(result.path ?? '')
        setError(t('settings.backup.wrongPassphrase'))
        return
      }
      closePrompt()
      showToast(t('settings.local.restoredCount', { count: result.restored ?? 0 }), 'success')
      void loadStats()
      void loadFolders(LOCAL_ACCOUNT_ID, false)
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t(prompt === 'localExport' ? 'settings.backup.exportFailed' : 'settings.backup.restoreFailed'),
      )
    } finally {
      setBusy(false)
    }
  }
  useEffect(() => {
    let alive = true
    invoke<LocalStats>('account.localStats')
      .then((next) => {
        if (alive) setStats(next)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [])
  return (
    <SettingsGroup title={t('settings.sections.data')}>
      <SettingRow
        title={t('settings.storage.usageTitle')}
        hint={stats?.path}
        control={
          <span className="text-xs text-secondary tabular-nums whitespace-nowrap">
            {stats ? `${t('chat.messagesCount', { count: stats.messages })} · ${formatBytes(stats.file_bytes)}` : '…'}
          </span>
        }
      />
      <SettingRow
        title={t('settings.backup.exportTitle')}
        hint={t('settings.local.backupHint')}
        control={
          <button
            type="button"
            onClick={() => setPrompt('localExport')}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:opacity-50 disabled:cursor-default"
          >
            <Download size={12} />
            {t('buttons.save')}
          </button>
        }
      />
      <SettingRow
        title={t('settings.backup.restoreTitle')}
        hint={t('settings.local.restoreHint')}
        control={
          <button
            type="button"
            onClick={() => setPrompt('restore')}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
          >
            <Upload size={12} />
            {t('settings.backup.restoreAction')}
          </button>
        }
      />
      {prompt && (
        <BackupPassphraseDialog
          mode={prompt}
          busy={busy}
          error={error}
          onCancel={closePrompt}
          onSubmit={(passphrase) => void submit(passphrase)}
        />
      )}
    </SettingsGroup>
  )
}
