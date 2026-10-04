import { Loader2, Maximize2, Paperclip, Send } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openReplyInFullEditor } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { isQuickReplyBlank } from '../../states/quickReply'
import { sendShortcutLabel, settings$ } from '../../states/settings'
import { useQuickReply } from './useQuickReply'
import { QuickReplyAttachments } from './QuickReplyAttachments'
import { QuickReplyFrom } from './QuickReplyFrom'
import { QuickReplyRecipients } from './QuickReplyRecipients'

export function QuickReplyComposer() {
  const { t } = useTranslation()
  const spellCheck = useValue(settings$.spellCheck)
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

  // A box holding only its seeded signature counts as empty. The check peeks
  // rather than subscribing, but `composer` is rendered below, so every
  // keystroke re-renders this and re-evaluates it.
  const canSend = !sendingReply && !isQuickReplyBlank()

  return (
    <footer className="px-3.5 pb-3.5 pt-1 bg-chat z-10 flex flex-col items-center justify-center">
      <div className="flex flex-col gap-2 w-full bg-chats p-2 rounded-2xl border border-border/60 shadow-sm focus-within:ring-1 focus-within:ring-accent transition-all duration-150">
        {/* One line, not two: From and To are both single-address disclosures,
            and stacking them pushed the box itself down the window. Recipients
            lead — they are what changes from thread to thread — and the send-as
            address sits out of the way at the right edge. */}
        <div className="flex w-full min-w-0 items-center gap-2 empty:hidden">
          <QuickReplyRecipients />
          <QuickReplyFrom />
        </div>
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
            onClick={() => openReplyInFullEditor()}
            className="flex h-8.5 w-8.5 shrink-0 items-center justify-center rounded-xl text-secondary hover:bg-active transition-colors cursor-pointer"
            title={t('composer.actions.openFullEditor')}
          >
            <Maximize2 size={15} />
          </button>

          {/* One line of text sits centred in the 34px row beside the buttons at any
            message text size: the padding is what the line height leaves over. */}
          <textarea
            ref={textareaRef}
            value={composer}
            onChange={(event) => compose$.composer.set(event.target.value)}
            placeholder={t('composer.placeholders.quickMessage')}
            rows={1}
            spellCheck={spellCheck}
            className="flex-1 py-[max(0px,calc((2.125rem_-_1.25rem_*_var(--me-message-scale))_/_2))] px-1 max-h-[254px] min-h-8.5 bg-transparent text-[calc(0.9375rem*var(--me-message-scale))] text-primary resize-none placeholder-secondary border-none outline-none leading-[1.3333]"
            onKeyDown={handleComposerKeyDown}
            onPaste={handleComposerPaste}
          />

          <button
            onClick={handleSendReply}
            disabled={!canSend}
            className={`flex h-8.5 w-8.5 shrink-0 items-center justify-center rounded-full shadow transition-all ${
              sendingReply
                ? 'bg-accent text-accent-label cursor-wait'
                : canSend
                  ? 'bg-accent text-accent-label hover:scale-105 cursor-pointer'
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
