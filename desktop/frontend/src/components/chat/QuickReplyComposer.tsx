import { Loader2, Maximize2, Paperclip, Send } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openReplyInFullEditor, replyQuoteWithheld } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { isQuickReplyBlank } from '../../states/quickReply'
import { sendShortcutLabel, settings$ } from '../../states/settings'
import { useQuickReply } from './useQuickReply'
import { QuickReplyAttachments } from './QuickReplyAttachments'
import { QuickReplyFrom } from './QuickReplyFrom'
import { QuickReplyRecipients } from './QuickReplyRecipients'
import { quickReplyEncrypts, quickReplyRecipients, quickReplyTarget } from '../../states/composeReply'
import { EncryptToggle, encryptionBlocksSend, recipientAddresses, useRecipientKeys } from '../crypto/EncryptToggle'

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
  const target = useValue(quickReplyTarget)
  const { to, cc } = useValue(quickReplyRecipients)
  const encrypt = useValue(quickReplyEncrypts)
  const keys = useRecipientKeys(target?.account_id ?? '', recipientAddresses(to, cc))
  const sensitive = target?.crypto?.encrypted ? ('reply' as const) : undefined
  // Subscribed so the tooltip follows the setting; the helper peeks.
  useValue(settings$.quoteReplies)
  const quoteWithheld = replyQuoteWithheld(target)
  const blocked = encryptionBlocksSend(encrypt, keys)
  const canSend = !sendingReply && !isQuickReplyBlank() && !blocked

  return (
    // The footer floats over the message list (MessagePane) and paints nothing
    // of its own, so the conversation runs on behind it and messages scroll off
    // under the box; MessagePane fades what shows around it.
    <footer className="px-3 pb-3 pt-1 flex flex-col items-center justify-center">
      <div
        onMouseDown={(event) => {
          // A press on the box's bare padding or the gaps of the control row
          // goes to the text, keeping its caret; controls, and the menus and
          // previews portalled out of the box, keep their own presses.
          const target = event.target as HTMLElement
          if (!event.currentTarget.contains(target)) return
          if (target.closest('button, a, input, textarea, select, [role], [tabindex]')) return
          event.preventDefault()
          textareaRef.current?.focus()
        }}
        className="pointer-events-auto flex flex-col gap-1 w-full bg-chats p-2 rounded-2xl border border-border/40 shadow-float focus-within:ring-1 focus-within:ring-accent transition-all duration-150"
      >
        <QuickReplyAttachments attachments={composerAttachments} />

        {/* The text leads and has the box's full width; everything else shares
            the one line under it, so a growing reply never strands a control
            beside it and a narrow pane gives no text width to buttons. */}
        <textarea
          ref={textareaRef}
          value={composer}
          onChange={(event) => compose$.composer.set(event.target.value)}
          placeholder={t('composer.placeholders.quickMessage')}
          rows={1}
          spellCheck={spellCheck}
          className="w-full px-1.5 max-h-[254px] bg-transparent text-[calc(0.9375rem*var(--me-message-scale))] text-primary resize-none placeholder-secondary border-none outline-none leading-[1.3333]"
          onKeyDown={handleComposerKeyDown}
          onPaste={handleComposerPaste}
        />

        {/* From and To are both single-address disclosures, kept to one line:
            the recipients sit against the send button, and the send-as address
            follows the buttons on the left, where it is the first to truncate
            when the pane narrows — recipients change from thread to thread. */}
        <div className="-mb-1 flex w-full min-w-0 items-center gap-1">
          <button
            onClick={() => void pickAttachmentFiles()}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
            title={t('composer.actions.attachFiles')}
          >
            <Paperclip size={15} />
          </button>
          <button
            onClick={() => openReplyInFullEditor()}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
            title={t('composer.actions.openFullEditor')}
          >
            <Maximize2 size={14} />
          </button>
          {target && (
            <EncryptToggle
              key={target.thread_id}
              keys={keys}
              hasRecipients={!!(to || cc).trim()}
              on={encrypt}
              sensitive={sensitive}
              quoteWithheld={quoteWithheld}
              onChange={(on) => compose$.quickReplyEncrypt.set({ threadId: target.thread_id, on })}
            />
          )}
          <QuickReplyFrom />
          <div className="ml-auto flex min-w-0 shrink items-center">
            <QuickReplyRecipients />
          </div>
          <button
            onClick={handleSendReply}
            disabled={!canSend}
            className={`ml-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full shadow transition-all ${
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
