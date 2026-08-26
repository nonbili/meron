import { useCallback, useEffect, useLayoutEffect, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { markMessagesRead } from '../../states/mail'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import {
  ANCHOR_GAP_PX,
  anchorScrollTop,
  collectManualUnreadIds,
  isMessageRead,
  isUserScroll,
  OPEN_ANCHOR_WINDOW_MS,
  pinnedScrollTop,
  resolveOpenScroll,
  resolveResizeScrollTop,
  type ScrollAnchor,
  type ScrollMetrics,
} from './conversationScroll'

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
  const markingMessageIdsRef = useRef(new Set<string>())
  const conversationAnchorRef = useRef(new Map<string, ScrollAnchor>())
  // Threads awaiting a scroll restore. A single slot would lose the first one
  // when the reader leaves A for B and comes back: switching to A overwrites
  // the pending thread with B, and A restores nothing.
  const pendingScrollRestoreRef = useRef(new Set<string>())
  // Thread we've already done the one-time open positioning for, and the message
  // count at the last positioning — used to tell "thread opened" / "new message
  // arrived" apart from "read state changed" so we don't yank the user's scroll.
  const positionedThreadRef = useRef('')
  const messageCountRef = useRef(0)
  // Message the last positioning landed on — a starred-list jump, or the first
  // unread on open. While set, the ResizeObserver below re-anchors to it (instead
  // of snapping to the bottom) so bodies growing from their placeholder height
  // don't push it out of view; released shortly after the jump.
  const pinnedRef = useRef<ScrollAnchor | null>(null)
  const pinReleaseTimerRef = useRef(0)

  // Last position we assigned ourselves, so scroll events we caused can be told
  // apart from the reader's — see isUserScroll.
  const expectedScrollTopRef = useRef<number | null>(null)

  const applyScrollTop = useCallback((container: HTMLElement, scrollTop: number) => {
    container.scrollTop = scrollTop
    // Read it back: the browser clamps to the scrollable range, and the clamped
    // value is what the scroll event will report.
    expectedScrollTopRef.current = container.scrollTop
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

  // The settle window runs from the last time the anchor was applied, not from
  // the first: a body that is still growing keeps it alive. Otherwise the pin
  // can expire mid-expansion — and while the target is out of reach the view
  // sits clamped at the bottom, which the unpinned rule then reads as "the
  // reader is at the bottom" and keeps it there.
  const armPinRelease = useCallback(() => {
    window.clearTimeout(pinReleaseTimerRef.current)
    pinReleaseTimerRef.current = window.setTimeout(() => {
      pinnedRef.current = null
    }, OPEN_ANCHOR_WINDOW_MS)
  }, [])

  const pinMessage = useCallback(
    (messageId: string, offset = -ANCHOR_GAP_PX) => {
      pinnedRef.current = { messageId, offset }
      armPinRelease()
    },
    [armPinRelease],
  )

  useEffect(() => () => window.clearTimeout(pinReleaseTimerRef.current), [])

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
      if (anchor) conversationAnchorRef.current.set(activeThreadId, anchor)
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
    // The reader moving the view — including by dragging the scrollbar, which
    // dispatches no mouse events here — outranks the settle-window anchor.
    if (container && pinnedRef.current && isUserScroll(container.scrollTop, expectedScrollTopRef.current)) {
      pinnedRef.current = null
    }
    saveConversationScroll()
    maybeMarkRead()
  }, [maybeMarkRead, saveConversationScroll])

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
      if (pinned) armPinRelease()
      lastScrollHeightRef.current = container.scrollHeight
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
    let savedAnchor: ScrollAnchor | null = null
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
      metrics: readScrollMetrics(container),
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
    applyScrollTop,
  ])

  return { scrollRef, bottomAnchorRef, messagesWrapperRef, handleConversationScroll, maybeMarkRead, setScrollTop }
}
