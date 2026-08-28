import { useState } from 'react'
import { X, ShieldCheck, RefreshCw } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

/**
 * Passphrase prompt shared by both halves of backup/restore.
 *
 * 'export' collects a passphrase (and whether to include account passwords);
 * 'restore' collects the passphrase for a file that turned out to be encrypted.
 * The parent owns the actual call, so this component never touches the bridge.
 */
export type BackupPassphraseMode = 'export' | 'restore'

type Props = {
  mode: BackupPassphraseMode
  busy?: boolean
  /** Shown under the fields, e.g. after a wrong passphrase. */
  error?: string
  onCancel: () => void
  onSubmit: (passphrase: string, includeSecrets: boolean) => void
}

export function BackupPassphraseDialog({ mode, busy = false, error, onCancel, onSubmit }: Props) {
  const { t } = useTranslation()
  const [passphrase, setPassphrase] = useState('')
  const [confirmation, setConfirmation] = useState('')
  const [includeSecrets, setIncludeSecrets] = useState(false)

  const exporting = mode === 'export'
  useEscapeKey(onCancel, !busy)

  // Exporting: a passphrase is optional unless passwords are included, but once
  // typed it must be confirmed — a typo would produce a file nobody can open.
  // Restoring: whatever the user types is checked against the file immediately,
  // so no confirmation field.
  const mismatched = exporting && passphrase !== confirmation
  const missing = exporting ? includeSecrets && !passphrase : !passphrase
  const canSubmit = !busy && !missing && !mismatched

  const submit = () => {
    if (!canSubmit) return
    onSubmit(passphrase, exporting && includeSecrets)
  }

  return (
    <div className="fixed inset-0 flex items-center justify-center bg-black/40 dark:bg-black/60 backdrop-blur-[3px] z-50 p-4 select-none animate-fade-in">
      <div className="bg-chats border border-border text-primary max-w-md w-full rounded-3xl p-6 shadow-2xl animate-slide-up flex flex-col gap-5">
        {/* Header */}
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
              <ShieldCheck size={17} />
            </div>
            <div className="min-w-0">
              <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">
                {exporting ? t('settings.backup.exportTitle') : t('settings.backup.restoreTitle')}
              </h2>
              <p className="text-[0.65625rem] text-secondary mt-1 font-medium">
                {exporting ? t('settings.backup.exportSubtitle') : t('settings.backup.restoreSubtitle')}
              </p>
            </div>
          </div>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onCancel} />
        </div>

        {/* Content */}
        <div className="flex flex-col gap-4">
          {exporting && (
            <label className="flex items-start gap-2.5 px-1 cursor-pointer">
              <input
                type="checkbox"
                checked={includeSecrets}
                onChange={(event) => setIncludeSecrets(event.target.checked)}
                className="mt-0.5 accent-accent cursor-pointer"
              />
              <span className="min-w-0">
                <span className="block text-[0.75rem] font-semibold">{t('settings.backup.includeSecrets')}</span>
                <span className="block text-[0.65625rem] text-secondary mt-0.5 leading-relaxed font-medium">
                  {t('settings.backup.includeSecretsHint')}
                </span>
              </span>
            </label>
          )}

          <div className="flex flex-col gap-2">
            <label className="text-[0.6875rem] font-semibold text-secondary px-1">
              {t('settings.backup.passphrase')}
              {exporting && !includeSecrets && (
                <span className="font-medium text-secondary"> · {t('settings.network.optional')}</span>
              )}
            </label>
            <input
              autoFocus
              type="password"
              value={passphrase}
              onChange={(event) => setPassphrase(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') submit()
              }}
              className="w-full rounded-xl bg-hover px-3.5 py-2.5 text-[0.8125rem] text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:bg-chats border border-transparent transition-all duration-150"
            />
          </div>

          {exporting && (
            <div className="flex flex-col gap-2">
              <label className="text-[0.6875rem] font-semibold text-secondary px-1">
                {t('settings.backup.passphraseConfirm')}
              </label>
              <input
                type="password"
                value={confirmation}
                onChange={(event) => setConfirmation(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') submit()
                }}
                className="w-full rounded-xl bg-hover px-3.5 py-2.5 text-[0.8125rem] text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:bg-chats border border-transparent transition-all duration-150"
              />
            </div>
          )}

          <p className="text-[0.65625rem] text-secondary px-1 leading-relaxed font-medium">
            {exporting ? t('settings.backup.passphraseHint') : t('settings.backup.restoreHint')}
          </p>
          {mismatched && confirmation.length > 0 && (
            <p className="text-[0.6875rem] text-rose-500 px-1 font-medium">{t('settings.backup.passphraseMismatch')}</p>
          )}
          {error && <p className="text-[0.6875rem] text-rose-500 px-1 font-medium">{error}</p>}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 select-none">
          <Button variant="ghost" onClick={onCancel} disabled={busy}>
            {t('buttons.cancel')}
          </Button>
          <Button variant="primary" onClick={submit} disabled={!canSubmit}>
            {busy && <RefreshCw size={11} className="animate-spin" />}
            <span>{exporting ? t('common.export') : t('settings.backup.restoreAction')}</span>
          </Button>
        </div>
      </div>
    </div>
  )
}
