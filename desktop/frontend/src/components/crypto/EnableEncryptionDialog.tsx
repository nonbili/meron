import { useEffect, useState } from 'react'
import { FileKey, KeyRound, RefreshCw, ShieldCheck, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { formatFingerprint } from '../../lib/crypto'
import type { KeyInfo } from '../../lib/crypto'
import { createKey, crypto$, importOwnKey, pickKeyFile } from '../../states/crypto'
import type { KeyFile } from '../../states/crypto'
import { showToast } from '../../states/ui'
import { Dialog } from '../dialog/Dialog'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

type Step =
  | { kind: 'choose' }
  | { kind: 'passphrase'; file: KeyFile; key: KeyInfo; fingerprint?: string; wrong: boolean }
  | { kind: 'pick'; file: KeyFile; keys: KeyInfo[] }

/**
 * Turning end-to-end encryption on for an account. Asks one question: create
 * a new key, or use the one the user already has (GnuPG, Thunderbird, …).
 * After that nothing else is asked; Meron shares the key through Autocrypt.
 */
export function EnableEncryptionDialog() {
  const { t } = useTranslation()
  const target = useValue(crypto$.enableFor)
  const [step, setStep] = useState<Step>({ kind: 'choose' })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [passphrase, setPassphrase] = useState('')

  useEffect(() => {
    setStep({ kind: 'choose' })
    setError('')
    setPassphrase('')
  }, [target?.accountId])

  if (!target) return null
  const close = () => crypto$.enableFor.set(null)

  const done = (key: KeyInfo, addressMatches = true) => {
    showToast(t('crypto.enable.done'), 'success')
    if (!addressMatches) showToast(t('crypto.enable.addressMismatch', { address: target.address }), 'error')
    void key
    close()
  }

  const create = async () => {
    setBusy(true)
    setError('')
    try {
      done(await createKey(target.accountId))
    } catch (err) {
      setError(String(err))
    } finally {
      setBusy(false)
    }
  }

  const tryImport = async (file: KeyFile, pass?: string, fingerprint?: string) => {
    setBusy(true)
    setError('')
    try {
      const { result } = await importOwnKey(target.accountId, file, pass, fingerprint)
      switch (result.status) {
        case 'imported':
          done(result.key, result.address_matches)
          break
        case 'needs_passphrase':
          setPassphrase('')
          setStep({ kind: 'passphrase', file, key: result.key, fingerprint, wrong: pass !== undefined })
          break
        case 'choose':
          setStep({ kind: 'pick', file, keys: result.keys })
          break
        case 'on_card':
          setError(t('crypto.enable.onCard'))
          setStep({ kind: 'choose' })
          break
        case 'no_secret_key':
          setError(t('crypto.enable.noSecretKey'))
          setStep({ kind: 'choose' })
          break
      }
    } catch (err) {
      setError(String(err))
    } finally {
      setBusy(false)
    }
  }

  const useExisting = async () => {
    const file = await pickKeyFile(t('crypto.enable.pickTitle'))
    if (file) await tryImport(file)
  }

  return (
    <Dialog
      onClose={close}
      dismissible={!busy}
      closeOnBackdrop={false}
      aria-label={t('crypto.enable.title')}
      className="max-w-md p-6 flex flex-col gap-5"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <ShieldCheck size={17} />
          </div>
          <div className="min-w-0">
            <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">{t('crypto.enable.title')}</h2>
            <p className="mt-1 truncate text-[0.65625rem] font-medium text-secondary">{target.address}</p>
          </div>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={close} />
      </div>

      {step.kind === 'choose' && (
        <div className="flex flex-col gap-4">
          <p className="text-[0.8125rem] font-semibold">{t('crypto.enable.question', { address: target.address })}</p>
          <p className="rounded-xl bg-amber-500/10 px-3.5 py-2.5 text-[0.6875rem] leading-relaxed text-amber-800 dark:text-amber-200">
            {t('crypto.enable.otherDevices')}
          </p>
          <div className="flex flex-col gap-2">
            <Button variant="primary" onClick={() => void create()} disabled={busy}>
              {busy ? <RefreshCw size={12} className="animate-spin" /> : <KeyRound size={13} />}
              <span>{t('crypto.enable.create')}</span>
            </Button>
            <Button variant="secondary" onClick={() => void useExisting()} disabled={busy}>
              <FileKey size={13} />
              <span>{t('crypto.enable.useExisting')}</span>
            </Button>
          </div>
          <p className="px-1 text-[0.65625rem] leading-relaxed text-secondary">{t('crypto.enable.backupHint')}</p>
        </div>
      )}

      {step.kind === 'passphrase' && (
        <div className="flex flex-col gap-3">
          <p className="text-[0.75rem] leading-relaxed">
            {t('crypto.enable.passphraseFor', { key: step.key.user_ids[0] ?? '' })}
          </p>
          <p className="font-mono text-[0.65625rem] text-secondary">{formatFingerprint(step.key.fingerprint)}</p>
          <input
            autoFocus
            type="password"
            value={passphrase}
            aria-label={t('crypto.enable.passphrase')}
            onChange={(event) => setPassphrase(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && passphrase)
                void tryImport(step.file, passphrase, step.fingerprint ?? step.key.fingerprint)
            }}
            className="w-full rounded-xl border border-transparent bg-hover px-3.5 py-2.5 text-[0.8125rem] text-primary transition-all duration-150 focus:bg-chats focus:ring-1 focus:ring-accent"
          />
          {step.wrong && (
            <p role="alert" className="px-1 text-[0.6875rem] font-medium text-rose-500">
              {t('crypto.enable.wrongPassphrase')}
            </p>
          )}
          <p className="px-1 text-[0.65625rem] leading-relaxed text-secondary">{t('crypto.enable.passphraseOnce')}</p>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setStep({ kind: 'choose' })} disabled={busy}>
              {t('buttons.cancel')}
            </Button>
            <Button
              variant="primary"
              disabled={busy || !passphrase}
              onClick={() => void tryImport(step.file, passphrase, step.fingerprint ?? step.key.fingerprint)}
            >
              {busy && <RefreshCw size={11} className="animate-spin" />}
              <span>{t('crypto.enable.unlock')}</span>
            </Button>
          </div>
        </div>
      )}

      {step.kind === 'pick' && (
        <div className="flex flex-col gap-2">
          <p className="text-[0.75rem]">{t('crypto.enable.pickKey')}</p>
          {step.keys.map((key) => (
            <button
              key={key.fingerprint}
              type="button"
              disabled={busy}
              onClick={() => void tryImport(step.file, undefined, key.fingerprint)}
              className="flex flex-col items-start gap-0.5 rounded-xl bg-hover px-3.5 py-2.5 text-left hover:bg-accent/10 cursor-pointer"
            >
              <span className="text-[0.75rem] font-semibold">{key.user_ids[0] ?? key.emails[0] ?? '—'}</span>
              <span className="font-mono text-[0.625rem] text-secondary">{formatFingerprint(key.fingerprint)}</span>
            </button>
          ))}
        </div>
      )}

      {error && (
        <p role="alert" className="px-1 text-[0.6875rem] font-medium text-rose-500">
          {error}
        </p>
      )}
    </Dialog>
  )
}
