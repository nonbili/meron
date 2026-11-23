import { Loader2, Maximize2, Paperclip, Send } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openReplyInFullEditor, replyQuoteWithheld } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { isQuickReplyBlank, scheduleQuickReplyDraftSave, setQuickReplySign } from '../../states/quickReply'
import { sendShortcutLabel, settings$ } from '../../states/settings'
import { useQuickReply } from './useQuickReply'
import { QuickReplyAttachments } from './QuickReplyAttachments'
import { QuickReplyFrom } from './QuickReplyFrom'
import { QuickReplyRecipients } from './QuickReplyRecipients'
import { QuickReplySignatureToggle } from './QuickReplySignatureToggle'
import {
  quickReplyEncrypts,
  quickReplyHoldsEncryptedDraft,
  quickReplyRecipients,
  quickReplySigns,
  quickReplyTarget,
} from '../../states/composeReply'
import { EncryptToggle, encryptionBlocksSend, recipientAddresses, useRecipientKeys } from '../crypto/EncryptToggle'
import { SignToggle } from '../crypto/SignToggle'

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

  // Whether the box is blank is peeked rather than subscribed, but `composer`
  // is rendered below, so every keystroke re-renders this and re-evaluates it.
  const target = useValue(quickReplyTarget)
  const { to, cc } = useValue(quickReplyRecipients)
  const encrypt = useValue(quickReplyEncrypts)
  const sign = useValue(quickReplySigns)
  const offByUser = useValue(() => {
    const choice = compose$.quickReplyEncrypt.get()
    return !!target && choice?.threadId === target.thread_id && !choice.on && !!choice.byUser
  })
  const keys = useRecipientKeys(target?.account_id ?? '', recipientAddresses(to, cc))
  const encryptedDraft = useValue(() => quickReplyHoldsEncryptedDraft(target?.thread_id))
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

        {/* Extend the textarea through the box's right padding for the scrollbar.
            Keep that padding inside the textarea to preserve the text inset. */}
        <textarea
          ref={textareaRef}
          value={composer}
          onChange={(event) => compose$.composer.set(event.target.value)}
          placeholder={t('composer.placeholders.quickMessage')}
          rows={1}
          spellCheck={spellCheck}
          className="-mr-2 pl-1.5 pr-3.5 max-h-[254px] bg-transparent text-[calc(0.9375rem*var(--me-message-scale))] text-primary resize-none placeholder-secondary border-none outline-none leading-[1.3333]"
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
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
            title={t('composer.actions.attachFiles')}
          >
            <Paperclip size={15} />
          </button>
          <button
            onClick={() => openReplyInFullEditor()}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
            title={t('composer.actions.openFullEditor')}
          >
            <Maximize2 size={14} />
          </button>
          {target && (
            <EncryptToggle
              key={target.thread_id}
              // The full composer's size, so the two look alike.
              size="md"
              keys={keys}
              hasRecipients={!!(to || cc).trim()}
              on={encrypt}
              sensitive={sensitive}
              // A draft saved encrypted may quote decrypted mail in its text:
              // it is only switched to plain on purpose.
              encryptedDraft={encryptedDraft}
              // Switched off for this reply (signed only, say): coming back to
              // the thread must not turn it on again by itself.
              offByUser={offByUser}
              quoteWithheld={quoteWithheld}
              onChange={(on, byUser) => {
                const optOut = !on && !!byUser
                compose$.quickReplyEncrypt.set({
                  threadId: target.thread_id,
                  on,
                  draft: encryptedDraft || undefined,
                  byUser: optOut || undefined,
                })
                // Switched back on: the saved copy may be plain by now (a
                // draft loaded encrypted, switched off and autosaved), and the
                // encryption watcher skips such drafts.
                if (on && byUser && !isQuickReplyBlank()) scheduleQuickReplyDraftSave()
                if (!optOut) return
                // Switched off on purpose, the sign switch's state is saved with
                // the draft, which is what brings the opt-out back when it is
                // reopened.
                if (sign === undefined && keys?.enabled && !keys.own_key_unusable)
                  setQuickReplySign(target.thread_id, !!keys.sign_plain)
                else if (!isQuickReplyBlank()) scheduleQuickReplyDraftSave()
              }}
            />
          )}
          {/* Only a usable key can sign; a reply already set to sign (a
              draft saved before the key became unusable) keeps the switch to
              turn that off, or the core would refuse to send it. */}
          {target && keys && ((keys.enabled && !keys.own_key_unusable) || sign === true) && (
            <SignToggle
              size="md"
              on={sign ?? !!keys.sign_plain}
              encrypted={encrypt}
              onChange={(on) => setQuickReplySign(target.thread_id, on)}
            />
          )}
          <QuickReplySignatureToggle />
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
