import { Forward, Reply, ReplyAll, Trash2 } from 'lucide-react'
import type { ReactNode } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { forwardMessage, replyToMessage } from '../../states/compose'
import { actionBarTarget, messageCanReplyAll } from '../../states/composeReply'
import { deleteThread } from '../../states/mailMoves'
import type { Message } from '../../types'

// Reply, reply all, forward and delete as one floating row of buttons: at the
// foot of a reader tab, and under a conversation when the setting puts them
// there instead of the quick-reply box.
export function ActionBar({
  message,
  forwardDisabled,
  onReply,
  onReplyAll,
  onForward,
  onDelete,
}: {
  /** The message reply and forward act on; without one only delete shows. */
  message: Message | null
  /** Forwarding needs the body and its attachments, so it waits for them. */
  forwardDisabled: boolean
  onReply: () => void
  onReplyAll: () => void
  onForward: () => void
  onDelete: () => void
}) {
  const { t } = useTranslation()
  return (
    <footer className="pointer-events-auto flex flex-wrap items-center justify-center gap-2 rounded-2xl border border-border/40 bg-chats p-1.5 shadow-float select-none">
      {message && (
        <>
          <ActionBarButton icon={<Reply size={15} />} label={t('chat.actions.reply')} onClick={onReply} />
          {messageCanReplyAll(message) && (
            <ActionBarButton icon={<ReplyAll size={15} />} label={t('chat.actions.replyAll')} onClick={onReplyAll} />
          )}
          <ActionBarButton
            icon={<Forward size={15} />}
            label={t('chat.actions.forward')}
            disabled={forwardDisabled}
            onClick={onForward}
          />
        </>
      )}
      <ActionBarButton icon={<Trash2 size={15} />} label={t('buttons.delete')} danger onClick={onDelete} />
    </footer>
  )
}

// The bar under a conversation (the conversation-footer setting's "actions").
// Reply and forward take the newest message received (actionBarTarget) and
// open the full editor, as a message's own menu does. Delete takes the whole
// conversation, as the header's does: the bar sits under all of it.
export function ConversationActionBar({ thread }: { thread: Message }) {
  const target = useValue(actionBarTarget)
  return (
    <div className="px-3 pb-3 pt-1">
      <ActionBar
        message={target}
        forwardDisabled={!!target?.body_missing}
        onReply={() => target && replyToMessage(target)}
        onReplyAll={() => target && replyToMessage(target, true)}
        onForward={() => target && void forwardMessage(target)}
        // As the header's delete: deleteThread finds the thread's folder, and
        // whether the delete is permanent and has to be confirmed.
        onDelete={() => void deleteThread(thread.thread_id)}
      />
    </div>
  )
}

function ActionBarButton({
  icon,
  label,
  danger = false,
  disabled = false,
  onClick,
}: {
  icon: ReactNode
  label: string
  danger?: boolean
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold cursor-pointer hover:bg-hover disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent ${
        danger ? 'text-red-500' : 'text-secondary hover:text-primary'
      }`}
    >
      {icon}
      {label}
    </button>
  )
}
