import { useState } from 'react'
import type { MouseEvent } from 'react'
import { BadgeCheck, Lock, Signature } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { TONE_CLASS, formatFingerprint, indicatorFor } from '../../lib/crypto'
import type { MessageCrypto } from '../../lib/crypto'
import { crypto$ } from '../../states/crypto'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

type T = ReturnType<typeof useTranslation>['t']

/** One sentence on who stands behind the message, for the tooltip and popover. */
export function trustSentence(crypto: MessageCrypto, sender: string, t: T): string {
  if (crypto.own) return t('crypto.trust.own')
  if (crypto.problem === 'invalid_signature') return t('crypto.trust.invalid')
  if (crypto.problem === 'key_revoked') return t('crypto.trust.keyRevoked', { sender })
  if (crypto.problem === 'key_expired') return t('crypto.trust.keyExpired', { sender })
  if (crypto.problem === 'signer_mismatch') return t('crypto.trust.signerMismatch', { sender })
  if (crypto.problem === 'redirected') return t('crypto.trust.redirected')
  if (crypto.verified_conflict) return t('crypto.trust.verifiedConflict', { sender })
  if (crypto.weak_hash) return t('crypto.trust.weakHash')
  if (crypto.partial && crypto.on_list) return t('crypto.trust.listChanged')
  if (crypto.partial) return t('crypto.trust.partial')
  if (crypto.trust === 'verified') return t('crypto.trust.verified', { sender })
  if (crypto.trust === 'known') return t('crypto.trust.known', { sender })
  if (!crypto.signed) return t('crypto.trust.unsigned')
  return t('crypto.trust.unknownKey')
}

/**
 * The end-to-end encryption indicator in a message header: a padlock for
 * encrypted mail, a signature mark for signed-only mail, coloured by how far
 * the sender's key is trusted (muted green verified, neutral known, amber not
 * confirmed, rose a problem). Drawn by the app, never inside the message,
 * so a message can't fake it. Ordinary mail renders nothing.
 */
export function CryptoBadge({
  crypto,
  accountId,
  senderName,
  senderAddress,
  size = 15,
}: {
  crypto?: MessageCrypto | null
  accountId: string
  senderName: string
  senderAddress: string
  size?: number
}) {
  const { t } = useTranslation()
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)
  const indicator = indicatorFor(crypto)
  if (!crypto || !indicator) return null

  const sender = senderName || senderAddress
  const what = crypto.encrypted ? t('crypto.badge.encrypted') : t('crypto.badge.signed')
  const who = trustSentence(crypto, sender, t)
  const label = `${what} · ${who}`
  const Icon = indicator.kind === 'lock' ? Lock : Signature
  const fingerprint = crypto.signer || crypto.issuer || ''
  const canVerify = !!crypto.signer && crypto.signer_stored && crypto.trust === 'known' && !crypto.own

  const open = (event: MouseEvent<HTMLButtonElement>) => {
    // Headers collapse the message on click; the badge opens its details.
    event.stopPropagation()
    const rect = event.currentTarget.getBoundingClientRect()
    setAnchor({ x: rect.right, y: rect.bottom + 4 })
  }

  return (
    <>
      <button
        type="button"
        onClick={open}
        title={label}
        aria-label={label}
        data-tone={indicator.tone}
        className={`flex items-center justify-center rounded p-0.5 hover:bg-black/[0.05] dark:hover:bg-white/[0.08] cursor-pointer transition-colors ${TONE_CLASS[indicator.tone]}`}
      >
        <Icon size={size} />
      </button>
      {anchor && (
        <FloatingContextMenu
          x={anchor.x}
          y={anchor.y}
          onClose={() => setAnchor(null)}
          onClick={(event) => event.stopPropagation()}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] w-[300px] bg-header shadow-xl"
        >
          <div className="flex flex-col gap-1.5 px-3 py-2.5 select-text" role="status">
            <div className={`flex items-center gap-2 text-[0.75rem] font-semibold ${TONE_CLASS[indicator.tone]}`}>
              <Icon size={14} className="shrink-0" />
              <span>{what}</span>
            </div>
            <p className="text-[0.71875rem] leading-snug text-primary">{who}</p>
            {senderAddress && <p className="text-[0.65625rem] text-secondary break-all">{senderAddress}</p>}
            {fingerprint && (
              <p
                className="font-mono text-[0.65625rem] leading-relaxed text-secondary break-words"
                aria-label={t('crypto.fingerprint')}
              >
                {formatFingerprint(fingerprint)}
              </p>
            )}
          </div>
          {canVerify && (
            <MenuItem
              icon={<BadgeCheck size={13} className="text-verified" />}
              label={t('crypto.verifyKeyOf', { sender })}
              onClick={() => {
                setAnchor(null)
                crypto$.verify.set({ addr: senderAddress, name: sender, fingerprint: crypto.signer ?? '', accountId })
              }}
            />
          )}
        </FloatingContextMenu>
      )}
    </>
  )
}
