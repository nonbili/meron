import { useState } from 'react'
import type { MouseEvent } from 'react'
import { useTranslation } from '../../lib/i18n'
import { AlertCircle, Check, ChevronDown, ExternalLink, Loader2, MoreHorizontal, Star } from 'lucide-react'

import { openDraftCompose, openMessageTab, retrySend } from '../../states/compose'
import type { Message } from '../../types'
import { formatFullTimestamp, formatMessageStamp } from './messageHelpers'
import { AddressRow } from './AddressList'
import { MessageContent } from './MessageContent'
import { useMessageView } from './useMessageView'
import type { MessageContextMenuState } from './MessageContextMenu'

interface MessageBubbleProps {
  message: Message
  // Index of this bubble's first image within the thread-wide gallery list.
  galleryOffset: number
  onOpenContextMenu: (state: MessageContextMenuState) => void
  onLinkHover?: (url: string | null) => void
}

export function MessageBubble({ message, galleryOffset, onOpenContextMenu, onLinkHover }: MessageBubbleProps) {
  const { t } = useTranslation()
  const [metaOpen, setMetaOpen] = useState(false)
  const view = useMessageView(message)
  const {
    outgoing,
    isDraft,
    isRSS,
    useHtmlBody,
    recipientSummary,
    fromRaw,
    toRaw,
    ccRaw,
    bccRaw,
    replyToRaw,
    replyToDiffers,
  } = view

  const openActionsMenu = (event: MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    onOpenContextMenu({ x: rect.right, y: rect.bottom + 4, message, hideOpenInNewTab: true })
  }
  const openMessageOrDraftTab = () => {
    if (isDraft) {
      void openDraftCompose(message)
      return
    }
    openMessageTab(message)
  }

  return (
    <div className={`flex w-full animate-slide-up ${outgoing ? 'justify-end' : 'justify-start'}`}>
      <div
        className={`group/message-bubble relative ${useHtmlBody ? 'w-[70%]' : 'max-w-[70%]'} min-w-[100px] p-3.5 border transition-shadow duration-200 ${
          isDraft
            ? 'bg-bubble-out/55 text-bubble-out-text/80 border-dashed border-accent/45 rounded-2xl rounded-tr-sm shadow-none'
            : outgoing
              ? 'bg-bubble-out text-bubble-out-text border-border/35 rounded-2xl rounded-tr-sm shadow-bubble-out'
              : 'bg-bubble-in text-bubble-in-text border-border/40 rounded-2xl rounded-tl-sm shadow-bubble-in'
        }`}
      >
        <div className="absolute right-2 -top-3.5 z-20 flex items-center gap-1 rounded-full border border-border/40 bg-header/95 p-0.5 text-secondary opacity-0 shadow-sm transition-opacity group-hover/message-bubble:opacity-100 focus-within:opacity-100">
          <button
            type="button"
            title={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
            aria-label={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
            onClick={openMessageOrDraftTab}
            className="flex h-6 w-6 items-center justify-center rounded-full hover:bg-hover hover:text-primary cursor-pointer transition-colors"
          >
            <ExternalLink size={13} />
          </button>
          <button
            type="button"
            title={t('common.more')}
            aria-label={t('chat.moreMessageActions')}
            onClick={openActionsMenu}
            className="flex h-6 w-6 items-center justify-center rounded-full hover:bg-hover hover:text-primary cursor-pointer transition-colors"
          >
            <MoreHorizontal size={14} />
          </button>
        </div>

        {/* Header: sender + optional meta toggle on the left, timestamp on the right */}
        <div className="relative flex items-center justify-between gap-2 mb-1.5">
          <div className="relative flex items-center gap-1 min-w-0">
            {!outgoing ? (
              <span className="text-[0.78125rem] font-bold text-accent select-none truncate tracking-wide">
                {message.from_name || message.from_addr}
              </span>
            ) : (
              recipientSummary && (
                <span
                  title={[toRaw, ccRaw].filter(Boolean).join(', ')}
                  className="text-[0.6875rem] font-normal text-secondary/70 select-none truncate"
                >
                  {t('chat.toRecipients', { recipients: recipientSummary })}
                </span>
              )
            )}
            {/* A feed item has no recipients, so the details panel could only
                repeat the feed name the header already shows. */}
            {!isRSS && (
              <button
                type="button"
                onClick={() => setMetaOpen((open) => !open)}
                title={metaOpen ? t('chat.hideDetails') : t('chat.showDetails')}
                className="flex items-center justify-center w-4 h-4 rounded text-secondary hover:text-primary hover:bg-black/[0.05] dark:hover:bg-white/[0.08] cursor-pointer transition-colors"
              >
                <ChevronDown size={12} className={`transition-transform ${metaOpen ? 'rotate-180' : ''}`} />
              </button>
            )}
          </div>
          <div className="flex items-center gap-1 text-[0.65625rem] text-secondary/80 select-none shrink-0">
            {isDraft && (
              <span className="rounded-full border border-accent/35 bg-accent/10 px-1.5 py-0.5 text-[0.59375rem] font-bold uppercase tracking-wide text-accent">
                {t('chat.draft')}
              </span>
            )}
            {message.starred && <Star size={11} className="fill-amber-500 text-amber-500" />}
            <span title={formatFullTimestamp(message.date)}>
              {formatMessageStamp(message.date, view.showOriginalDate)}
            </span>
            {outgoing &&
              !isDraft &&
              (message.send_status === 'sending' ? (
                <Loader2 size={12} className="text-secondary/70 animate-spin" />
              ) : message.send_status === 'failed' ? (
                <button
                  type="button"
                  title={t('chat.failedToSendRetry')}
                  onClick={() => void retrySend(message.id)}
                  className="flex items-center gap-0.5 text-red-500 hover:text-red-600 cursor-pointer"
                >
                  <AlertCircle size={12} />
                  <span className="text-[0.625rem] font-semibold">{t('chat.retry')}</span>
                </button>
              ) : (
                <Check size={12} className="text-accent opacity-90" />
              ))}
          </div>
          {/* Anchored to the bubble edge, not the chevron: an outgoing bubble
              sits at the right of the pane, so opening rightwards pushed the
              panel past the viewport and gave the message list a horizontal
              scrollbar. Only an html bubble clamps to its own width: it is a
              fixed 70% of the pane, while a plain-text bubble shrinks to its
              content and would squeeze the addresses down to a few characters
              per line. */}
          {metaOpen && (
            <>
              <div className="fixed inset-0 z-40" onClick={() => setMetaOpen(false)} />
              <div
                className={`absolute top-full mt-1 z-50 w-[460px] max-w-[calc(100vw-48px)] max-h-[260px] overflow-y-auto space-y-2 rounded-lg border border-border bg-chats p-3 shadow-xl text-secondary select-text ${
                  outgoing ? (useHtmlBody ? 'right-0 max-w-full' : 'right-0') : 'left-0'
                }`}
              >
                <AddressRow label={t('composer.fields.from')} rawList={fromRaw} />
                {toRaw && <AddressRow label={t('composer.fields.to')} rawList={toRaw} />}
                {ccRaw && <AddressRow label={t('composer.fields.cc')} rawList={ccRaw} />}
                {bccRaw && <AddressRow label={t('composer.fields.bcc')} rawList={bccRaw} />}
                {replyToDiffers && <AddressRow label={t('chat.replyTo')} rawList={replyToRaw!} />}
              </div>
            </>
          )}
        </div>

        <MessageContent message={message} view={view} galleryOffset={galleryOffset} onLinkHover={onLinkHover} />
      </div>
    </div>
  )
}
