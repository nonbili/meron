import { useState } from 'react'
import type { MouseEvent } from 'react'
import { Check, Image, ImageOff, UserCheck } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { revealMessageRemote } from '../../states/compose'
import { useValue } from '@legendapp/state/react'
import { normalizeSenderAddr, settings$, setRemoteImageSender } from '../../states/settings'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

export function RemoteContentButton({
  messageId,
  blocked,
  canReveal = blocked,
  hiddenRemoteCount,
  senderAddress,
  size = 15,
  onReveal,
}: {
  messageId?: string
  /** Whether this message is holding remote content back. */
  blocked: boolean
  canReveal?: boolean
  /** Blocked attachment images/videos, 0 when only the body is held back. */
  hiddenRemoteCount: number
  /** From address, or empty when sender management is unavailable. */
  senderAddress: string
  size?: number
  /** Standalone readers reveal content within their own session. */
  onReveal?: () => void
}) {
  const { t } = useTranslation()
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)

  const senders = useValue(settings$.remoteImageSenders)
  const sender = normalizeSenderAddr(senderAddress)
  const senderAllowed = senders.includes(sender)
  if (!canReveal && !sender) return null

  const label = t(
    blocked ? 'chat.remoteBlocked' : senderAllowed ? 'chat.remoteSenderAllowed' : 'settings.privacy.remoteSenders',
  )
  const StatusIcon = blocked ? ImageOff : senderAllowed ? Check : Image

  const openMenu = (event: MouseEvent<HTMLButtonElement>) => {
    // Both headers are click-to-collapse; the icon opens its menu instead.
    event.stopPropagation()
    const rect = event.currentTarget.getBoundingClientRect()
    setAnchor({ x: rect.right, y: rect.bottom + 4 })
  }

  return (
    <>
      <button
        type="button"
        onClick={openMenu}
        title={label}
        aria-label={label}
        disabled={senderAllowed && !canReveal}
        className="flex items-center justify-center gap-1 rounded p-0.5 text-accent/65 hover:bg-accent/10 hover:text-accent cursor-pointer transition-colors disabled:cursor-default disabled:text-secondary disabled:hover:bg-transparent"
      >
        <StatusIcon size={size} />
        {hiddenRemoteCount > 0 && (
          <span className="text-[0.625rem] font-semibold leading-none">{hiddenRemoteCount}</span>
        )}
      </button>
      {anchor && (canReveal || !senderAllowed) && (
        <FloatingContextMenu
          x={anchor.x}
          y={anchor.y}
          onClose={() => setAnchor(null)}
          // Same reason as the overlay's: this menu lives inside a header that
          // collapses the message when clicked.
          onClick={(event) => event.stopPropagation()}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] min-w-[180px] bg-header shadow-xl"
        >
          {canReveal && (
            <MenuItem
              icon={<Image size={13} className="text-accent" />}
              label={
                hiddenRemoteCount > 0 ? t('chat.showImages', { count: hiddenRemoteCount }) : t('chat.showRemoteContent')
              }
              onClick={() => {
                setAnchor(null)
                if (onReveal) onReveal()
                else if (messageId) revealMessageRemote(messageId)
              }}
            />
          )}
          {!!sender && (
            <MenuItem
              icon={senderAllowed ? <Check size={13} /> : <UserCheck size={13} className="text-accent" />}
              label={senderAllowed ? t('chat.remoteSenderAllowed') : t('chat.allowRemoteFrom', { sender })}
              disabled={senderAllowed}
              // The sender address can be long; truncate rather than let one
              // row stretch the whole menu.
              className="max-w-[280px]"
              trailing={<span />}
              onClick={() => {
                setAnchor(null)
                setRemoteImageSender(sender, true)
              }}
            />
          )}
        </FloatingContextMenu>
      )}
    </>
  )
}
