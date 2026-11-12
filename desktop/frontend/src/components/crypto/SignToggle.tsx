import { Signature } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'

/**
 * The composer's OpenPGP sign switch, beside the lock: whether this message,
 * when it isn't encrypted, goes out signed with the account's key. It starts
 * from the account's "Sign unencrypted mail" setting. Encrypted mail is always
 * signed, so with the lock on it shows on and can't be switched.
 *
 * lucide's `Signature`, the mark CryptoBadge shows on signed mail (the text
 * signature's switch beside it is a pen).
 */
export function SignToggle({
  on,
  encrypted,
  onChange,
  size = 'sm',
}: {
  on: boolean
  /** The message is encrypted, which always signs. */
  encrypted: boolean
  onChange: (on: boolean) => void
  size?: 'sm' | 'md'
}) {
  const { t } = useTranslation()
  const shown = on || encrypted
  const label = encrypted ? t('crypto.sign.encrypted') : on ? t('crypto.sign.on') : t('crypto.sign.off')
  const box = size === 'md' ? 'h-8 w-8 rounded-xl' : 'h-7 w-7 rounded-lg'
  const tone = shown ? 'text-accent bg-accent/10' : 'text-secondary enabled:hover:bg-active enabled:hover:text-primary'
  return (
    <button
      type="button"
      role="switch"
      aria-checked={shown}
      aria-label={label}
      title={label}
      disabled={encrypted}
      onClick={() => onChange(!on)}
      className={`flex ${box} shrink-0 items-center justify-center transition-colors enabled:cursor-pointer disabled:opacity-60 ${tone}`}
    >
      <Signature size={15} strokeWidth={shown ? 2.4 : 2} />
    </button>
  )
}
