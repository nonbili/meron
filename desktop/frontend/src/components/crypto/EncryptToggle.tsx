import { useEffect, useMemo, useState } from 'react'
import { Lock, LockOpen } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import type { RecipientKeys } from '../../lib/crypto'
import { crypto$, recipientKeys } from '../../states/crypto'
import { settings$ } from '../../states/settings'
import { confirmAction } from '../../states/ui'

/** Bare addresses of a comma-separated recipient field (quotes and angle
 * brackets respected, like the core's `split_recipients`). */
export function recipientAddresses(...fields: string[]): string[] {
  const out: string[] = []
  for (const field of fields) {
    let current = ''
    let quoted = false
    let bracket = false
    for (const ch of field) {
      if (ch === '"') quoted = !quoted
      else if (ch === '<' && !quoted) bracket = true
      else if (ch === '>' && !quoted) bracket = false
      if (ch === ',' && !quoted && !bracket) {
        out.push(current)
        current = ''
      } else current += ch
    }
    out.push(current)
  }
  return out
    .map((entry) => {
      const m = entry.match(/<([^>]+)>/)
      return (m ? m[1] : entry).trim().replace(/^"|"$/g, '').toLowerCase()
    })
    .filter((addr) => addr.includes('@'))
}

/** The composer's key check for these recipients, debounced while typing. */
export function useRecipientKeys(accountId: string, addrs: string[]): RecipientKeys | null {
  const version = useValue(crypto$.version)
  const key = addrs.join(',')
  const [state, setState] = useState<RecipientKeys | null>(null)
  useEffect(() => {
    let live = true
    const timer = setTimeout(() => {
      void recipientKeys(accountId, key ? key.split(',') : [])
        .then((res) => live && setState(res))
        .catch(() => live && setState(null))
    }, 250)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [accountId, key, version])
  return state
}

/** Why the composer may not send as it stands, or null. */
export function encryptionBlocksSend(on: boolean, keys: RecipientKeys | null): boolean {
  if (!on) return false
  if (!keys) return true
  return !keys.enabled || keys.own_key_unusable || keys.missing.length > 0
}

/**
 * The composer's end-to-end encryption toggle, left of Send (a toggle,
 * sticky on replies). Shown only when the From account has encryption on.
 * Mail that carries text from an encrypted message (a reply, a forward, an
 * unfolded quote) never downgrades silently: with a recipient who has no key it
 * stays on and Send is blocked; the lock looks the key up first, and a reply
 * can then be switched off on purpose (with a confirmation).
 */
export function EncryptToggle({
  keys,
  on,
  onChange,
  sensitive,
  bodyIsDecrypted = false,
  quoteWithheld = false,
  hasRecipients = true,
  size = 'sm',
}: {
  keys: RecipientKeys | null
  on: boolean
  onChange: (on: boolean) => void
  /** The message has at least one recipient (nothing to encrypt for otherwise). */
  hasRecipients?: boolean
  /** Carries text from an encrypted message (a reply's quote, a forward). */
  sensitive?: 'reply' | 'forward'
  /** The body itself holds decrypted text (an unfolded quote): locked on. */
  bodyIsDecrypted?: boolean
  /** The quick reply leaves its original unquoted (`replyQuoteWithheld`); the
   * box shows no quote either way, so the lock's tooltip says why. */
  quoteWithheld?: boolean
  size?: 'sm' | 'md'
}) {
  const { t } = useTranslation()
  const missing = useMemo(() => keys?.missing.map((m) => m.addr).join(', ') ?? '', [keys])
  // A lookup opened from this lock: once every recipient has a key (the user
  // picked "Use this key"), encryption switches on by itself.
  const [lookingUp, setLookingUp] = useState(false)
  // A new message is only encrypted when every recipient has a key: a
  // recipient without one switches it off, so the lock shows open and the mail
  // can go out unencrypted. Mail that carries text from an encrypted message
  // is the exception: switching that off by itself would send the reply plain
  // (and skip the core's rule that replies to encrypted mail stay encrypted),
  // so it stays on and Send waits for the keys.
  const someoneLacksKey = !!keys?.enabled && !keys.own_key_unusable && keys.missing.length > 0
  useEffect(() => {
    if (on && someoneLacksKey && !sensitive) onChange(false)
  }, [on, someoneLacksKey, sensitive, onChange])
  // Encrypt by default when every recipient has a key (preference, on by
  // default), and after a lookup this lock opened. A user who switched it off
  // in this message keeps it off.
  const encryptWhenPossible = useValue(settings$.encryptWhenPossible)
  const [userOff, setUserOff] = useState(false)
  const everyoneHasKey = !!keys?.enabled && !keys.own_key_unusable && keys.missing.length === 0 && hasRecipients
  useEffect(() => {
    if (on || !everyoneHasKey) return
    if (lookingUp || (encryptWhenPossible && !userOff)) {
      setLookingUp(false)
      onChange(true)
    }
  }, [on, everyoneHasKey, lookingUp, encryptWhenPossible, userOff, onChange])
  if (!keys) return null
  if (!keys.enabled) {
    // Answering encrypted mail from an account without encryption: say why
    // the quote is gone and how to reply encrypted, without a toggle.
    if (!sensitive) return null
    return (
      <span
        className="flex h-7 w-7 shrink-0 items-center justify-center text-amber-600 dark:text-amber-400"
        title={t('crypto.toggle.accountOff')}
        aria-label={t('crypto.toggle.accountOff')}
      >
        <LockOpen size={15} />
      </span>
    )
  }

  const unavailable = keys.own_key_unusable || keys.missing.length > 0
  // Missing keys can be looked up from here, on this click only. A sensitive
  // message that stays on offers the lookup first; once it found nothing, the
  // next click is the deliberate "turn off" (a reply) or nothing (locked).
  const canLookUp = !keys.own_key_unusable && keys.missing.length > 0 && (!on || (!!sensitive && !lookingUp))
  const lookUp = () => {
    setLookingUp(true)
    crypto$.lookup.set({ addrs: keys.missing.map((m) => m.addr) })
  }
  const locked = on && (sensitive === 'forward' || bodyIsDecrypted)
  let title = on ? t('crypto.toggle.on') : t('crypto.toggle.off')
  if (keys.own_key_unusable) title = t('crypto.toggle.ownKeyUnusable')
  else if (unavailable && on)
    title = canLookUp
      ? t('crypto.toggle.blockedLookUp', { addresses: missing })
      : locked
        ? t('crypto.toggle.blockedLocked', { addresses: missing })
        : t('crypto.toggle.blocked', { addresses: missing })
  else if (unavailable)
    title = `${t('crypto.toggle.noKey', { addresses: missing })} ${t('crypto.toggle.clickToLookUp')}`
  else if (locked)
    title = sensitive === 'forward' ? t('crypto.toggle.forwardLocked') : t('crypto.toggle.decryptedLocked')
  const changed = on && !unavailable ? (keys.changed ?? []) : []
  if (changed.length > 0) title = t('crypto.toggle.keyChanged', { addresses: changed.map((c) => c.addr).join(', ') })
  if (quoteWithheld) title = `${/[.!?]$/.test(title) ? title : `${title}.`} ${t('crypto.quoteWithheld')}`

  const click = async () => {
    if (canLookUp) {
      lookUp()
      return
    }
    if (locked) return
    if (!on) {
      if (!unavailable) {
        setUserOff(false)
        onChange(true)
      }
      return
    }
    if (sensitive === 'reply') {
      const ok = await confirmAction({
        title: t('crypto.toggle.offTitle'),
        message: t('crypto.toggle.offMessage'),
        confirmLabel: t('crypto.toggle.offConfirm'),
      })
      if (!ok) return
    }
    setUserOff(true)
    onChange(false)
  }

  const box = size === 'md' ? 'h-8 w-8' : 'h-7 w-7'
  const tone = on
    ? unavailable || changed.length > 0
      ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10'
      : 'text-accent bg-accent/10'
    : // Off, with or without keys: the same grey as the attachment and full-editor
      // buttons beside it (a dimmed lock looked paler than them in light mode).
      'text-secondary hover:bg-active hover:text-primary'
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={title}
      title={title}
      disabled={!on && keys.own_key_unusable}
      onClick={() => void click()}
      className={`flex ${box} shrink-0 items-center justify-center rounded-lg transition-colors cursor-pointer ${tone}`}
    >
      {on ? <Lock size={15} strokeWidth={2.4} /> : <LockOpen size={15} />}
    </button>
  )
}
