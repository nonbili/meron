import {
  CheckSquare,
  Copy,
  Download,
  ExternalLink,
  FilePen,
  Forward,
  Globe,
  Link2,
  Mail,
  MailOpen,
  Reply,
  ReplyAll,
  Printer,
  SquarePen,
  Star,
  Trash2,
} from 'lucide-react'
import { mailHtmlToText } from '../../lib/html'
import { useTranslation } from '../../lib/i18n'
import {
  openMessageTab,
  editAsNewMessage,
  forwardMessage,
  openDraftCompose,
  replyAllToMessage,
} from '../../states/compose'
import { messageCanReplyAll } from '../../states/composeReply'
import { printMail } from '../../lib/printMail'
import { copyAttachmentImage, downloadAttachment, saveMessageAsEml } from '../../states/mail'
import { markMessageReadState, starMessage } from '../../states/mailFlags'
import { isDraftFolder } from '../../states/mailFolders'
import { deleteMessage } from '../../states/mailMoves'
import { copyText, openExternal } from '../../lib/native'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import type { MessageContextMenuState } from '../../states/thread'
import type { Message } from '../../types'

// Right-click menu for a message (or a link inside it). Clamps itself inside the
// viewport on mount since clientX/clientY can land near the right/bottom edge.
export function MessageContextMenu({
  state,
  isRSS,
  headerOnly = false,
  onClose,
  onSelectMessage,
  onMessageChange,
  onReply,
  onPrint,
  onDeleted,
}: {
  state: MessageContextMenuState
  isRSS: boolean
  /** The message carries headers only (a starred-list row): hide the actions
   * that need its body (forward / edit as new). */
  headerOnly?: boolean
  onClose: () => void
  onSelectMessage?: (message: Message) => void
  /** Hears the read and star changes the menu makes once they succeed, for a
   * holder of its own copy of the message (a reader tab), which the thread's
   * update can miss. */
  onMessageChange?: (patch: Partial<Message>) => void
  /** Offers Reply, where no quick reply sits under the message (a reader tab). */
  onReply?: () => void
  /** Prints in place of the default, for a caller that knows more about it
   * (a reader tab and the remote content it shows). */
  onPrint?: () => void
  onDeleted?: () => void
}) {
  const { t } = useTranslation()
  const isDraft = isDraftFolder(state.message.folder_id, state.message.account_id)
  return (
    <FloatingContextMenu
      x={state.x}
      y={state.y}
      onClose={onClose}
      overlay
      overlayClassName="fixed inset-0 z-[60]"
      className="fixed z-[61] min-w-[180px] bg-header shadow-xl"
    >
      {state.linkUrl || state.image ? (
        <>
          {state.linkUrl && (
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
                  copyText(state.linkUrl!).catch(() => undefined)
                  onClose()
                }}
              />
            </>
          )}
          {/* A linked picture offers both. */}
          {state.image && (
            <>
              <MenuItem
                icon={<Download size={13} className="text-accent" />}
                label={t('chat.actions.saveImage')}
                onClick={() => {
                  const image = state.image!
                  onClose()
                  void downloadAttachment({
                    ...image,
                    mime: image.mime ?? state.message.attachments?.find((att) => att.key === image.key)?.mime,
                  })
                }}
              />
              <MenuItem
                icon={<Copy size={13} className="text-accent" />}
                label={t('chat.actions.copyImage')}
                onClick={() => {
                  const image = state.image!
                  onClose()
                  void copyAttachmentImage(image)
                }}
              />
            </>
          )}
        </>
      ) : (
        <>
          {(isDraft || !state.hideOpenInNewTab) && (
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
          {isRSS && state.message.link && (
            <>
              <MenuItem
                icon={<Globe size={13} className="text-accent" />}
                label={t('chat.actions.openLink')}
                onClick={() => {
                  openExternal(state.message.link!)
                  onClose()
                }}
              />
              <MenuItem
                icon={<Link2 size={13} className="text-accent" />}
                label={t('chat.actions.copyLinkAddress')}
                onClick={() => {
                  copyText(state.message.link!).catch(() => undefined)
                  onClose()
                }}
              />
            </>
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
              // Only once it took: a failed change is rolled back in the
              // conversation, and a copy patched ahead would keep it.
              void markMessageReadState(message, message.unread).then(() =>
                onMessageChange?.({ unread: !message.unread }),
              )
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
              void starMessage(message, !message.starred).then(() => onMessageChange?.({ starred: !message.starred }))
            }}
          />
          {onReply && !isDraft && !isRSS && (
            <MenuItem
              icon={<Reply size={13} className="text-accent" />}
              label={t('chat.actions.reply')}
              onClick={() => {
                onClose()
                onReply()
              }}
            />
          )}
          {!isDraft && !isRSS && messageCanReplyAll(state.message) && (
            <MenuItem
              icon={<ReplyAll size={13} className="text-accent" />}
              label={t('chat.actions.replyAll')}
              onClick={() => {
                const message = state.message
                onClose()
                replyAllToMessage(message)
              }}
            />
          )}
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
              icon={<FilePen size={13} className="text-accent" />}
              label={t('chat.actions.editAsNewMessage')}
              onClick={() => {
                const message = state.message
                onClose()
                void editAsNewMessage(message)
              }}
            />
          )}
          {!headerOnly && !state.message.body_missing && (
            <MenuItem
              icon={<Copy size={13} className="text-accent" />}
              label={t('chat.copyMessageText')}
              onClick={() => {
                copyText(messagePlainText(state.message)).catch(() => undefined)
                onClose()
              }}
            />
          )}
          {!isDraft && !isRSS && (
            <MenuItem
              icon={<Download size={13} className="text-accent" />}
              label={t('chat.actions.saveAsEml')}
              onClick={() => {
                const message = state.message
                onClose()
                void saveMessageAsEml(message)
              }}
            />
          )}
          {!headerOnly && !state.message.body_missing && !isRSS && (
            <MenuItem
              icon={<Printer size={13} className="text-accent" />}
              label={t('chat.actions.print')}
              onClick={() => {
                onClose()
                if (onPrint) onPrint()
                else void printMail(state.message)
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
                void deleteMessage(message).then((deleted) => {
                  if (deleted) onDeleted?.()
                })
              }}
            />
          )}
        </>
      )}
    </FloatingContextMenu>
  )
}

/** The message as plain text: its text body, or the rich body's text when it has none. */
function messagePlainText(message: Message): string {
  if (message.body.trim()) return message.body
  return message.body_html ? mailHtmlToText(message.body_html).trim() : ''
}
