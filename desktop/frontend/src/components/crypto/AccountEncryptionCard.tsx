import { useCallback, useEffect, useState } from 'react'
import { Copy, Download, KeyRound, RefreshCw, ShieldCheck, Trash2, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { formatFingerprint } from '../../lib/crypto'
import type { AccountEncryptionState } from '../../lib/crypto'
import {
  accountEncryption,
  crypto$,
  deleteOwnKey,
  disableEncryption,
  enableExisting,
  exportPublicKey,
  exportSecretKey,
} from '../../states/crypto'
import { confirmAction, showToast } from '../../states/ui'
import type { Account } from '../../types'
import { Dialog } from '../dialog/Dialog'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { SettingRow, SettingsGroup, ToggleRow } from '../dialog/AccountSettingsRows'

function slug(address: string) {
  return address.replace(/[^a-zA-Z0-9.@_-]+/g, '-')
}

/**
 * End-to-end encryption for one account: the switch, and once on, the key.
 * Turning it on asks only "new key or existing key"; turning it off keeps the
 * key so earlier encrypted mail stays readable.
 */
export function AccountEncryptionCard({ account }: { account: Account }) {
  const { t } = useTranslation()
  const version = useValue(crypto$.version)
  const [state, setState] = useState<AccountEncryptionState | null>(null)
  const [exporting, setExporting] = useState(false)

  const reload = useCallback(() => {
    void accountEncryption(account.id)
      .then(setState)
      .catch(() => setState(null))
  }, [account.id])
  useEffect(reload, [reload, version])

  const toggle = async () => {
    if (!state) return
    if (state.enabled) {
      const ok = await confirmAction({
        title: t('crypto.account.disableTitle'),
        message: t('crypto.account.disableMessage'),
        confirmLabel: t('crypto.account.disable'),
      })
      if (ok) await disableEncryption(account.id)
      return
    }
    // A key kept from before: switch back on without asking again.
    if (state.key && (await enableExisting(account.id))) return
    crypto$.enableFor.set({ accountId: account.id, address: account.email })
  }

  const key = state?.key
  return (
    <SettingsGroup title={t('crypto.account.title')}>
      <ToggleRow
        title={t('crypto.account.switch')}
        hint={t('crypto.account.switchHint')}
        checked={!!state?.enabled}
        onChange={() => void toggle()}
      />
      {key && (
        <>
          <SettingRow
            title={t('crypto.account.yourKey')}
            hint={t('crypto.account.yourKeyHint')}
            control={
              <div className="flex items-center gap-1.5">
                <span className="font-mono text-[0.6875rem] text-secondary select-text">
                  {formatFingerprint(key.fingerprint)}
                </span>
                <IconButton
                  icon={Copy}
                  iconSize={13}
                  size="sm"
                  label={t('crypto.account.copyFingerprint')}
                  onClick={() => {
                    void navigator.clipboard.writeText(formatFingerprint(key.fingerprint))
                    showToast(t('crypto.account.copied'), 'success')
                  }}
                />
              </div>
            }
          />
          {key.unusable && (
            <p className="px-3.5 py-2 text-[0.6875rem] text-amber-700 dark:text-amber-300">
              {t(`crypto.account.unusable.${key.unusable}`)}
            </p>
          )}
          <SettingRow
            title={t('crypto.account.exportPublic')}
            hint={t('crypto.account.exportPublicHint')}
            control={
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void exportPublicKey(account.id, `${slug(account.email)}-public.asc`)}
              >
                <Download size={12} />
                <span>{t('common.export')}</span>
              </Button>
            }
          />
          <SettingRow
            title={t('crypto.account.exportSecret')}
            hint={t('crypto.account.exportSecretHint')}
            control={
              <Button variant="secondary" size="sm" onClick={() => setExporting(true)}>
                <KeyRound size={12} />
                <span>{t('common.export')}</span>
              </Button>
            }
          />
          <SettingRow
            title={t('crypto.account.replace')}
            hint={t('crypto.account.replaceHint')}
            control={
              <Button
                variant="secondary"
                size="sm"
                onClick={() => crypto$.enableFor.set({ accountId: account.id, address: account.email })}
              >
                <span>{t('crypto.account.replaceAction')}</span>
              </Button>
            }
          />
          <SettingRow
            title={t('crypto.account.delete')}
            hint={t('crypto.account.deleteHint')}
            control={
              <Button
                variant="danger"
                size="sm"
                onClick={async () => {
                  const ok = await confirmAction({
                    title: t('crypto.account.deleteTitle'),
                    message: t('crypto.account.deleteMessage'),
                    confirmLabel: t('crypto.account.delete'),
                    tone: 'danger',
                  })
                  if (ok) await deleteOwnKey(account.id, key.fingerprint)
                }}
              >
                <Trash2 size={12} />
                <span>{t('buttons.delete')}</span>
              </Button>
            }
          />
        </>
      )}
      {exporting && (
        <ExportSecretDialog
          onCancel={() => setExporting(false)}
          onSubmit={async (passphrase) => {
            const res = await exportSecretKey(account.id, passphrase, `${slug(account.email)}-secret.asc`)
            setExporting(false)
            if (res.saved) showToast(t('crypto.account.exported'), 'success')
          }}
        />
      )}
    </SettingsGroup>
  )
}

/** A new passphrase (typed twice) for a secret-key export, after a warning. */
function ExportSecretDialog({
  onCancel,
  onSubmit,
}: {
  onCancel: () => void
  onSubmit: (passphrase: string) => Promise<void>
}) {
  const { t } = useTranslation()
  const [pass, setPass] = useState('')
  const [again, setAgain] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const ok = pass.length >= 8 && pass === again && !busy
  const submit = async () => {
    if (!ok) return
    setBusy(true)
    try {
      await onSubmit(pass)
    } catch (err) {
      setError(String(err))
      setBusy(false)
    }
  }
  const input =
    'w-full rounded-xl border border-transparent bg-hover px-3.5 py-2.5 text-[0.8125rem] text-primary transition-all duration-150 focus:bg-chats focus:ring-1 focus:ring-accent'
  return (
    <Dialog
      onClose={onCancel}
      dismissible={!busy}
      closeOnBackdrop={false}
      aria-label={t('crypto.account.exportSecret')}
      className="max-w-md p-6 flex flex-col gap-5"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <ShieldCheck size={17} />
          </div>
          <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">
            {t('crypto.account.exportSecret')}
          </h2>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onCancel} />
      </div>
      <p className="rounded-xl bg-amber-500/10 px-3.5 py-2.5 text-[0.6875rem] leading-relaxed text-amber-800 dark:text-amber-200">
        {t('crypto.account.exportSecretWarning')}
      </p>
      <div className="flex flex-col gap-2">
        <label className="px-1 text-[0.6875rem] font-semibold text-secondary">
          {t('crypto.account.exportPassphrase')}
        </label>
        <input autoFocus type="password" value={pass} onChange={(e) => setPass(e.target.value)} className={input} />
        <label className="px-1 text-[0.6875rem] font-semibold text-secondary">
          {t('settings.backup.passphraseConfirm')}
        </label>
        <input
          type="password"
          value={again}
          onChange={(e) => setAgain(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void submit()
          }}
          className={input}
        />
        {pass.length > 0 && pass.length < 8 && (
          <p className="px-1 text-[0.6875rem] text-secondary">{t('crypto.account.passphraseTooShort')}</p>
        )}
        {again.length > 0 && pass !== again && (
          <p className="px-1 text-[0.6875rem] text-rose-500">{t('settings.backup.passphraseMismatch')}</p>
        )}
        {error && (
          <p role="alert" className="px-1 text-[0.6875rem] text-rose-500">
            {error}
          </p>
        )}
      </div>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancel} disabled={busy}>
          {t('buttons.cancel')}
        </Button>
        <Button variant="primary" onClick={() => void submit()} disabled={!ok}>
          {busy && <RefreshCw size={11} className="animate-spin" />}
          <span>{t('common.export')}</span>
        </Button>
      </div>
    </Dialog>
  )
}
