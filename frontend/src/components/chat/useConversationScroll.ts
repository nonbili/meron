import { useCallback, useEffect, useLayoutEffect, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { markMessagesRead } from '../../states/mail'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'

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
  const conversationScrollTopRef = useRef(new Map<string, number>())
  const pendingScrollRestoreThreadRef = useRef('')
  // Thread we've already done the one-time open positioning for, and the message
  // count at the last positioning — used to tell "thread opened" / "new message
  // arrived" apart from "read state changed" so we don't yank the user's scroll.
  const positionedThreadRef = useRef('')
  const messageCountRef = useRef(0)
  // Message a starred-list jump landed on. While set, the ResizeObserver below
  // re-anchors to it (instead of snapping to the bottom) so late-loading images
  // above don't push it out of view; released shortly after the jump.
  const pinnedMessageIdRef = useRef('')
  const pendingScrollMessageId = useValue(thread$.pendingScrollMessageId)

  const saveConversationScroll = useCallback(
    (restoreOnReturn = false) => {
      const container = scrollRef.current
      if (!container || !activeThreadId) return
      conversationScrollTopRef.current.set(activeThreadId, container.scrollTop)
      if (restoreOnReturn) {
        pendingScrollRestoreThreadRef.current = activeThreadId
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
    const visibleMessageIds = Array.from(container.querySelectorAll<HTMLElement>('[data-unread="true"]'))
      .filter((element) => {
        const rect = element.getBoundingClientRect()
        return rect.top < containerRect.bottom && rect.bottom > containerRect.top
      })
      .map((element) => element.dataset.messageId)
      .filter((id): id is string => !!id && !markingMessageIdsRef.current.has(id))

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

  useEffect(() => {
    const container = scrollRef.current
    const wrapper = messagesWrapperRef.current
    if (activeTab !== '' || !container || !wrapper || !activeThreadId) return

    lastScrollHeightRef.current = container.scrollHeight

    const observer = new ResizeObserver(() => {
      if (pinnedMessageIdRef.current) {
        const pinned = container.querySelector<HTMLElement>(
          `[data-message-id="${CSS.escape(pinnedMessageIdRef.current)}"]`,
        )
        if (pinned) {
          container.scrollTop = Math.max(0, pinned.offsetTop - container.offsetTop - 24)
          lastScrollHeightRef.current = container.scrollHeight
          maybeMarkRead()
          return
        }
      }
      // Keep the view pinned to the bottom only when it already was (content
      // grew under the fold, e.g. images loading after open). A reader scrolled
      // up — to star or reread something — must not be yanked back down.
      const previousDistanceFromBottom = lastScrollHeightRef.current - container.scrollTop - container.clientHeight

      if (previousDistanceFromBottom <= 160) {
        container.scrollTop = container.scrollHeight
      }
      lastScrollHeightRef.current = container.scrollHeight
      maybeMarkRead()
    })

    observer.observe(wrapper)
    return () => observer.disconnect()
  }, [activeTab, activeThreadId, messages])

  useLayoutEffect(() => {
    const container = scrollRef.current
    if (activeTab !== '' || !container || !activeThreadId || messages.length === 0) return
    if (messages.some((message) => message.thread_id !== activeThreadId)) return

    // A starred-list jump: scroll to the requested message and flash its ring.
    // Consumed exactly once; if the message isn't in the loaded page (older than
    // the first page), fall through to the normal open positioning.
    if (pendingScrollMessageId) {
      thread$.pendingScrollMessageId.set('')
      const target = container.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(pendingScrollMessageId)}"]`)
      if (target) {
        positionedThreadRef.current = activeThreadId
        messageCountRef.current = messages.length
        pendingScrollRestoreThreadRef.current = ''
        pinnedMessageIdRef.current = pendingScrollMessageId
        container.scrollTop = Math.max(0, target.offsetTop - container.offsetTop - 24)
        thread$.flashMessageId.set(pendingScrollMessageId)
        window.setTimeout(() => {
          pinnedMessageIdRef.current = ''
          if (thread$.flashMessageId.peek() === pendingScrollMessageId) {
            thread$.flashMessageId.set('')
          }
        }, 1800)
        maybeMarkRead()
        return
      }
    }

    const isNewThread = positionedThreadRef.current !== activeThreadId
    const grew = messages.length > messageCountRef.current
    messageCountRef.current = messages.length
    const shouldRestoreScroll = pendingScrollRestoreThreadRef.current === activeThreadId

    if (shouldRestoreScroll) {
      pendingScrollRestoreThreadRef.current = ''
      const savedTop = conversationScrollTopRef.current.get(activeThreadId)
      if (savedTop !== undefined) {
        container.scrollTop = Math.min(savedTop, Math.max(0, container.scrollHeight - container.clientHeight))
        maybeMarkRead()
        return
      }
    }

    if (!isNewThread) {
      if (!grew) return
      const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight
      if (distanceFromBottom > 160) return
      container.scrollTop = container.scrollHeight
      return
    }

    positionedThreadRef.current = activeThreadId
    if (messages.some((message) => message.unread)) {
      const firstUnread = container.querySelector<HTMLElement>('[data-unread="true"]')
      if (!firstUnread) {
        maybeMarkRead()
        return
      }
      container.scrollTop = Math.max(0, firstUnread.offsetTop - container.offsetTop - 24)
      maybeMarkRead()
    } else {
      container.scrollTop = container.scrollHeight
      maybeMarkRead()
    }
  }, [activeTab, activeThreadId, messages.length, unreadKey, maybeMarkRead, pendingScrollMessageId])

  return { scrollRef, bottomAnchorRef, messagesWrapperRef, handleConversationScroll, maybeMarkRead }
}
