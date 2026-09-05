import { useCallback, useEffect, useLayoutEffect, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { markMessagesRead } from '../../states/mail'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import {
  ANCHOR_GAP_PX,
  anchorScrollTop,
  collectManualUnreadIds,
  isAtBottom,
  isMessageRead,
  isUserScroll,
  OPEN_ANCHOR_WINDOW_MS,
  pinnedScrollTop,
  resolveOpenScroll,
  resolveResizeScrollTop,
  type ScrollAnchor,
  type ScrollMetrics,
} from './conversationScroll'

/** How long an assignment of ours waits for its scroll event before we stop
 *  expecting one. Long enough to outlast a frame, short enough that a reader
 *  scrolling right after is not mistaken for it. */
const OWN_SCROLL_WINDOW_MS = 150

/** The message at the top of the viewport and how far into it the view has
 *  scrolled — a position that survives bodies growing from placeholder size. */
function readScrollAnchor(container: HTMLElement): ScrollAnchor | null {
  const elements = Array.from(container.querySelectorAll<HTMLElement>('[data-message-id]'))
  let anchor: HTMLElement | null = null
  for (const element of elements) {
    if (element.offsetTop - container.offsetTop > container.scrollTop) break
    anchor = element
  }
  // Scrolled above the first message (the load-earlier row): anchor to it
  // anyway, with the negative offset that puts it back below the top edge.
  anchor ??= elements[0] ?? null
  if (!anchor?.dataset.messageId) return null
  return {
    messageId: anchor.dataset.messageId,
    offset: container.scrollTop - (anchor.offsetTop - container.offsetTop),
  }
}

/** A saved anchor plus whether the reader had it parked at the end of the
 *  thread, which decides whether returning restores the position or follows
 *  whatever arrived in the meantime. */
type SavedScrollAnchor = ScrollAnchor & { atBottom: boolean }

function readScrollMetrics(container: HTMLElement): ScrollMetrics {
  return {
    scrollTop: container.scrollTop,
    scrollHeight: container.scrollHeight,
    clientHeight: container.clientHeight,
  }
}

// Owns the conversation scroll container and all of its positioning behaviour:
// restoring scroll when returning to a thread, autoscrolling on new messages,
// jumping to the first unread on open, and marking rendered messages read as they
// scroll past. Returns the refs the message list wires up plus the scroll
// handler. `unreadKey` changes whenever any message's unread flag flips.
export function useConversationScroll(
  activeThreadId: string,
  messages: Message[],
  activeTab: string,
  unreadKey: string,
) {
  const scrollRef = useRef<HTMLDivElement | null>(null)
  const bottomAnchorRef = useRef<HTMLDivElement | null>(null)
  const messagesWrapperRef = useRef<HTMLDivElement | null>(null)
  const lastScrollHeightRef = useRef(0)
  // Content height as of the last position we know the reader's view was
  // measured against — their own scrolling, or our own anchoring. A message
  // arriving is judged against this rather than against the height it just
  // created, so following the thread does not depend on the new message being
  // shorter than the stick distance.
  const viewedScrollHeightRef = useRef(0)
  const markingMessageIdsRef = useRef(new Set<string>())
  const conversationAnchorRef = useRef(new Map<string, SavedScrollAnchor>())
  // Threads awaiting a scroll restore. A single slot would lose the first one
  // when the reader leaves A for B and comes back: switching to A overwrites
  // the pending thread with B, and A restores nothing.
  const pendingScrollRestoreRef = useRef(new Set<string>())
  // Thread we've already done the one-time open positioning for, and the message
  // count at the last positioning — used to tell "thread opened" / "new message
  // arrived" apart from "read state changed" so we don't yank the user's scroll.
  const positionedThreadRef = useRef('')
  const messageCountRef = useRef(0)
  // Message the last positioning landed on. While set, the ResizeObserver below
  // re-anchors to it (instead of snapping to the bottom) so bodies growing from
  // their placeholder height don't push it out of view. Thread-open and direct-
  // jump pins expire after settling; an explicitly expanded message stays pinned
  // until the reader scrolls because its frame and images can finish much later.
  const pinnedRef = useRef<(ScrollAnchor & { persistent: boolean }) | null>(null)
  const pinReleaseTimerRef = useRef(0)

  // Last position we assigned ourselves, so scroll events we caused can be told
  // apart from the reader's — see isUserScroll.
  const expectedScrollTopRef = useRef<number | null>(null)
  // Whether an assignment of ours is still waiting for its scroll event. The
  // position check above is not enough on its own: a container still settling
  // (a body growing behind the assignment) can report a position we never set,
  // which reads as the reader moving the view and drops the pin — after which
  // the next resize parks the thread at the bottom. A flag rather than a count
  // of outstanding assignments: the browser coalesces several assignments made
  // in one frame into a single scroll event, and a count would then carry the
  // surplus over onto the reader's own scrolling. At most the first event after
  // an assignment is taken as ours, and the timer drops even that when the
  // assignment fired no event at all (setting the position the container
  // already had).
  const ownScrollPendingRef = useRef(false)
  const ownScrollForgetTimerRef = useRef(0)

  const applyScrollTop = useCallback((container: HTMLElement, scrollTop: number) => {
    const previousScrollTop = container.scrollTop
    container.scrollTop = scrollTop
    viewedScrollHeightRef.current = container.scrollHeight
    // Read it back: the browser clamps to the scrollable range, and the clamped
    // value is what the scroll event will report.
    expectedScrollTopRef.current = container.scrollTop
    // Assigning the current (or effectively identical) position fires no event.
    // Do not repeatedly extend the ownership window while a pinned resize keeps
    // resolving to the same place, or the reader's first scroll can be swallowed.
    if (isUserScroll(container.scrollTop, previousScrollTop)) {
      ownScrollPendingRef.current = true
      window.clearTimeout(ownScrollForgetTimerRef.current)
      ownScrollForgetTimerRef.current = window.setTimeout(() => {
        ownScrollPendingRef.current = false
      }, OWN_SCROLL_WINDOW_MS)
    }
  }, [])

  // Same bookkeeping for the message list's own repositioning (holding the view
  // still while older history is prepended): without it that assignment reads as
  // the reader scrolling and drops the anchor.
  const setScrollTop = useCallback(
    (scrollTop: number) => {
      const container = scrollRef.current
      if (container) applyScrollTop(container, scrollTop)
    },
    [applyScrollTop],
  )

  // Browser scroll anchoring is set up to hold the bottom of the thread (see
  // .message-scroll-anchor), which is the opposite of what a pin wants: a body
  // growing above that anchor drags the view down, and the scroll event the
  // browser produces looks exactly like the reader moving the view, so the pin
  // is dropped and the next resize parks the thread at the bottom. While a pin
  // is in force our own re-anchoring is the only positioning allowed.
  const releasePin = useCallback(() => {
    pinnedRef.current = null
    const container = scrollRef.current
    if (container) container.style.overflowAnchor = ''
  }, [])

  const releasePinForUserScroll = useCallback(() => {
    if (pinnedRef.current) releasePin()
  }, [releasePin])

  // The settle window runs from the last time the anchor was applied, not from
  // the first: a body that is still growing keeps it alive. Otherwise the pin
  // can expire mid-expansion — and while the target is out of reach the view
  // sits clamped at the bottom, which the unpinned rule then reads as "the
  // reader is at the bottom" and keeps it there.
  const armPinRelease = useCallback(() => {
    window.clearTimeout(pinReleaseTimerRef.current)
    pinReleaseTimerRef.current = window.setTimeout(releasePin, OPEN_ANCHOR_WINDOW_MS)
  }, [releasePin])

  const pinMessage = useCallback(
    (messageId: string, offset = -ANCHOR_GAP_PX, persistent = false) => {
      pinnedRef.current = { messageId, offset, persistent }
      const container = scrollRef.current
      if (container) container.style.overflowAnchor = 'none'
      window.clearTimeout(pinReleaseTimerRef.current)
      if (!persistent) armPinRelease()
    },
    [armPinRelease],
  )

  useEffect(
    () => () => {
      window.clearTimeout(pinReleaseTimerRef.current)
      window.clearTimeout(ownScrollForgetTimerRef.current)
    },
    [],
  )

  /** Brings a message's header to the top of the viewport, pinned so a body
   *  still growing from its placeholder height doesn't push it back out of
   *  view. Used when the reader expands a collapsed message: expanding in place
   *  leaves the body wherever the click happened to be, when the point of the
   *  click is to read it from its beginning. */
  const scrollMessageToTop = useCallback(
    (messageId: string) => {
      const container = scrollRef.current
      if (!container) return
      const target = container.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(messageId)}"]`)
      if (!target) return
      // An explicitly expanded message remains authoritative until the reader
      // scrolls. HTML frames and remote images can change height well after the
      // normal thread-open settle window has elapsed.
      pinMessage(messageId, -ANCHOR_GAP_PX, true)
      applyScrollTop(container, anchorScrollTop(target.offsetTop, container.offsetTop))
    },
    [applyScrollTop, pinMessage],
  )

  const pendingScrollMessageId = useValue(thread$.pendingScrollMessageId)

  // Messages the reader turned unread by hand while the thread was open, and the
  // unread flags of the last render they were derived from. Declared as a layout
  // effect ahead of the positioning ones below so the set is filled before any
  // of them can mark the message read again in the same commit.
  const heldUnreadIdsRef = useRef(new Set<string>())
  const previousUnreadRef = useRef(new Map<string, boolean>())
  const trackedUnreadThreadRef = useRef('')

  useLayoutEffect(() => {
    if (activeThreadId !== trackedUnreadThreadRef.current) {
      trackedUnreadThreadRef.current = activeThreadId
      heldUnreadIdsRef.current.clear()
      previousUnreadRef.current.clear()
    }
    const previous = previousUnreadRef.current
    for (const id of collectManualUnreadIds(messages, previous)) {
      heldUnreadIdsRef.current.add(id)
    }
    for (const message of messages) {
      previous.set(message.id, !!message.unread)
    }
  }, [activeThreadId, messages, unreadKey])

  const saveConversationScroll = useCallback(
    (restoreOnReturn = false) => {
      const container = scrollRef.current
      if (!container || !activeThreadId) return
      const anchor = readScrollAnchor(container)
      if (anchor) {
        conversationAnchorRef.current.set(activeThreadId, {
          ...anchor,
          atBottom: isAtBottom(readScrollMetrics(container)),
        })
      }
      if (restoreOnReturn) {
        pendingScrollRestoreRef.current.add(activeThreadId)
      }
    },
    [activeThreadId],
  )

  const maybeMarkRead = useCallback(() => {
    const container = scrollRef.current
    if (!container || !activeThreadId) return
    const hasUnread = messages.some((message) => message.thread_id === activeThreadId && message.unread)
    if (!hasUnread) return

    const containerRect = container.getBoundingClientRect()
    const isVisible = (element: HTMLElement) => {
      const rect = element.getBoundingClientRect()
      return rect.top < containerRect.bottom && rect.bottom > containerRect.top
    }
    const isRead = (element: HTMLElement) => isMessageRead(element.getBoundingClientRect(), containerRect)

    // "Mark as unread" on a message the reader is looking at would otherwise be
    // undone by the very next scroll or body resize. Hold off until it leaves
    // the viewport, after which reading it again marks it read as usual.
    for (const id of heldUnreadIdsRef.current) {
      const element = container.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(id)}"]`)
      if (!element || !isVisible(element)) heldUnreadIdsRef.current.delete(id)
    }

    const visibleMessageIds = Array.from(container.querySelectorAll<HTMLElement>('[data-unread="true"]'))
      .filter(isRead)
      .map((element) => element.dataset.messageId)
      .filter((id): id is string => !!id && !markingMessageIdsRef.current.has(id) && !heldUnreadIdsRef.current.has(id))

    if (visibleMessageIds.length === 0) return
    for (const id of visibleMessageIds) {
      markingMessageIdsRef.current.add(id)
    }
    void markMessagesRead(activeThreadId, visibleMessageIds).catch((error) => {
      for (const id of visibleMessageIds) {
        markingMessageIdsRef.current.delete(id)
      }
      console.error('Failed to mark visible messages read:', error)
    })
  }, [activeThreadId, messages])

  const handleConversationScroll = useCallback(() => {
    const container = scrollRef.current
    const ours = ownScrollPendingRef.current
    ownScrollPendingRef.current = false
    // The reader moving the view — including by dragging the scrollbar, which
    // dispatches no mouse events here — outranks the settle-window anchor.
    if (
      container &&
      pinnedRef.current &&
      !pinnedRef.current.persistent &&
      !ours &&
      isUserScroll(container.scrollTop, expectedScrollTopRef.current)
    ) {
      releasePin()
    }
    if (container) viewedScrollHeightRef.current = container.scrollHeight
    saveConversationScroll()
    maybeMarkRead()
  }, [maybeMarkRead, releasePin, saveConversationScroll])

  useLayoutEffect(() => {
    return () => {
      if (activeTab === '') {
        saveConversationScroll(true)
      }
    }
  }, [activeTab, saveConversationScroll])

  useEffect(() => {
    markingMessageIdsRef.current.clear()
  }, [activeThreadId, unreadKey])

  // Attach before paint and before child HtmlFrame effects begin reporting their
  // measured heights. On a cold open, attaching in a passive effect can miss the
  // first placeholder-to-content resize and leave a fully read thread at the top
  // instead of keeping its initial newest-message anchor.
  useLayoutEffect(() => {
    const container = scrollRef.current
    const wrapper = messagesWrapperRef.current
    if (activeTab !== '' || !container || !wrapper || !activeThreadId) return

    lastScrollHeightRef.current = container.scrollHeight

    const observer = new ResizeObserver(() => {
      const pin = pinnedRef.current
      const pinned = pin
        ? container.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(pin.messageId)}"]`)
        : null
      const scrollTop = resolveResizeScrollTop({
        metrics: readScrollMetrics(container),
        previousScrollHeight: lastScrollHeightRef.current,
        containerOffsetTop: container.offsetTop,
        pinned: pinned && pin ? { offsetTop: pinned.offsetTop, offset: pin.offset } : null,
      })
      if (scrollTop !== null) {
        applyScrollTop(container, scrollTop)
      }
      if (pinned && !pin?.persistent) armPinRelease()
      lastScrollHeightRef.current = container.scrollHeight
      viewedScrollHeightRef.current = container.scrollHeight
      maybeMarkRead()
    })

    observer.observe(wrapper)
    return () => observer.disconnect()
  }, [activeTab, activeThreadId, messages, applyScrollTop, armPinRelease])

  useLayoutEffect(() => {
    const container = scrollRef.current
    if (activeTab !== '' || !container || !activeThreadId || messages.length === 0) return
    if (messages.some((message) => message.thread_id !== activeThreadId)) return

    // A direct jump (for example, from starred items or shared media): scroll
    // to the requested message and flash its ring.
    // Consumed exactly once; if the message isn't in the loaded page (older than
    // the first page), fall through to the normal open positioning.
    if (pendingScrollMessageId) {
      thread$.pendingScrollMessageId.set('')
      const target = container.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(pendingScrollMessageId)}"]`)
      if (target) {
        positionedThreadRef.current = activeThreadId
        messageCountRef.current = messages.length
        pendingScrollRestoreRef.current.delete(activeThreadId)
        pinMessage(pendingScrollMessageId)
        applyScrollTop(container, anchorScrollTop(target.offsetTop, container.offsetTop))
        thread$.flashMessageId.set(pendingScrollMessageId)
        window.setTimeout(() => {
          if (thread$.flashMessageId.peek() === pendingScrollMessageId) {
            thread$.flashMessageId.set('')
          }
        }, OPEN_ANCHOR_WINDOW_MS)
        maybeMarkRead()
        return
      }
    }

    const isNewThread = positionedThreadRef.current !== activeThreadId
    const grew = messages.length > messageCountRef.current
    messageCountRef.current = messages.length
    let savedAnchor: SavedScrollAnchor | null = null
    let savedScrollTop: number | null = null
    if (pendingScrollRestoreRef.current.delete(activeThreadId)) {
      savedAnchor = conversationAnchorRef.current.get(activeThreadId) ?? null
      const saved = savedAnchor
        ? container.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(savedAnchor.messageId)}"]`)
        : null
      savedScrollTop =
        saved && savedAnchor ? pinnedScrollTop(saved.offsetTop, container.offsetTop, savedAnchor.offset) : null
    }

    const hasUnread = messages.some((message) => message.unread)
    const firstUnread = hasUnread ? container.querySelector<HTMLElement>('[data-unread="true"]') : null
    const messageElements = container.querySelectorAll<HTMLElement>('[data-message-id]')
    const lastMessage = messageElements.item(messageElements.length - 1)

    const plan = resolveOpenScroll({
      isNewThread,
      grew,
      savedScrollTop,
      savedAtBottom: savedAnchor?.atBottom ?? false,
      metrics: readScrollMetrics(container),
      previousScrollHeight: viewedScrollHeightRef.current,
      containerOffsetTop: container.offsetTop,
      hasUnread,
      firstUnreadOffsetTop: firstUnread ? firstUnread.offsetTop : null,
      lastMessageOffsetTop: lastMessage ? lastMessage.offsetTop : null,
    })
    // Only a pass that actually positioned counts as done. Bailing out because
    // the unread message is not in the DOM yet must leave the thread open for
    // another try, or the view stays wherever the previous thread left it.
    if (isNewThread && plan.scrollTop !== null) {
      positionedThreadRef.current = activeThreadId
    }
    if (plan.pin === 'unread' && firstUnread?.dataset.messageId) {
      pinMessage(firstUnread.dataset.messageId)
    } else if (plan.pin === 'last' && lastMessage?.dataset.messageId) {
      pinMessage(lastMessage.dataset.messageId)
    } else if (plan.pin === 'restore' && savedAnchor) {
      pinMessage(savedAnchor.messageId, savedAnchor.offset)
    } else if (plan.scrollTop !== null && pinnedRef.current) {
      // A plan that positions without an anchor is authoritative, so an older
      // pin has to go: the persistent one an expanded message leaves behind
      // outlives a trip to the full editor, and the first resize after the
      // reply lands would pull the view straight back off it.
      releasePin()
    }
    if (plan.scrollTop !== null) {
      applyScrollTop(container, plan.scrollTop)
    }
    // Only a positioning pass marks read. A bare read-state change must not:
    // it is what "mark as unread" produces, and marking there would undo it
    // before the reader sees anything happen.
    if (isNewThread || savedScrollTop !== null) {
      maybeMarkRead()
    }
  }, [
    activeTab,
    activeThreadId,
    messages.length,
    unreadKey,
    maybeMarkRead,
    pendingScrollMessageId,
    pinMessage,
    releasePin,
    applyScrollTop,
  ])

  return {
    scrollRef,
    bottomAnchorRef,
    messagesWrapperRef,
    handleConversationScroll,
    maybeMarkRead,
    setScrollTop,
    scrollMessageToTop,
    releasePinForUserScroll,
  }
}
