import { describe, expect, it } from 'bun:test'
import {
  ANCHOR_GAP_PX,
  anchorScrollTop,
  BOTTOM_STICK_PX,
  collectManualUnreadIds,
  isAtBottom,
  isMessageRead,
  isUserScroll,
  resolveOpenScroll,
  resolveResizeScrollTop,
  type ScrollMetrics,
} from './conversationScroll'

const CONTAINER_TOP = 100
const VIEWPORT = 800

function metrics(scrollTop: number, scrollHeight: number): ScrollMetrics {
  return { scrollTop, scrollHeight, clientHeight: VIEWPORT }
}

describe('anchorScrollTop', () => {
  it('puts the target just below the top of the container', () => {
    expect(anchorScrollTop(1000, CONTAINER_TOP)).toBe(1000 - CONTAINER_TOP - ANCHOR_GAP_PX)
  })

  it('never scrolls above the top', () => {
    expect(anchorScrollTop(CONTAINER_TOP, CONTAINER_TOP)).toBe(0)
    expect(anchorScrollTop(0, CONTAINER_TOP)).toBe(0)
  })
})

describe('isAtBottom', () => {
  it('counts a view within the stick distance of the end', () => {
    expect(isAtBottom(metrics(3000 - VIEWPORT - BOTTOM_STICK_PX, 3000))).toBe(true)
    expect(isAtBottom(metrics(3000 - VIEWPORT - BOTTOM_STICK_PX - 1, 3000))).toBe(false)
  })

  it('measures against the given height rather than the one content just grew to', () => {
    // The shared rule both a resize and an arriving message ask: the reader was
    // at the end of a 3000px thread, and the 600px that just landed under them
    // is not a reason to say they had scrolled away from it.
    expect(isAtBottom(metrics(3000 - VIEWPORT, 3600), 3000)).toBe(true)
    expect(isAtBottom(metrics(3000 - VIEWPORT, 3600))).toBe(false)
  })
})

describe('isUserScroll', () => {
  it('ignores the scroll event our own positioning fires', () => {
    expect(isUserScroll(1276, 1276)).toBe(false)
  })

  it('tolerates the sub-pixel position a browser reports back', () => {
    expect(isUserScroll(1275.5, 1276)).toBe(false)
  })

  it('recognises the reader moving the view', () => {
    // Dragging or clicking the scrollbar dispatches no mouse events to the
    // element, so this position check is what releases the anchor.
    expect(isUserScroll(2400, 1276)).toBe(true)
    expect(isUserScroll(1200, 1276)).toBe(true)
  })

  it("treats a scroll before any positioning as the reader's", () => {
    expect(isUserScroll(40, null)).toBe(true)
  })
})

describe('resolveOpenScroll on a fresh open', () => {
  const open = (over: Partial<Parameters<typeof resolveOpenScroll>[0]> = {}) =>
    resolveOpenScroll({
      isNewThread: true,
      grew: true,
      savedScrollTop: null,
      savedAtBottom: false,
      metrics: metrics(0, 2000),
      previousScrollHeight: 2000,
      containerOffsetTop: CONTAINER_TOP,
      hasUnread: false,
      firstUnreadOffsetTop: null,
      lastMessageOffsetTop: null,
      ...over,
    })

  it('lands on the first unread message', () => {
    expect(open({ hasUnread: true, firstUnreadOffsetTop: 1400 })).toEqual({
      scrollTop: 1400 - CONTAINER_TOP - ANCHOR_GAP_PX,
      pin: 'unread',
    })
  })

  it('pins the first unread even when it sits within a screen of the bottom', () => {
    // The regression: with the two unread mails near the end of the thread the
    // anchor is close to the bottom, so an unpinned view got snapped past them
    // by the first body-height resize.
    const plan = open({ hasUnread: true, firstUnreadOffsetTop: 1900, metrics: metrics(0, 2000) })
    expect(plan.pin).toBe('unread')
    expect(plan.scrollTop).toBe(1900 - CONTAINER_TOP - ANCHOR_GAP_PX)
  })

  it('lands on the top of the newest message when the thread is fully read', () => {
    expect(open({ metrics: metrics(0, 2000), lastMessageOffsetTop: 1600 })).toEqual({
      scrollTop: 1600 - CONTAINER_TOP - ANCHOR_GAP_PX,
      pin: 'last',
    })
  })

  it('falls back to the bottom when no message is rendered to anchor to', () => {
    expect(open({ metrics: metrics(0, 2000) })).toEqual({ scrollTop: 2000, pin: null })
  })

  it('leaves the view alone while unread messages are still unrendered', () => {
    expect(open({ hasUnread: true, firstUnreadOffsetTop: null })).toEqual({ scrollTop: null, pin: null })
  })
})

describe('resolveOpenScroll on an already-open thread', () => {
  const reopen = (over: Partial<Parameters<typeof resolveOpenScroll>[0]> = {}) =>
    resolveOpenScroll({
      isNewThread: false,
      grew: false,
      savedScrollTop: null,
      savedAtBottom: false,
      metrics: metrics(500, 3000),
      previousScrollHeight: 3000,
      containerOffsetTop: CONTAINER_TOP,
      hasUnread: false,
      firstUnreadOffsetTop: null,
      lastMessageOffsetTop: null,
      ...over,
    })

  it('does not move the view when messages merely re-render', () => {
    expect(reopen({ hasUnread: true, firstUnreadOffsetTop: 900 })).toEqual({ scrollTop: null, pin: null })
  })

  it('follows a newly arrived message for a reader at the bottom', () => {
    expect(reopen({ grew: true, metrics: metrics(3000 - VIEWPORT, 3000) })).toEqual({ scrollTop: 3000, pin: null })
  })

  it('leaves a reader who scrolled up where they are when a message arrives', () => {
    expect(reopen({ grew: true, metrics: metrics(3000 - VIEWPORT - BOTTOM_STICK_PX - 1, 3000) })).toEqual({
      scrollTop: null,
      pin: null,
    })
  })

  it('follows a message taller than the stick distance', () => {
    // The regression: a quick reply long enough to push the bottom more than
    // BOTTOM_STICK_PX away is measured against the height it just created, and
    // the reader who sent it was left looking at the date divider above it.
    expect(
      reopen({
        grew: true,
        previousScrollHeight: 3000,
        metrics: metrics(3000 - VIEWPORT, 3600),
      }),
    ).toEqual({ scrollTop: 3600, pin: null })
  })

  it('holds the restored position against bodies still settling', () => {
    // The saved position was measured against settled bodies. Restoring it
    // while they are placeholder-sized and then leaving it unpinned is how a
    // reader who left off at the head of a long post came back to its end.
    expect(reopen({ savedScrollTop: 640 }).pin).toBe('restore')
  })

  it('goes to unread the reader left above where they stopped', () => {
    // "Mark as unread" and walk away: coming back should show that message
    // rather than the position scrolled away from.
    expect(reopen({ savedScrollTop: 2000, hasUnread: true, firstUnreadOffsetTop: 900 })).toEqual({
      scrollTop: 900 - CONTAINER_TOP - ANCHOR_GAP_PX,
      pin: 'unread',
    })
  })

  it('keeps the saved position when the unread messages are further down', () => {
    expect(reopen({ savedScrollTop: 640, hasUnread: true, firstUnreadOffsetTop: 2400 })).toEqual({
      scrollTop: 640,
      pin: 'restore',
    })
  })

  it('restores the saved position when returning to a thread', () => {
    expect(reopen({ savedScrollTop: 640 })).toEqual({ scrollTop: 640, pin: 'restore' })
  })

  it('follows the thread when the reader left off at its end', () => {
    // The regression: a reply sent from the full editor lands while the
    // conversation is behind the compose tab, and restoring the position saved
    // on the way out parks the view at the old end, above the new message.
    expect(reopen({ savedScrollTop: 2200, savedAtBottom: true, metrics: metrics(2200, 3000) })).toEqual({
      scrollTop: 3000,
      pin: null,
    })
  })

  it('still shows an unread message left above a saved position at the end', () => {
    expect(reopen({ savedScrollTop: 2200, savedAtBottom: true, hasUnread: true, firstUnreadOffsetTop: 900 })).toEqual({
      scrollTop: 900 - CONTAINER_TOP - ANCHOR_GAP_PX,
      pin: 'unread',
    })
  })

  it('clamps a saved position that no longer exists to the bottom', () => {
    expect(reopen({ savedScrollTop: 9999, metrics: metrics(0, 3000) })).toEqual({
      scrollTop: 3000 - VIEWPORT,
      pin: 'restore',
    })
  })
})

describe('resolveResizeScrollTop', () => {
  const resize = (over: Partial<Parameters<typeof resolveResizeScrollTop>[0]> = {}) =>
    resolveResizeScrollTop({
      metrics: metrics(0, 4000),
      previousScrollHeight: 4000,
      containerOffsetTop: CONTAINER_TOP,
      pinned: null,
      ...over,
    })

  it('re-anchors to the pinned message as bodies grow', () => {
    expect(resize({ pinned: { offsetTop: 2600, offset: -ANCHOR_GAP_PX }, metrics: metrics(1200, 6000) })).toBe(
      2600 - CONTAINER_TOP - ANCHOR_GAP_PX,
    )
  })

  it('keeps the pin even when the view sat at the pre-resize bottom', () => {
    // A cold open positions the first unread near the bottom of the still
    // placeholder-sized content; without the pin this resize would snap to the
    // new bottom and hide the unread messages.
    expect(
      resize({
        pinned: { offsetTop: 1900, offset: -ANCHOR_GAP_PX },
        previousScrollHeight: 2000,
        metrics: metrics(2000 - VIEWPORT, 6000),
      }),
    ).toBe(1900 - CONTAINER_TOP - ANCHOR_GAP_PX)
  })

  it('sticks to the bottom for a reader who was already there', () => {
    expect(resize({ previousScrollHeight: 4000, metrics: metrics(4000 - VIEWPORT, 5000) })).toBe(5000)
  })

  it('does not yank back a reader who scrolled up', () => {
    expect(
      resize({ previousScrollHeight: 4000, metrics: metrics(4000 - VIEWPORT - BOTTOM_STICK_PX - 1, 5000) }),
    ).toBeNull()
  })

  it('measures the distance from the bottom before the resize, not after', () => {
    // Content that grew far below the fold must not count as "the reader is
    // near the bottom" just because they were before it appeared.
    expect(resize({ previousScrollHeight: 4000, metrics: metrics(4000 - VIEWPORT, 40000) })).toBe(40000)
    expect(resize({ previousScrollHeight: 40000, metrics: metrics(4000 - VIEWPORT, 40000) })).toBeNull()
  })
})

describe('collectManualUnreadIds', () => {
  const seen = (entries: Record<string, boolean>) => new Map(Object.entries(entries))

  it('catches a message the reader turned unread', () => {
    expect(collectManualUnreadIds([{ id: 'a', unread: true }], seen({ a: false }))).toEqual(['a'])
  })

  it('leaves messages the thread arrived with unread alone', () => {
    // Nothing seen before: opening a thread on its unread messages must still
    // mark them read as the reader goes through them.
    expect(collectManualUnreadIds([{ id: 'a', unread: true }], seen({}))).toEqual([])
    expect(collectManualUnreadIds([{ id: 'a', unread: true }], seen({ a: true }))).toEqual([])
  })

  it('ignores messages that were just marked read', () => {
    expect(collectManualUnreadIds([{ id: 'a', unread: false }], seen({ a: true }))).toEqual([])
  })
})

describe('isMessageRead', () => {
  const container = { top: 0, bottom: 800 }

  it('counts a message whose bottom came into view', () => {
    expect(isMessageRead({ top: 200, bottom: 600 }, container)).toBe(true)
  })

  it('counts a message the reader scrolled up through', () => {
    // Taller than the viewport: its bottom never shows while its top is still
    // on screen, so passing the top edge is what marks it read.
    expect(isMessageRead({ top: -400, bottom: 2000 }, container)).toBe(true)
  })

  it('does not count a message peeking in from the bottom', () => {
    expect(isMessageRead({ top: 795, bottom: 1600 }, container)).toBe(false)
  })

  it('does not count a long message the reader has only started', () => {
    expect(isMessageRead({ top: 24, bottom: 3000 }, container)).toBe(false)
  })

  it('counts a message that scrolled fully above the viewport', () => {
    expect(isMessageRead({ top: -900, bottom: -100 }, container)).toBe(true)
  })

  it('counts a message exactly filling the viewport', () => {
    expect(isMessageRead({ top: 0, bottom: 800 }, container)).toBe(true)
  })
})
