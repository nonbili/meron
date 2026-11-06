import { KeyRound, Lock, ShieldAlert } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { stripFor } from '../../lib/crypto'
import type { MessageCrypto } from '../../lib/crypto'
import { crypto$, dismissEncryptionOffer } from '../../states/crypto'

/**
 * A strip above a message body, only when something needs saying: a message
 * that couldn't be opened, a broken or foreign signature, a contact's new
 * key, or the offer to turn encryption on. Quiet mail gets nothing; the
 * header badge covers the normal states.
 */
export function CryptoStrip({
  crypto,
  accountId,
  accountAddress,
  senderName,
  senderAddress,
}: {
  crypto?: MessageCrypto | null
  accountId: string
  accountAddress: string
  senderName: string
  senderAddress: string
}) {
  const { t } = useTranslation()
  const strip = stripFor(crypto)
  if (!crypto || !strip) return null
  const sender = senderName || senderAddress

  const verify = () =>
    crypto$.verify.set({ addr: senderAddress, name: sender, fingerprint: crypto.signer ?? '', accountId })
  const turnOn = () => crypto$.enableFor.set({ accountId, address: accountAddress })

  let danger = false
  let icon = <ShieldAlert size={14} className="mt-px shrink-0" />
  let text = ''
  const actions: { label: string; onClick: () => void; primary?: boolean }[] = []

  if (strip.kind === 'problem') {
    danger = strip.danger
    text = t(`crypto.problem.${strip.problem}`, { sender })
    if (strip.problem === 'no_key') {
      icon = <Lock size={14} className="mt-px shrink-0" />
      actions.push({ label: t('crypto.useExistingKey'), onClick: turnOn, primary: true })
    }
  } else if (strip.kind === 'key_changed') {
    danger = strip.danger
    icon = <KeyRound size={14} className="mt-px shrink-0" />
    text = strip.danger ? t('crypto.keyChangedVerified', { sender }) : t('crypto.keyChanged', { sender })
    if (crypto.signer) {
      actions.push({ label: strip.danger ? t('crypto.verifyNewKey') : t('crypto.verifyKey'), onClick: verify })
    }
  } else {
    icon = <Lock size={14} className="mt-px shrink-0" />
    text = t('crypto.offer', { sender, account: accountAddress })
    actions.push({ label: t('crypto.turnOn'), onClick: turnOn, primary: true })
    actions.push({ label: t('crypto.notNow'), onClick: () => void dismissEncryptionOffer(accountId, senderAddress) })
    actions.push({ label: t('crypto.dontAskAgain'), onClick: () => void dismissEncryptionOffer(accountId, '*') })
  }

  return (
    <div
      role={danger ? 'alert' : 'status'}
      data-danger={danger || undefined}
      className={`mb-2 flex flex-wrap items-start gap-x-3 gap-y-1.5 rounded-lg px-3 py-2 text-[0.71875rem] leading-snug ${
        danger ? 'bg-rose-500/10 text-rose-700 dark:text-rose-300' : 'bg-hover text-primary'
      }`}
      // Header clicks collapse messages; the strip's buttons must not.
      onClick={(event) => event.stopPropagation()}
    >
      <div className="flex min-w-[12rem] flex-1 items-start gap-2">
        {icon}
        <span className="select-text">{text}</span>
      </div>
      {actions.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {actions.map((action) => (
            <button
              key={action.label}
              type="button"
              onClick={action.onClick}
              className={`rounded-md px-2 py-1 text-[0.6875rem] font-semibold cursor-pointer transition-colors ${
                action.primary
                  ? 'bg-accent text-accent-label hover:bg-accent-hover'
                  : 'text-secondary hover:bg-black/[0.05] hover:text-primary dark:hover:bg-white/[0.08]'
              }`}
            >
              {action.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
