import { useEffect, useRef, useState } from 'react'
import type { MouseEvent } from 'react'
import { useTranslation } from '../../lib/i18n'
import { AlertCircle, ChevronDown, ExternalLink, Loader2, MoreHorizontal, Paperclip, Star } from 'lucide-react'

import { openDraftCompose, openMessageTab } from '../../states/compose'
import { retrySend } from '../../states/quickReplySend'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import { targetWithin, useDismissOnOutside } from '../menu/useDismissOnOutside'
import { Avatar } from '../avatar/Avatar'
import { AddressRow } from './AddressList'
import { BlockedRemoteButton } from './BlockedRemoteButton'
import { CryptoBadge } from '../crypto/CryptoBadge'
import { FrostedBand } from './FrostedBand'
import { MessageContent } from './MessageContent'
import { formatFullTimestamp, formatMessageStamp, normalizeBodyText } from './messageHelpers'
import { useMessageView } from './useMessageView'

const COLLAPSED_PREVIEW_CHARS = 200

function collapsedPreview(message: Message): string {
  const raw = message.preview?.trim() || normalizeBodyText(message.body).replace(/\s+/g, ' ').trim()
  return raw.slice(0, COLLAPSED_PREVIEW_CHARS)
}

/**
 * Marks an expanded message's header with `data-stuck` while it is pinned over
 * its body, which CSS alone can't tell from resting at the top of its card. The
 * attribute is set on the element directly: scrolling a long message past is
 * no reason to render it again.
 *
 * What is watched is not the header but a mark that stays where the header
 * rests, raised by the gap a pinned header keeps from the scroller's edge: the
 * header is pinned while that mark is above the edge. The gap is the
 * mark's own CSS, so it scales with the UI text size along with the header's
 * offset. Everything below the edge counts as in view, however far down, so
 * the mark crosses one line only and a jump of a page or more — from under the
 * viewport straight to pinned — can't pass unseen.
 */
function usePinnedHeader(expanded: boolean) {
  const headerRef = useRef<HTMLDivElement | null>(null)
  const restRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    const header = headerRef.current
    const rest = restRef.current
    const scroller = header?.closest('.message-scroll')
    if (!header || !rest || !scroller || typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver(
      ([entry]) => {
        const stuck = !entry.isIntersecting
        // A pinned header shrinks to one row. Its place in the card keeps the
        // height it had, or the body would jump up by the difference as the
        // header pinned and back down as it let go.
        if (!stuck) header.style.removeProperty('height')
        else if (!header.hasAttribute('data-stuck')) header.style.height = `${header.offsetHeight}px`
        header.toggleAttribute('data-stuck', stuck)
      },
      { root: scroller, rootMargin: '0px 0px 10000000px 0px' },
    )
    observer.observe(rest)
    return () => {
      observer.disconnect()
      header.style.removeProperty('height')
      header.removeAttribute('data-stuck')
    }
  }, [expanded])
  return { headerRef, restRef }
}

/**
 * One message in the traditional conversation layout: a full-width card that
 * collapses to a single summary line, the way classic mail clients stack a
 * thread. The chat layout's MessageBubble is the alternative (see
 * settings$.conversationLayout); both share MessageContent for the body.
 */
export function MessageRow({
  message,
  galleryOffset,
  expanded,
  onToggleExpanded,
}: {
  message: Message
  galleryOffset: number
  expanded: boolean
  onToggleExpanded: () => void
}) {
  const { t } = useTranslation()
  const [metaOpen, setMetaOpen] = useState(false)
  // The overlay below is stacked with the list, so it lies under the composer
  // floating over it (MessagePane); a click that the overlay does not catch
  // has to close the panel from here.
  useDismissOnOutside(
    metaOpen,
    (target) => targetWithin(target, '.message-scroll'),
    () => setMetaOpen(false),
  )
  const { headerRef: pinnedRef, restRef: pinnedRestRef } = usePinnedHeader(expanded)
  const view = useMessageView(message)
  const {
    outgoing,
    isDraft,
    isRSS,
    recipientSummary,
    allRecipientSummary,
    fromRaw,
    toRaw,
    ccRaw,
    bccRaw,
    replyToRaw,
    replyToDiffers,
  } = view

  const senderName = message.from_name || message.from_addr
  const stamp = formatMessageStamp(message.date, view.showOriginalDate)
  const fullStamp = formatFullTimestamp(message.date)

  const openActionsMenu = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation()
    const rect = event.currentTarget.getBoundingClientRect()
    thread$.messageContextMenu.set({ x: rect.right, y: rect.bottom + 4, message, hideOpenInNewTab: true })
  }
  const openMessageOrDraftTab = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation()
    if (isDraft) {
      void openDraftCompose(message)
      return
    }
    openMessageTab(message)
  }

  // The whole header collapses the message: a mail row is mostly blank space,
  // and having only the name line react reads as broken. Its own controls stop
  // their clicks from getting here; the details panel hangs inside the header,
  // so it is excluded by hand — as is the click that ends a text selection.
  const collapseFromHeader = (event: MouseEvent<HTMLDivElement>) => {
    const target = event.target as Element | null
    if (target?.closest?.('[data-header-panel]')) return
    if (window.getSelection()?.toString().trim()) return
    onToggleExpanded()
  }

  // Only in-flight and failed sends earn a marker here: unlike the chat layout,
  // a full-width mail row has no need for a "delivered" tick on every message.
  const statusIcon =
    outgoing && !isDraft ? (
      message.send_status === 'sending' ? (
        <Loader2 size={12} className="text-secondary/70 animate-spin" />
      ) : message.send_status === 'failed' ? (
        <button
          type="button"
          title={t('chat.failedToSendRetry')}
          onClick={(event) => {
            event.stopPropagation()
            void retrySend(message.id)
          }}
          className="flex items-center gap-0.5 text-red-500 hover:text-red-600 cursor-pointer"
        >
          <AlertCircle size={12} />
          <span className="text-[0.625rem] font-semibold">{t('chat.retry')}</span>
        </button>
      ) : null
    ) : null

  const draftBadge = isDraft ? (
    <span className="rounded-full border border-accent/35 bg-accent/10 px-1.5 py-0.5 text-[0.59375rem] font-bold uppercase tracking-wide text-accent">
      {t('chat.draft')}
    </span>
  ) : null

  if (!expanded) {
    return (
      <div
        role="button"
        tabIndex={0}
        onClick={onToggleExpanded}
        onKeyDown={(event) => {
          // Only the row itself: Enter/Space on a control inside it (the
          // blocked-remote button) bubbles up here, and expanding the message
          // out from under the key that just activated one is not what the
          // reader asked for — the same guard the expanded header uses.
          if (event.target !== event.currentTarget) return
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            onToggleExpanded()
          }
        }}
        title={t('chat.expandMessage')}
        className="group/message-row flex w-full cursor-pointer items-center gap-2.5 rounded-xl border border-border/40 bg-chats px-3 py-2.5 text-left transition-colors hover:bg-hover"
      >
        <Avatar name={view.avatarName} email={view.avatarEmail} src={view.avatarSrc} size={26} className="shrink-0" />
        <span
          className={`shrink-0 max-w-[30%] truncate text-[0.8125rem] text-primary ${message.unread ? 'font-bold' : 'font-semibold'}`}
        >
          {outgoing && recipientSummary ? t('chat.toRecipients', { recipients: recipientSummary }) : senderName}
        </span>
        {/* Every item in a feed thread comes from the same feed, so the title is
            what tells the collapsed rows apart. */}
        {isRSS && message.subject ? (
          <span className="min-w-0 flex-1 truncate text-[0.78125rem] text-primary">{message.subject}</span>
        ) : (
          <span className="min-w-0 flex-1 truncate text-[0.78125rem] text-secondary/80">
            {collapsedPreview(message)}
          </span>
        )}
        <div className="flex shrink-0 items-center gap-1.5 text-[0.65625rem] text-secondary/80">
          {draftBadge}
          {message.has_attachments && <Paperclip size={12} />}
          {message.starred && <Star size={12} className="fill-amber-500 text-amber-500" />}
          <CryptoBadge
            crypto={message.crypto}
            accountId={message.account_id}
            senderName={message.from_name}
            senderAddress={message.from_addr}
          />
          <BlockedRemoteButton
            messageId={message.id}
            blocked={view.blockedRemote}
            hiddenRemoteCount={view.hiddenRemoteCount}
            senderAddress={view.outgoing ? '' : view.senderAddress}
            size={15}
          />
          <span title={fullStamp}>{stamp}</span>
        </div>
      </div>
    )
  }

  return (
    <div className="group/message-row relative w-full rounded-xl border border-border/40 bg-chats shadow-card">
      {/* The header sticks to the top of the scroller while a long message
          scrolls past, so the collapse toggle stays within reach. The card
          carries no padding of its own: the header has to be a flush child so
          it pins from the card's very top. The negative offset cancels the
          scroller's top padding — a sticky box pins to the scroller's content
          box, so top-0 would park the header a padding's width down the
          viewport — less the gap a pinned header keeps from the edge. At rest
          a hairline is all that parts it from the body, which is the same
          card; the shadow is for when it rides over the text. Pinned,
          it is a card floating on the conversation, drawn like the composer at
          the other end of it, and what shows above it is frosted. It is also one row there — the sender
          and the actions, without the recipients or the details they open —
          since it sits on top of the text being read. The wrapper keeps the
          resting header's height (usePinnedHeader), so only the card inside
          takes the pointer and the body under the rest stays in reach. The
          change is eased, so everything in it has to be something that can
          be: the row stays top-aligned and is centred by margins, and the
          recipients fold away rather than leave the layout. */}
      {/* Where the header rests, raised by that gap (usePinnedHeader). */}
      <div ref={pinnedRestRef} className="pointer-events-none absolute -top-1.5 left-0 h-px w-px" />
      <div ref={pinnedRef} className="group/pinned sticky -top-1.5 z-20 data-[stuck]:pointer-events-none">
        {/* Frosted from the scroller's edge down to the middle of the resting
            header's height, which the pinned one still covers. */}
        <FrostedBand className="inset-x-0 -top-1.5 hidden h-[calc(50%+0.375rem)] group-data-[stuck]/pinned:block" />
        <div
          role="button"
          tabIndex={0}
          onClick={collapseFromHeader}
          onKeyDown={(event) => {
            // Only the header itself: Enter and Space on a control inside it
            // (details, open in a tab, the actions menu) bubble up here, and
            // collapsing the message out from under the key that just activated
            // one of them is not what the reader asked for.
            if (event.target !== event.currentTarget) return
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault()
              onToggleExpanded()
            }
          }}
          title={t('chat.collapseMessage')}
          className="pointer-events-auto relative flex cursor-pointer items-start gap-2.5 rounded-t-xl border-b border-border/40 bg-chats px-4 py-2.5 ring-border/40 transition-[border-radius,padding,box-shadow] duration-150 group-data-[stuck]/pinned:rounded-xl group-data-[stuck]/pinned:border-b-transparent group-data-[stuck]/pinned:py-1.5 group-data-[stuck]/pinned:shadow-float group-data-[stuck]/pinned:ring-1"
        >
          <Avatar
            name={view.avatarName}
            email={view.avatarEmail}
            src={view.avatarSrc}
            size={32}
            // Important, to win over the size Avatar sets inline.
            className="mt-0.5 shrink-0 transition-[width,height,margin,font-size] duration-150 group-data-[stuck]/pinned:mt-0! group-data-[stuck]/pinned:size-6! group-data-[stuck]/pinned:text-[0.5rem]!"
          />
          <div className="min-w-0 flex-1 transition-[margin] duration-150 group-data-[stuck]/pinned:mt-0.5">
            <div className="flex items-baseline gap-1.5">
              <span className="truncate text-[0.84375rem] font-semibold text-primary">{senderName}</span>
              <span className="truncate text-[0.71875rem] text-secondary/80">{message.from_addr}</span>
            </div>
            {/* The details toggle trails the recipients, the way the chat bubble
              puts it right after the header line it expands. */}
            {/* A grid row that goes from its content's height to nothing; out of
                sight it is out of the tab order too. */}
            <div className="grid grid-rows-[1fr] transition-[grid-template-rows,opacity,visibility] duration-150 group-data-[stuck]/pinned:invisible group-data-[stuck]/pinned:grid-rows-[0fr] group-data-[stuck]/pinned:opacity-0">
              <div className="flex min-h-0 min-w-0 items-center gap-1 overflow-hidden">
                {allRecipientSummary && (
                  <span
                    className="truncate text-[0.71875rem] text-secondary/80"
                    title={[toRaw, ccRaw].filter(Boolean).join(', ')}
                  >
                    {t('chat.toRecipients', { recipients: allRecipientSummary })}
                  </span>
                )}
                {/* A feed item has no recipients, so the details panel could only
                  repeat the feed name the header already shows. */}
                {!isRSS && (
                  <button
                    type="button"
                    onClick={(event) => {
                      event.stopPropagation()
                      setMetaOpen((open) => !open)
                    }}
                    title={metaOpen ? t('chat.hideDetails') : t('chat.showDetails')}
                    className="flex h-4 w-4 shrink-0 items-center justify-center rounded text-secondary hover:bg-black/[0.05] hover:text-primary dark:hover:bg-white/[0.08] cursor-pointer transition-colors"
                  >
                    <ChevronDown size={12} className={`transition-transform ${metaOpen ? 'rotate-180' : ''}`} />
                  </button>
                )}
              </div>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5 text-[0.65625rem] text-secondary/80 transition-[margin] duration-150 group-data-[stuck]/pinned:mt-0.5">
            {draftBadge}
            {message.has_attachments && <Paperclip size={12} />}
            {message.starred && <Star size={12} className="fill-amber-500 text-amber-500" />}
            <CryptoBadge
              crypto={message.crypto}
              accountId={message.account_id}
              senderName={message.from_name}
              senderAddress={message.from_addr}
            />
            <BlockedRemoteButton
              messageId={message.id}
              blocked={view.blockedRemote}
              hiddenRemoteCount={view.hiddenRemoteCount}
              senderAddress={view.outgoing ? '' : view.senderAddress}
              size={15}
            />
            <span title={fullStamp}>{stamp}</span>
            {statusIcon}
            <button
              type="button"
              title={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
              aria-label={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
              onClick={openMessageOrDraftTab}
              className="flex h-5 w-5 items-center justify-center rounded text-secondary hover:bg-black/[0.05] hover:text-primary dark:hover:bg-white/[0.08] cursor-pointer transition-colors"
            >
              <ExternalLink size={12} />
            </button>
            <button
              type="button"
              title={t('common.more')}
              aria-label={t('chat.moreMessageActions')}
              onClick={openActionsMenu}
              className="flex h-5 w-5 items-center justify-center rounded text-secondary hover:bg-black/[0.05] hover:text-primary dark:hover:bg-white/[0.08] cursor-pointer transition-colors"
            >
              <MoreHorizontal size={13} />
            </button>
          </div>
          {metaOpen && (
            <>
              <div
                className="fixed inset-0 z-40"
                onClick={(event) => {
                  event.stopPropagation()
                  setMetaOpen(false)
                }}
              />
              <div
                data-header-panel
                className="absolute left-0 top-full z-50 mt-1 max-h-[260px] w-[460px] max-w-[calc(100vw-48px)] space-y-2 overflow-y-auto rounded-lg border border-border bg-chats p-3 text-secondary shadow-xl select-text"
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
      </div>

      <div className="px-4 pb-3 pt-2.5">
        <MessageContent message={message} view={view} galleryOffset={galleryOffset} fullHeight surface="row" />
      </div>
    </div>
  )
}
