import { useConversationView } from './conversationView'
import { useCallback, useContext, useEffect, useLayoutEffect, useMemo, useReducer, useRef } from 'react'
import type { CSSProperties } from 'react'
import { Loader2 } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { scrollbarHitWidth } from '../../lib/nativeScrollbars'
import { loadMoreMessages } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { thread$ } from '../../states/thread'
import { threadView } from '../../states/threadView'
import type { Message } from '../../types'
import { LinkHoverPreview } from './LinkHoverPreview'
import { MessageBubble } from './MessageBubble'
import { MessageRow } from './MessageRow'
import { formatDateDivider, mediaFilename, imageSource } from './messageHelpers'
import { MESSAGE_PAGE_EVENT } from './messagePaging'
import { ConversationScrollContext } from './useConversationScroll'

const AUTO_LOAD_EARLIER_THRESHOLD_PX = 400

function remoteContextImage(target: Element | null) {
  if (!target?.closest?.('[data-media-src]')) return undefined
  const img = target?.closest?.('img') as HTMLImageElement | null
  return img ? (imageSource(img.currentSrc || img.src) ?? undefined) : undefined
}

function hasSelectedText(): boolean {
  return !!window.getSelection()?.toString().trim()
}

// Subscribes on its own so hovering a link doesn't re-render the message list.
function HoveredLinkPreview() {
  return <LinkHoverPreview url={useValue(thread$.hoveredLink)} />
}

// The scrollable conversation body: the "load earlier" affordance, date dividers
// and one MessageBubble per message. Scroll positioning lives in the parent's
// useConversationScroll hook, which owns the refs wired up here and reaches
// this component through ConversationScrollContext.
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
  wallpaperClassName,
  wallpaperStyle,
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
  wallpaperClassName: string
  wallpaperStyle?: CSSProperties
}) {
  const { state: thread$, active: viewActive, scope: viewScope } = useConversationView()
  const { t } = useTranslation()
  const traditional = useValue(settings$.conversationLayout) === 'traditional'
  const {
    scrollRef,
    messagesWrapperRef,
    bottomAnchorRef,
    handleConversationScroll: onScroll,
    // Repositions the container through useConversationScroll's bookkeeping, so
    // the move is not mistaken for the reader scrolling.
    setScrollTop: onSetScrollTop,
    // Brings a message's header to the top of the viewport, pinned while its
    // body grows. Used when the reader expands a collapsed message.
    scrollMessageToTop: onScrollMessageToTop,
    // Holds the reader's place while every message expands or collapses.
    holdTopMessage: onHoldTopMessage,
    // Releases expansion anchoring before input that intentionally moves the view.
    releasePinForUserScroll: onUserScrollIntent,
  } = useContext(ConversationScrollContext)!
  // Traditional layout only: what the reader expanded or collapsed in this
  // thread, overriding the default below. Remembered per thread, so switching
  // to another tab or thread and back keeps it.
  const expansion = threadView(viewScope || activeThreadId)
  const expandOverrides = expansion.overrides
  const [, rerender] = useReducer((n: number) => n + 1, 0)
  const setExpandOverrides = useCallback(
    (threadId: string, update: (current: Record<string, boolean>) => Record<string, boolean>) => {
      const target = threadView(viewScope || threadId)
      const next = update(target.overrides)
      if (next === target.overrides) return
      target.overrides = next
      rerender()
    },
    [viewScope],
  )
  const pendingExpandedMessageIdRef = useRef('')
  const autoLoadInFlightRef = useRef(false)
  const viewActiveRef = useRef(viewActive)
  viewActiveRef.current = viewActive
  const activeThreadIdRef = useRef(activeThreadId)
  activeThreadIdRef.current = activeThreadId

  // Expand all / collapse all, from the thread's menu: the default below gives
  // way to it, and so does every message toggled before. Heard as it is set
  // rather than on the render it causes, which is the last moment the view can
  // still be read for the place to hold. Only this view's command state is
  // observed, so a hidden conversation keeps its expansion untouched.
  const expandAll = expansion.expandAll
  useEffect(
    () =>
      thread$.expandAll.onChange(({ value }) => {
        if (!value) return
        onHoldTopMessage(!value.expanded)
        pendingExpandedMessageIdRef.current = ''
        const target = threadView(viewScope || activeThreadIdRef.current)
        target.expandAll = { expanded: value.expanded }
        target.overrides = {}
        rerender()
      }),
    [onHoldTopMessage, thread$, viewScope],
  )

  useLayoutEffect(() => {
    if (!jumpMessageId) return
    setExpandOverrides(activeThreadIdRef.current, (current) =>
      current[jumpMessageId] === true ? current : { ...current, [jumpMessageId]: true },
    )
  }, [jumpMessageId, setExpandOverrides])

  useLayoutEffect(() => {
    const messageId = pendingExpandedMessageIdRef.current
    if (!messageId || expandOverrides[messageId] !== true) return
    pendingExpandedMessageIdRef.current = ''
    // Position only after React has replaced the collapsed summary with the
    // expanded row. Scrolling against the old, short layout can be clamped to
    // the bottom before the body and its asynchronously measured frame appear.
    onScrollMessageToTop(messageId)
  }, [expandOverrides, onScrollMessageToTop])

  // Which messages were unread when they first rendered. Scroll-driven read
  // marking clears the flag while the user reads, so a default derived from the
  // live one would collapse an open message out from under them — and reaching
  // the bottom, which marks the whole thread read, would collapse every one of
  // them at once. Messages still showing from the previous thread don't count.
  for (const message of messages) {
    if (message.unread && message.thread_id === activeThreadId) expansion.unreadOnArrival.add(message.id)
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
    // Collapsing all still leaves the newest open: a thread of nothing but
    // summary lines has nothing to read, and it is the one a reply answers.
    if (expandAll) return expandAll.expanded || message.id === lastMessageId
    return message.id === lastMessageId || expansion.unreadOnArrival.has(message.id) || searchMatchSet.has(message.id)
  }
  const expandAllAction =
    !traditional || showThreadLoading
      ? null
      : messages.some((message) => !isExpanded(message))
        ? 'expand'
        : // Collapsing all leaves the newest open, which is all a thread of one has.
          messages.length > 1
          ? 'collapse'
          : null
  useEffect(() => {
    thread$.expandAllAction.set(expandAllAction)
  }, [expandAllAction])
  useEffect(() => () => thread$.expandAllAction.set(null), [])
  const setExpanded = (messageId: string, expanded: boolean) => {
    pendingExpandedMessageIdRef.current = expanded ? messageId : ''
    setExpandOverrides(activeThreadId, (current) => ({ ...current, [messageId]: expanded }))
  }

  const loadEarlier = useCallback(() => {
    if (!viewActive || !messagesCursor || messagesLoadingMore || autoLoadInFlightRef.current) return
    const container = scrollRef.current
    const prevHeight = container?.scrollHeight ?? 0
    const prevTop = container?.scrollTop ?? 0
    const loadingThreadId = activeThreadId
    autoLoadInFlightRef.current = true

    const restoreScrollPosition = () => {
      requestAnimationFrame(() => {
        const el = scrollRef.current
        if (el && viewActiveRef.current && activeThreadIdRef.current === loadingThreadId) {
          onSetScrollTop(prevTop + (el.scrollHeight - prevHeight))
        }
        autoLoadInFlightRef.current = false
      })
    }
    void loadMoreMessages(activeThreadId).then(restoreScrollPosition, restoreScrollPosition)
  }, [viewActive, activeThreadId, messagesCursor, messagesLoadingMore, scrollRef, onSetScrollTop])

  const handleScroll = useCallback(() => {
    onScroll()
    const container = scrollRef.current
    if (container && container.scrollTop <= AUTO_LOAD_EARLIER_THRESHOLD_PX) loadEarlier()
  }, [loadEarlier, onScroll, scrollRef])

  // Space / Shift+Space (and arrows from inside a message frame) scroll the view
  // from the global hotkeys, with focus anywhere, so the keydown capture below
  // never sees them.
  useEffect(() => {
    const container = scrollRef.current
    if (!container) return
    container.addEventListener(MESSAGE_PAGE_EVENT, onUserScrollIntent)
    return () => container.removeEventListener(MESSAGE_PAGE_EVENT, onUserScrollIntent)
  }, [onUserScrollIntent, scrollRef])

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
      onMouseLeave={() => thread$.hoveredLink.set(null)}
      // A right-click that lands on no message is for the thread: its actions
      // menu, where the pointer is. A message answers its own (below), and
      // leaves the browser's menu alone over selected text.
      onContextMenu={(event) => {
        if ((event.target as Element | null)?.closest?.('[data-message-id]')) return
        event.preventDefault()
        thread$.conversationMenu.set({ x: event.clientX, y: event.clientY })
      }}
      className={`flex-1 flex flex-col min-h-0 relative ${wallpaperClassName}`}
    >
      <div
        ref={scrollRef}
        data-message-scroller
        // Focusable, so a click into the conversation takes focus off the
        // thread list and the arrow keys scroll here instead of stepping it.
        tabIndex={-1}
        onScroll={handleScroll}
        onWheel={onUserScrollIntent}
        onTouchMove={onUserScrollIntent}
        onKeyDownCapture={(event) => {
          if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) {
            onUserScrollIntent()
          }
        }}
        onPointerDown={(event) => {
          const container = event.currentTarget
          const scrollbarWidth = scrollbarHitWidth(
            container,
            document.documentElement.classList.contains('native-scrollbars'),
          )
          if (scrollbarWidth > 0 && event.clientX >= container.getBoundingClientRect().right - scrollbarWidth) {
            onUserScrollIntent()
          }
        }}
        // The padding round the messages lines them up with the composer's
        // box. MessageRow's pinned header offsets itself against the top
        // padding; keep the two in step. The end pads past the
        // composer floating over the list (MessagePane), so the newest message
        // clears it — by the same gap, less the pt-1 the composer's footer
        // already puts above its box; the rest of the list runs on behind it.
        className="message-scroll flex-1 overflow-y-auto outline-none px-3 pt-3 pb-[calc(0.5rem+var(--me-composer-overlap,0px))] space-y-4 z-10 relative"
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
                    const target = event.target as Element | null
                    const imageEl =
                      target && typeof target.closest === 'function'
                        ? (target.closest('[data-media-key]') as HTMLElement | null)
                        : null
                    // A picture inside the email's frame, forwarded by HtmlFrame.
                    const frameImageKey: string | undefined = (event.nativeEvent as any)?.meronImageKey
                    const image = imageEl
                      ? { key: imageEl.dataset.mediaKey!, filename: imageEl.dataset.mediaFilename ?? '' }
                      : frameImageKey
                        ? { key: frameImageKey, filename: mediaFilename(frameImageKey, message.attachments) }
                        : ((event.nativeEvent as any)?.meronImage ?? remoteContextImage(target))
                    thread$.messageContextMenu.set({ x: event.clientX, y: event.clientY, message, linkUrl, image })
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
                    />
                  ) : (
                    <MessageBubble message={message} galleryOffset={galleryOffsets.get(message.id) ?? 0} />
                  )}
                </div>
              )
            })}
        </div>
        <div ref={bottomAnchorRef} className="message-scroll-anchor h-px" />
      </div>
      {/* Above the composer floating over the end of the list, not on it. */}
      <div className="pointer-events-none absolute inset-x-0 bottom-[var(--me-composer-overlap,0px)] z-30">
        <HoveredLinkPreview />
      </div>
    </div>
  )
}
