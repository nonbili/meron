import { useState } from 'react'
import type { MouseEvent } from 'react'
import { Image, ImageOff, UserCheck } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { revealMessageRemote } from '../../states/compose'
import { setRemoteImageSender } from '../../states/settings'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

/**
 * The whole blocked-remote-content affordance for one message: a tinted icon in
 * the header that opens the two reveal actions. It replaces the banner that
 * used to sit above every newsletter body — on a newsletter-heavy inbox that
 * banner showed on nearly every message.
 *
 * Renders nothing when the message has no blocked remote content.
 */
export function BlockedRemoteButton({
  messageId,
  blocked,
  hiddenRemoteCount,
  senderAddress,
  size = 15,
}: {
  messageId: string
  /** Whether this message is holding remote content back. */
  blocked: boolean
  /** Blocked attachment images/videos, 0 when only the body is held back. */
  hiddenRemoteCount: number
  /** Bare From address, the key the sender allowlist is stored under; empty
   *  when there is no sender worth trusting (an outgoing message). */
  senderAddress: string
  size?: number
}) {
  const { t } = useTranslation()
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)

  if (!blocked) return null

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
        title={t('chat.remoteBlocked')}
        aria-label={t('chat.remoteBlocked')}
        // Tinted rather than muted: a grey glyph among the other header icons
        // read as decoration, and this one is the only sign that part of the
        // message is missing. The theme's accent at partial opacity — it tracks
        // light/dark (and a custom accent) without shouting the way a full
        // accent glyph next to the timestamp does.
        className="flex items-center justify-center gap-1 rounded p-0.5 text-accent/65 hover:bg-accent/10 hover:text-accent cursor-pointer transition-colors"
      >
        <ImageOff size={size} />
        {hiddenRemoteCount > 0 && (
          <span className="text-[0.625rem] font-semibold leading-none">{hiddenRemoteCount}</span>
        )}
      </button>
      {anchor && (
        <FloatingContextMenu
          x={anchor.x}
          y={anchor.y}
          onClose={() => setAnchor(null)}
          // Same reason as the overlay's: this menu lives inside a header that
          // collapses the message when clicked.
          onClick={(event) => event.stopPropagation()}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] min-w-[180px] rounded-xl border border-border bg-header p-1 shadow-xl"
        >
          <MenuItem
            icon={<Image size={13} className="text-accent" />}
            label={
              hiddenRemoteCount > 0 ? t('chat.showImages', { count: hiddenRemoteCount }) : t('chat.showRemoteContent')
            }
            onClick={() => {
              setAnchor(null)
              revealMessageRemote(messageId)
            }}
          />
          {!!senderAddress && (
            <MenuItem
              icon={<UserCheck size={13} className="text-accent" />}
              label={t('chat.allowRemoteFrom', { sender: senderAddress })}
              // The sender address can be long; truncate rather than let one
              // row stretch the whole menu.
              className="max-w-[280px]"
              trailing={<span />}
              onClick={() => {
                setAnchor(null)
                setRemoteImageSender(senderAddress, true)
              }}
            />
          )}
        </FloatingContextMenu>
      )}
    </>
  )
}
