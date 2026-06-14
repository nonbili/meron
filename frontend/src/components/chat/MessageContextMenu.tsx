import { Copy, ExternalLink, Forward, Link2, Mail, MailOpen, Star, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { openMessageTab, editAsNewMessage, forwardMessage } from '../../states/compose'
import { deleteMessage, markMessageReadState, starMessage } from '../../states/mail'
import { openExternal } from '../../lib/native'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import type { Message } from '../../types'

export type MessageContextMenuState = {
  x: number
  y: number
  message: Message
  linkUrl?: string
  hideOpenInNewTab?: boolean
}

// Right-click menu for a message (or a link inside it). Clamps itself inside the
// viewport on mount since clientX/clientY can land near the right/bottom edge.
export function MessageContextMenu({
  state,
  isRSS,
  headerOnly = false,
  onClose,
}: {
  state: MessageContextMenuState
  isRSS: boolean
  /** The message carries headers only (a starred-list row): hide the actions
   * that need its body (forward / edit as new). */
  headerOnly?: boolean
  onClose: () => void
}) {
  const { t } = useTranslation()
  return (
    <FloatingContextMenu
      x={state.x}
      y={state.y}
      onClose={onClose}
      overlay
      overlayClassName="fixed inset-0 z-[60]"
      className="fixed z-[61] min-w-[180px] rounded-xl border border-border bg-header p-1 shadow-xl"
    >
      {state.linkUrl ? (
        <>
          <MenuItem
            icon={<ExternalLink size={13} className="text-accent" />}
            label={t('chat.actions.openLink')}
            className="whitespace-nowrap"
            onClick={() => {
              openExternal(state.linkUrl!)
              onClose()
            }}
          />
          <MenuItem
            icon={<Link2 size={13} className="text-accent" />}
            label={t('chat.actions.copyLinkAddress')}
            className="whitespace-nowrap"
            onClick={() => {
              navigator.clipboard?.writeText(state.linkUrl!).catch(() => undefined)
              onClose()
            }}
          />
        </>
      ) : (
        <>
          {!state.hideOpenInNewTab && (
            <MenuItem
              icon={<ExternalLink size={13} className="text-accent" />}
              label={t('threads.actions.openInNewTab')}
              className="whitespace-nowrap"
              onClick={() => {
                openMessageTab(state.message)
                onClose()
              }}
            />
          )}
          <MenuItem
            icon={
              state.message.unread ? (
                <MailOpen size={13} className="text-accent" />
              ) : (
                <Mail size={13} className="text-accent" />
              )
            }
            label={state.message.unread ? t('threads.actions.markAsRead') : t('threads.actions.markAsUnread')}
            className="whitespace-nowrap"
            onClick={() => {
              const message = state.message
              onClose()
              void markMessageReadState(message, message.unread)
            }}
          />
          <MenuItem
            icon={
              <Star size={13} className={state.message.starred ? 'fill-amber-500 text-amber-500' : 'text-accent'} />
            }
            label={state.message.starred ? t('chat.unstar') : t('chat.star')}
            className="whitespace-nowrap"
            onClick={() => {
              const message = state.message
              onClose()
              void starMessage(message, !message.starred)
            }}
          />
          {!isRSS && !headerOnly && (
            <MenuItem
              icon={<Forward size={13} className="text-accent" />}
              label={t('chat.actions.forward')}
              className="whitespace-nowrap"
              onClick={() => {
                const message = state.message
                onClose()
                void forwardMessage(message)
              }}
            />
          )}
          {!isRSS && !headerOnly && (
            <MenuItem
              icon={<Copy size={13} className="text-accent" />}
              label={t('chat.actions.editAsNewMessage')}
              className="whitespace-nowrap"
              onClick={() => {
                const message = state.message
                onClose()
                void editAsNewMessage(message)
              }}
            />
          )}
          {!isRSS && (
            <MenuItem
              danger
              icon={<Trash2 size={13} />}
              label={t('chat.actions.deleteMessage')}
              className="whitespace-nowrap"
              onClick={() => {
                const message = state.message
                onClose()
                void deleteMessage(message)
              }}
            />
          )}
        </>
      )}
    </FloatingContextMenu>
  )
}
