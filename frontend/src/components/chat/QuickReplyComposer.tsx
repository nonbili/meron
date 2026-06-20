import { Loader2, Maximize2, Paperclip, Send } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { compose$, openReplyInFullEditor } from '../../states/compose'
import { sendShortcutLabel } from '../../states/settings'
import { useQuickReply } from './useQuickReply'
import { QuickReplyAttachments } from './QuickReplyAttachments'

export function QuickReplyComposer() {
  const { t } = useTranslation()
  const {
    composer,
    composerAttachments,
    sendShortcut,
    sendingReply,
    textareaRef,
    handleSendReply,
    pickAttachmentFiles,
    handleComposerPaste,
    handleComposerKeyDown,
  } = useQuickReply()

  const canSend = !sendingReply && (composer.trim().length > 0 || composerAttachments.length > 0)

  return (
    <footer className="p-3.5 bg-header border-t border-border z-10 flex flex-col items-center justify-center">
      <div className="flex flex-col gap-2 w-full bg-hover p-2 rounded-2xl border border-border/50 shadow-sm focus-within:ring-1 focus-within:ring-accent focus-within:bg-chats transition-all duration-150">
        <QuickReplyAttachments attachments={composerAttachments} />

        <div className="flex items-end gap-2 w-full">
          <button
            onClick={() => void pickAttachmentFiles()}
            className="flex h-8.5 w-8.5 shrink-0 items-center justify-center rounded-xl text-secondary hover:bg-active transition-colors cursor-pointer"
            title={t('composer.actions.attachFiles')}
          >
            <Paperclip size={16} />
          </button>
          <button
            onClick={openReplyInFullEditor}
            className="flex h-8.5 w-8.5 shrink-0 items-center justify-center rounded-xl text-secondary hover:bg-active transition-colors cursor-pointer"
            title={t('composer.actions.openFullEditor')}
          >
            <Maximize2 size={15} />
          </button>

          <textarea
            ref={textareaRef}
            value={composer}
            onChange={(event) => compose$.composer.set(event.target.value)}
            placeholder={t('composer.placeholders.quickMessage')}
            rows={1}
            className="flex-1 py-[7px] px-1 max-h-[254px] min-h-8.5 bg-transparent text-[15px] text-primary resize-none placeholder-secondary border-none outline-none leading-5"
            onKeyDown={handleComposerKeyDown}
            onPaste={handleComposerPaste}
          />

          <button
            onClick={handleSendReply}
            disabled={!canSend}
            className={`flex h-8.5 w-8.5 shrink-0 items-center justify-center rounded-full shadow transition-all ${
              sendingReply
                ? 'bg-accent text-white cursor-wait'
                : canSend
                  ? 'bg-accent text-white hover:scale-105 cursor-pointer'
                  : 'bg-active text-secondary/70 cursor-not-allowed'
            }`}
            title={
              sendingReply
                ? t('composer.status.sending')
                : t('composer.actions.sendMessageWithShortcut', { shortcut: sendShortcutLabel(sendShortcut) })
            }
          >
            {sendingReply ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Send size={13} className="relative left-[0.5px]" />
            )}
          </button>
        </div>
      </div>
    </footer>
  )
}
