import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { RefObject } from 'react'
import type { CSSProperties } from 'react'
import { Loader2 } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { loadMoreMessages } from '../../states/mail'
import { settings$ } from '../../states/settings'
import type { Message } from '../../types'
import { LinkHoverPreview } from './LinkHoverPreview'
import { MessageBubble } from './MessageBubble'
import { MessageRow } from './MessageRow'
import { formatDateDivider } from './messageHelpers'
import type { MessageContextMenuState } from './MessageContextMenu'

const AUTO_LOAD_EARLIER_THRESHOLD_PX = 400

function hasSelectedText(): boolean {
  return !!window.getSelection()?.toString().trim()
}

// The scrollable conversation body: the "load earlier" affordance, date dividers
// and one MessageBubble per message. Scroll positioning lives in the parent's
// useConversationScroll hook, which owns the refs wired up here.
export function ConversationMessageList({
  messages,
  showThreadLoading,
  showThreadError,
  onRetryThreadLoad,
  messagesCursor,
  messagesLoadingMore,
  activeThreadId,
  searchMatches,
  activeSearchId,
  jumpMessageId,
  galleryOffsets,
  scrollRef,
  messagesWrapperRef,
  bottomAnchorRef,
  wallpaperClassName,
  wallpaperStyle,
  onScroll,
  onSetScrollTop,
  onScrollMessageToTop,
  onOpenContextMenu,
}: {
  messages: Message[]
  showThreadLoading: boolean
  showThreadError: boolean
  onRetryThreadLoad: () => void
  messagesCursor: string
  messagesLoadingMore: boolean
  activeThreadId: string
  searchMatches: string[]
  activeSearchId: string
  /** Message briefly highlighted by a direct jump. Its expansion is persisted
   *  separately so it stays open after the highlight expires. */
  jumpMessageId: string
  galleryOffsets: Map<string, number>
  scrollRef: RefObject<HTMLDivElement | null>
  messagesWrapperRef: RefObject<HTMLDivElement | null>
  bottomAnchorRef: RefObject<HTMLDivElement | null>
  wallpaperClassName: string
  wallpaperStyle?: CSSProperties
  onScroll: () => void
  /** Repositions the container through useConversationScroll's bookkeeping, so
   *  the move is not mistaken for the reader scrolling. */
  onSetScrollTop: (scrollTop: number) => void
  /** Brings a message's header to the top of the viewport, pinned while its
   *  body grows. Used when the reader expands a collapsed message. */
  onScrollMessageToTop: (messageId: string) => void
  onOpenContextMenu: (state: MessageContextMenuState) => void
}) {
  const { t } = useTranslation()
  const traditional = useValue(settings$.conversationLayout) === 'traditional'
  const [hoveredLink, setHoveredLink] = useState<string | null>(null)
  // Traditional layout only: message ids the user has explicitly expanded or
  // collapsed, overriding the default below. Cleared when the thread changes.
  const [expandOverrides, setExpandOverrides] = useState<Record<string, boolean>>({})
  const autoLoadInFlightRef = useRef(false)
  const activeThreadIdRef = useRef(activeThreadId)
  activeThreadIdRef.current = activeThreadId

  // Which messages were unread when they first rendered. Scroll-driven read
  // marking clears the flag while the user reads, so a default derived from the
  // live one would collapse an open message out from under them — and reaching
  // the bottom, which marks the whole thread read, would collapse every one of
  // them at once.
  const unreadOnArrivalRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    setExpandOverrides({})
    unreadOnArrivalRef.current = new Set()
  }, [activeThreadId])

  useLayoutEffect(() => {
    if (!jumpMessageId) return
    setExpandOverrides((current) => (current[jumpMessageId] === true ? current : { ...current, [jumpMessageId]: true }))
  }, [jumpMessageId])

  for (const message of messages) {
    if (message.unread) unreadOnArrivalRef.current.add(message.id)
  }

  // Mail-client default: the newest message is open, along with anything unread
  // or matched by the in-thread search; everything older collapses to a summary
  // line until the user clicks it.
  const searchMatchSet = useMemo(() => new Set(searchMatches), [searchMatches])
  const lastMessageId = messages.length > 0 ? messages[messages.length - 1].id : ''
  const isExpanded = (message: Message) => {
    // The match the user navigated to always shows its body: landing on a
    // collapsed summary would hide the very text that was searched for.
    if (message.id === activeSearchId) return true
    const override = expandOverrides[message.id]
    if (override !== undefined) return override
    return message.id === lastMessageId || unreadOnArrivalRef.current.has(message.id) || searchMatchSet.has(message.id)
  }
  const setExpanded = (messageId: string, expanded: boolean) => {
    setExpandOverrides((current) => ({ ...current, [messageId]: expanded }))
    // Expanding is a "let me read this" click, so bring the message to the top
    // rather than opening its body wherever it happens to sit. Collapsing is
    // "I'm done" and leaves the view alone.
    if (expanded) onScrollMessageToTop(messageId)
  }

  const loadEarlier = useCallback(() => {
    if (!messagesCursor || messagesLoadingMore || autoLoadInFlightRef.current) return
    const container = scrollRef.current
    const prevHeight = container?.scrollHeight ?? 0
    const prevTop = container?.scrollTop ?? 0
    const loadingThreadId = activeThreadId
    autoLoadInFlightRef.current = true

    const restoreScrollPosition = () => {
      requestAnimationFrame(() => {
        const el = scrollRef.current
        if (el && activeThreadIdRef.current === loadingThreadId) {
          onSetScrollTop(prevTop + (el.scrollHeight - prevHeight))
        }
        autoLoadInFlightRef.current = false
      })
    }
    void loadMoreMessages(activeThreadId).then(restoreScrollPosition, restoreScrollPosition)
  }, [activeThreadId, messagesCursor, messagesLoadingMore, scrollRef, onSetScrollTop])

  const handleScroll = useCallback(() => {
    onScroll()
    const container = scrollRef.current
    if (container && container.scrollTop <= AUTO_LOAD_EARLIER_THRESHOLD_PX) loadEarlier()
  }, [loadEarlier, onScroll, scrollRef])

  // Also fill a short viewport automatically: when ten messages do not create
  // enough overflow there may be no scroll event to trigger the threshold.
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const container = scrollRef.current
      if (container && container.scrollTop <= AUTO_LOAD_EARLIER_THRESHOLD_PX) loadEarlier()
    })
    return () => cancelAnimationFrame(frame)
  }, [activeThreadId, loadEarlier, messages.length, scrollRef])

  return (
    <div
      style={wallpaperStyle}
      onMouseLeave={() => setHoveredLink(null)}
      className={`flex-1 flex flex-col min-h-0 relative ${wallpaperClassName}`}
    >
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="message-scroll flex-1 overflow-y-auto px-4 py-6 space-y-4 z-10 relative"
      >
        {showThreadLoading && (
          <div className="flex h-full items-center justify-center">
            <Loader2 size={28} className="animate-spin text-secondary/70" />
          </div>
        )}
        {showThreadError && (
          <div className="flex h-full flex-col items-center justify-center gap-3">
            <p className="text-sm text-secondary">{t('chat.threadLoadFailed')}</p>
            <button
              type="button"
              onClick={onRetryThreadLoad}
              className="rounded-full bg-active border border-border/30 px-4 py-1 text-xs font-medium text-secondary hover:bg-active cursor-pointer"
            >
              {t('chat.retry')}
            </button>
          </div>
        )}
        {!showThreadLoading && messagesCursor && (
          <div className="flex justify-center pb-2">
            <button
              type="button"
              disabled={messagesLoadingMore}
              onClick={loadEarlier}
              className="rounded-full bg-active border border-border/30 px-4 py-1 text-xs font-medium text-secondary hover:bg-active disabled:opacity-50 cursor-pointer"
            >
              {messagesLoadingMore ? 'Loading…' : 'Load earlier messages'}
            </button>
          </div>
        )}
        <div ref={messagesWrapperRef} className={traditional ? 'space-y-2' : 'space-y-4'}>
          {!showThreadLoading &&
            messages.map((message, index) => {
              const label = formatDateDivider(message.date)
              const previousLabel = index > 0 ? formatDateDivider(messages[index - 1].date) : ''
              const isSearchMatch = searchMatches.includes(message.id)
              const expanded = isExpanded(message)
              return (
                <div
                  key={message.id}
                  data-message-id={message.id}
                  data-unread={message.unread ? 'true' : undefined}
                  data-search-match-id={isSearchMatch ? message.id : undefined}
                  onContextMenu={(event) => {
                    if (hasSelectedText()) return
                    event.preventDefault()
                    let linkUrl = (event.nativeEvent as any)?.meronLinkUrl || (event as any)?.meronLinkUrl
                    if (!linkUrl) {
                      const target = event.target as Element | null
                      const anchor =
                        target && typeof target.closest === 'function'
                          ? (target.closest('a[href]') as HTMLAnchorElement | null)
                          : null
                      if (anchor) {
                        const rawHref = anchor.getAttribute('href') ?? ''
                        if (rawHref && !rawHref.startsWith('#')) {
                          try {
                            const url = new URL(rawHref, anchor.ownerDocument.baseURI)
                            linkUrl = url.href
                          } catch {
                            linkUrl = rawHref
                          }
                        }
                      }
                    }
                    onOpenContextMenu({ x: event.clientX, y: event.clientY, message, linkUrl })
                  }}
                  className={`rounded-2xl transition-shadow ${traditional ? 'space-y-2' : 'space-y-4'} ${
                    activeSearchId === message.id || jumpMessageId === message.id
                      ? 'ring-2 ring-amber-300/80 ring-offset-2 ring-offset-transparent'
                      : ''
                  }`}
                >
                  {/* The traditional layout carries a date on every message
                      header, so it needs no dividers. */}
                  {!traditional && label && label !== previousLabel && (
                    <div className="mx-auto w-max select-none rounded-full bg-active border border-border/30 px-3 py-1.2 text-center text-[0.6875rem] font-semibold uppercase tracking-wider text-secondary">
                      {label}
                    </div>
                  )}
                  {traditional ? (
                    <MessageRow
                      message={message}
                      galleryOffset={galleryOffsets.get(message.id) ?? 0}
                      expanded={expanded}
                      onToggleExpanded={() => setExpanded(message.id, !expanded)}
                      onOpenContextMenu={onOpenContextMenu}
                      onLinkHover={setHoveredLink}
                    />
                  ) : (
                    <MessageBubble
                      message={message}
                      galleryOffset={galleryOffsets.get(message.id) ?? 0}
                      onOpenContextMenu={onOpenContextMenu}
                      onLinkHover={setHoveredLink}
                    />
                  )}
                </div>
              )
            })}
        </div>
        <div ref={bottomAnchorRef} className="message-scroll-anchor h-px" />
      </div>
      <LinkHoverPreview url={hoveredLink} />
    </div>
  )
}
