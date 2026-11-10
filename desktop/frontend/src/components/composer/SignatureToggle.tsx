import { PenLine, PenOff } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import type { SignatureSwitch } from '../../lib/signature'

/**
 * The signature switch of the quick reply and the full composer: shows whether
 * the account's signature goes out with this message (the tooltip holds the
 * signature itself) and turns it on or off for this message only. Disabled
 * when the body's signature is not one the app can take out — edited, or a
 * reopened draft's that isn't the account's current one.
 *
 * Not lucide's `Signature` icon: CryptoBadge uses it for OpenPGP-signed mail,
 * and beside the lock it would read as "this message is signed with my key".
 */
export function SignatureToggle({
  state,
  signature,
  onChange,
  size = 'sm',
}: {
  state: SignatureSwitch
  /** The account's signature as plain text, for the tooltip. */
  signature: string
  onChange: (on: boolean) => void
  size?: 'sm' | 'md'
}) {
  const { t } = useTranslation()
  const on = state === 'on'
  const label =
    state === 'unmanaged'
      ? t('composer.signature.unmanaged')
      : on
        ? t('composer.signature.on')
        : t('composer.signature.off')
  // The lock's box beside it (EncryptToggle), at either size.
  const box = size === 'md' ? 'h-8 w-8 rounded-lg' : 'h-7 w-7 rounded-lg'
  const iconSize = 15
  // On looks like the lock when it's on (EncryptToggle); off and not switchable
  // take the grey of the attachment and full-editor buttons beside it.
  const tone = on ? 'text-accent bg-accent/10' : 'text-secondary enabled:hover:bg-active enabled:hover:text-primary'
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      aria-description={signature || undefined}
      title={signature ? `${label}\n\n${signature}` : label}
      disabled={state === 'unmanaged'}
      onClick={() => onChange(!on)}
      className={`flex ${box} shrink-0 items-center justify-center transition-colors enabled:cursor-pointer disabled:opacity-50 ${tone}`}
    >
      {on ? <PenLine size={iconSize} strokeWidth={2.4} /> : <PenOff size={iconSize} />}
    </button>
  )
}
