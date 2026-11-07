import { useEffect, useState } from 'react'
import { BadgeCheck, RefreshCw, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { fingerprintGroups, fingerprintsMatch } from '../../lib/crypto'
import { accountEncryption, crypto$, setKeyVerified } from '../../states/crypto'
import { showToast } from '../../states/ui'
import { Dialog } from '../dialog/Dialog'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

/** A fingerprint as ten groups of four, read out group by group. */
export function FingerprintBlock({ fingerprint, label }: { fingerprint: string; label: string }) {
  const groups = fingerprintGroups(fingerprint)
  return (
    <div className="flex flex-col gap-1">
      <span className="px-1 text-[0.6875rem] font-semibold text-secondary">{label}</span>
      <div
        role="group"
        aria-label={`${label}: ${groups.join(', ')}`}
        className="grid grid-cols-5 gap-x-3 gap-y-1 rounded-xl bg-hover px-4 py-3 font-mono text-[0.9375rem] tracking-wide text-primary select-text"
      >
        {groups.map((group, i) => (
          <span key={i}>{group}</span>
        ))}
      </div>
    </div>
  )
}

/**
 * Verifying a contact's key: compare fingerprints over another channel. Two
 * ways to confirm: paste the fingerprint they sent (Meron compares, no
 * misreading), or read them aloud and tick "they match" (a checkbox, so it
 * is never a reflex click). Verification is local, per key.
 */
export function VerifyKeyDialog() {
  const { t } = useTranslation()
  const target = useValue(crypto$.verify)
  const [own, setOwn] = useState('')
  const [pasted, setPasted] = useState('')
  const [compared, setCompared] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    setPasted('')
    setCompared(false)
    setError('')
    setOwn('')
    if (!target?.accountId) return
    void accountEncryption(target.accountId)
      .then((state) => setOwn(state.key?.fingerprint ?? ''))
      .catch(() => setOwn(''))
  }, [target?.fingerprint, target?.accountId])

  if (!target) return null
  const close = () => crypto$.verify.set(null)
  const pastedMatches = pasted.trim() !== '' && fingerprintsMatch(pasted, target.fingerprint)
  const pastedWrong = pasted.trim().length >= 8 && !pastedMatches
  // The easy mix-up: pasting one's own fingerprint from the block above.
  const pastedOwn = pastedWrong && !!own && fingerprintsMatch(pasted, own)
  const canConfirm = !busy && !pastedWrong && (pastedMatches || compared)

  const confirm = async () => {
    if (!canConfirm) return
    setBusy(true)
    setError('')
    try {
      // The core checks this equals the stored key's fingerprint.
      await setKeyVerified(target.addr, target.fingerprint, pastedMatches ? pasted : target.fingerprint, true)
      showToast(t('crypto.verify.done', { sender: target.name }), 'success')
      close()
    } catch (err) {
      setError(String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog
      onClose={close}
      dismissible={!busy}
      closeOnBackdrop={false}
      aria-label={t('crypto.verify.title', { sender: target.name })}
      className="max-w-lg p-6 flex flex-col gap-5"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-verified/10 text-verified">
            <BadgeCheck size={17} />
          </div>
          <div className="min-w-0">
            <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">
              {t('crypto.verify.title', { sender: target.name })}
            </h2>
            <p className="mt-1 truncate text-[0.65625rem] font-medium text-secondary">{target.addr}</p>
          </div>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={close} />
      </div>

      <div className="flex flex-col gap-4">
        <FingerprintBlock fingerprint={target.fingerprint} label={t('crypto.verify.theirs', { sender: target.name })} />
        {own && <FingerprintBlock fingerprint={own} label={t('crypto.verify.yours')} />}
        <p className="px-1 text-[0.6875rem] leading-relaxed text-secondary">
          {t('crypto.verify.howTo', { sender: target.name })}
        </p>

        <div className="flex flex-col gap-2">
          <label className="px-1 text-[0.6875rem] font-semibold text-secondary" htmlFor="verify-paste">
            {t('crypto.verify.paste', { sender: target.name })}
          </label>
          <input
            id="verify-paste"
            value={pasted}
            onChange={(event) => setPasted(event.target.value)}
            spellCheck={false}
            autoComplete="off"
            className="w-full rounded-xl border border-transparent bg-hover px-3.5 py-2.5 font-mono text-[0.75rem] text-primary placeholder-secondary transition-all duration-150 focus:bg-chats focus:ring-1 focus:ring-accent"
          />
          {pastedMatches && (
            <p role="status" className="px-1 text-[0.6875rem] font-semibold text-verified">
              {t('crypto.verify.matches')}
            </p>
          )}
          {pastedWrong && (
            <p role="alert" className="px-1 text-[0.6875rem] font-semibold text-rose-600 dark:text-rose-400">
              {pastedOwn ? t('crypto.verify.pastedOwn', { sender: target.name }) : t('crypto.verify.mismatch')}
            </p>
          )}
        </div>

        <label className="flex cursor-pointer items-center gap-2.5 px-1">
          <input
            type="checkbox"
            checked={compared}
            disabled={pastedWrong}
            onChange={(event) => setCompared(event.target.checked)}
            className="m-0 shrink-0 cursor-pointer accent-accent"
          />
          <span className="text-[0.75rem] font-semibold">{t('crypto.verify.compared')}</span>
        </label>
        {error && <p className="px-1 text-[0.6875rem] font-medium text-rose-500">{error}</p>}
      </div>

      <div className="flex justify-end gap-2 select-none">
        <Button variant="ghost" onClick={close} disabled={busy}>
          {t('buttons.cancel')}
        </Button>
        <Button variant="primary" onClick={() => void confirm()} disabled={!canConfirm}>
          {busy && <RefreshCw size={11} className="animate-spin" />}
          <span>{t('crypto.verify.confirm')}</span>
        </Button>
      </div>
    </Dialog>
  )
}
