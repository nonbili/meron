import { CheckSquare, Copy, ExternalLink, Forward, Link2, Mail, MailOpen, SquarePen, Star, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { openMessageTab, editAsNewMessage, forwardMessage, openDraftCompose } from '../../states/compose'
import { deleteMessage, isDraftFolder, markMessageReadState, starMessage } from '../../states/mail'
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
  onSelectMessage,
}: {
  state: MessageContextMenuState
  isRSS: boolean
  /** The message carries headers only (a starred-list row): hide the actions
   * that need its body (forward / edit as new). */
  headerOnly?: boolean
  onClose: () => void
  onSelectMessage?: (message: Message) => void
}) {
  const { t } = useTranslation()
  const isDraft = isDraftFolder(state.message.folder_id)
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
            onClick={() => {
              openExternal(state.linkUrl!)
              onClose()
            }}
          />
          <MenuItem
            icon={<Link2 size={13} className="text-accent" />}
            label={t('chat.actions.copyLinkAddress')}
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
              icon={
                isDraft ? (
                  <SquarePen size={13} className="text-accent" />
                ) : (
                  <ExternalLink size={13} className="text-accent" />
                )
              }
              label={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
              onClick={() => {
                if (isDraft) {
                  void openDraftCompose(state.message)
                } else {
                  openMessageTab(state.message)
                }
                onClose()
              }}
            />
          )}
          {onSelectMessage && (
            <MenuItem
              icon={<CheckSquare size={13} className="text-accent" />}
              label={t('buttons.select', { defaultValue: 'Select' })}
              onClick={() => {
                onSelectMessage(state.message)
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
            onClick={() => {
              const message = state.message
              onClose()
              void starMessage(message, !message.starred)
            }}
          />
          {!isDraft && !isRSS && !headerOnly && (
            <MenuItem
              icon={<Forward size={13} className="text-accent" />}
              label={t('chat.actions.forward')}
              onClick={() => {
                const message = state.message
                onClose()
                void forwardMessage(message)
              }}
            />
          )}
          {!isDraft && !isRSS && !headerOnly && (
            <MenuItem
              icon={<Copy size={13} className="text-accent" />}
              label={t('chat.actions.editAsNewMessage')}
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
              label={isDraft ? t('chat.actions.discardDraft') : t('chat.actions.deleteMessage')}
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
