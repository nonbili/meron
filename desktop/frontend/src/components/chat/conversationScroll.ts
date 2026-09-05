// The scroll-positioning arithmetic behind useConversationScroll, kept free of
// DOM lookups so it can be unit-tested. The hook measures the container and its
// message elements and feeds the numbers in; these functions decide where the
// view should land. Both conversation layouts (chat bubbles and the traditional
// rows) share the hook, so they share these rules.

/** Breathing room left above a message the view anchors to. */
export const ANCHOR_GAP_PX = 24

/** Below this distance from the bottom the view counts as "at the bottom", and
 *  content growing underneath keeps it there. */
export const BOTTOM_STICK_PX = 160

/** How long after positioning the view keeps re-anchoring to its target while
 *  asynchronously measured bodies settle. Mirrors THREAD_OPEN_ANCHOR_WINDOW_MS
 *  on mobile. */
export const OPEN_ANCHOR_WINDOW_MS = 1800

/** Slack for the sub-pixel scroll positions a browser can report back after a
 *  programmatic assignment. */
const SCROLL_MATCH_TOLERANCE_PX = 1

/**
 * Whether a scroll event came from the reader rather than from our own
 * positioning. Scrollbar drags and clicks dispatch no mouse events to the
 * element in Chromium, so the only signal that separates them from the
 * anchoring we do ourselves is the position: anything we did not just set is
 * the reader moving the view. `expectedScrollTop` is null before we have
 * positioned anything, when every scroll is the reader's.
 */
export function isUserScroll(scrollTop: number, expectedScrollTop: number | null): boolean {
  if (expectedScrollTop === null) return true
  return Math.abs(scrollTop - expectedScrollTop) > SCROLL_MATCH_TOLERANCE_PX
}

/**
 * Ids of messages that just went from read to unread — the reader picking "Mark
 * as unread" on a message that is very likely still on screen. Scroll-driven
 * read marking has to leave those alone until they scroll away, or the action
 * undoes itself. A message first seen already unread (no entry in
 * `previousUnread`) is not one of these: that is simply an unread message the
 * thread arrived with, and reading it marks it read as usual.
 */
export function collectManualUnreadIds(
  messages: readonly { id: string; unread?: boolean }[],
  previousUnread: ReadonlyMap<string, boolean>,
): string[] {
  return messages.filter((message) => message.unread && previousUnread.get(message.id) === false).map((m) => m.id)
}

export type VerticalBounds = {
  top: number
  bottom: number
}

/**
 * Whether a message counts as read from where the view sits. Two ways to have
 * read it: its bottom came into view, so all of it has been on screen, or its
 * top has passed above the container's edge, so the reader scrolled through it
 * — the case for a message taller than the viewport, which would otherwise
 * never show its bottom. Merely peeking in from below is not reading: that is
 * the next message waiting its turn.
 */
export function isMessageRead(message: VerticalBounds, container: VerticalBounds): boolean {
  return message.top < container.top || message.bottom <= container.bottom
}

export type ScrollMetrics = {
  scrollTop: number
  scrollHeight: number
  clientHeight: number
}

/**
 * Where to scroll to put a message back where it belongs. `offset` is the gap
 * between the container's top edge and the message's: negative leaves the
 * message below the edge (the anchor gap), positive means the reader had
 * scrolled that far into it.
 */
export function pinnedScrollTop(targetOffsetTop: number, containerOffsetTop: number, offset: number): number {
  return Math.max(0, targetOffsetTop - containerOffsetTop + offset)
}

/** Where to scroll so `targetOffsetTop` sits just below the container's top. */
export function anchorScrollTop(targetOffsetTop: number, containerOffsetTop: number): number {
  return pinnedScrollTop(targetOffsetTop, containerOffsetTop, -ANCHOR_GAP_PX)
}

/**
 * A scroll position expressed as content rather than pixels: the message at the
 * top of the viewport and how far into it the reader had scrolled. Message
 * bodies measure asynchronously, so a raw scrollTop saved before they settle
 * means something different by the time it is restored.
 */
export type ScrollAnchor = {
  messageId: string
  offset: number
}

function bottomScrollTop(metrics: ScrollMetrics): number {
  return Math.max(0, metrics.scrollHeight - metrics.clientHeight)
}

/**
 * Whether the view counts as parked at the end of the thread — the one rule
 * every "should the view follow the content" decision here asks. `scrollHeight`
 * overrides the measured one so a caller can ask "was the reader at the end"
 * against the height the position was measured against, rather than "is the new
 * content past the fold" against the height that content just created. Getting
 * that backwards is how a reply taller than the stick distance left the reader
 * looking at the message above it.
 */
export function isAtBottom(metrics: ScrollMetrics, scrollHeight = metrics.scrollHeight): boolean {
  return scrollHeight - metrics.scrollTop - metrics.clientHeight <= BOTTOM_STICK_PX
}

/**
 * Where a content resize should leave the view. `previousScrollHeight` is the
 * height measured before this resize, which is what isAtBottom is asked
 * against. Returns null to leave the scroll position alone.
 */
export function resolveResizeScrollTop({
  metrics,
  previousScrollHeight,
  containerOffsetTop,
  pinned,
}: {
  metrics: ScrollMetrics
  previousScrollHeight: number
  containerOffsetTop: number
  /** Measured position of the pinned message, or null when nothing is pinned. */
  pinned: { offsetTop: number; offset: number } | null
}): number | null {
  // A pinned target wins: bodies growing above it must not push it out of view,
  // and its own growth must not read as "the reader is at the bottom".
  if (pinned !== null) {
    return pinnedScrollTop(pinned.offsetTop, containerOffsetTop, pinned.offset)
  }
  // Keep the view pinned to the bottom only when it already was (content grew
  // under the fold, e.g. images loading after open). A reader scrolled up — to
  // star or reread something — must not be yanked back down.
  if (!isAtBottom(metrics, previousScrollHeight)) return null
  return metrics.scrollHeight
}

export type OpenScrollPlan = {
  /** Target scroll position, or null to leave the view where it is. */
  scrollTop: number | null
  /** Which anchor to hold against resizes for the settle window, if any: the
   *  first unread message, the newest one, or the saved anchor a restore
   *  landed on. */
  pin: 'unread' | 'last' | 'restore' | null
}

/**
 * Where the view belongs when a thread renders: a restored position when
 * returning to a thread, the first unread on a fresh open, the top of the
 * newest message when everything is read, and nothing at all when messages
 * merely re-render.
 */
export function resolveOpenScroll({
  isNewThread,
  grew,
  savedScrollTop,
  savedAtBottom,
  metrics,
  previousScrollHeight,
  containerOffsetTop,
  hasUnread,
  firstUnreadOffsetTop,
  lastMessageOffsetTop,
}: {
  isNewThread: boolean
  /** Whether the message count grew since the last positioning. */
  grew: boolean
  /** Position saved when leaving this thread, or null when not restoring. */
  savedScrollTop: number | null
  /** Whether that saved position was at the end of the thread. */
  savedAtBottom: boolean
  metrics: ScrollMetrics
  /** Height the container had when the reader's position was last known — i.e.
   *  before the message that just arrived was rendered into it. */
  previousScrollHeight: number
  containerOffsetTop: number
  hasUnread: boolean
  /** offsetTop of the first unread message, or null when none is rendered. */
  firstUnreadOffsetTop: number | null
  /** offsetTop of the newest message, or null when none is rendered. */
  lastMessageOffsetTop: number | null
}): OpenScrollPlan {
  const unreadAnchor = firstUnreadOffsetTop === null ? null : anchorScrollTop(firstUnreadOffsetTop, containerOffsetTop)

  if (savedScrollTop !== null) {
    const restored = Math.min(savedScrollTop, bottomScrollTop(metrics))
    // Unread above where the reader stopped means they left something behind —
    // most often by marking a message unread on purpose. Coming back should
    // show that message, not the position they scrolled away from. Unread below
    // is simply the thread continuing, so the saved position still wins.
    if (unreadAnchor !== null && unreadAnchor < restored) return { scrollTop: unreadAnchor, pin: 'unread' }
    // The reader left off at the end of the thread and it has grown since —
    // a reply sent from the full editor, or mail that arrived while they were
    // in another tab. The saved position is the old end, so restoring it would
    // leave the new message below the fold; follow the thread instead.
    if (savedAtBottom) return { scrollTop: metrics.scrollHeight, pin: null }
    // Pinned like any other target: the saved position was measured against
    // settled bodies, and restoring it against placeholder heights would let
    // the next resize snap the view to the bottom.
    return { scrollTop: restored, pin: 'restore' }
  }

  if (!isNewThread) {
    // A read-state change or a re-render is not a reason to move the view; only
    // a newly arrived message is, and only for a reader already at the bottom.
    if (!grew) return { scrollTop: null, pin: null }
    // Against the height from before the arriving message was rendered, the
    // same way a resize is measured: that message is itself what pushed the
    // bottom away.
    if (!isAtBottom(metrics, previousScrollHeight)) return { scrollTop: null, pin: null }
    return { scrollTop: metrics.scrollHeight, pin: null }
  }

  if (unreadAnchor === null) {
    // Unread messages the container hasn't rendered yet (a thread still
    // loading): leave the view alone rather than jumping to the bottom of a
    // list that is about to change under it.
    if (hasUnread) return { scrollTop: null, pin: null }
    if (lastMessageOffsetTop === null) return { scrollTop: metrics.scrollHeight, pin: null }
    // A fully read thread opens at the *top* of its newest message, not at the
    // bottom of the thread: scrolling to the end lands the reader in the
    // footer of a long message and makes them scroll up to read it. Pinned for
    // the same reason the unread anchor is — bodies grow from their
    // placeholder height afterwards. The browser clamps this for a message
    // shorter than the viewport, which then simply sits at the bottom.
    return { scrollTop: anchorScrollTop(lastMessageOffsetTop, containerOffsetTop), pin: 'last' }
  }
  // Bodies still carry their placeholder height here, so the first expansion
  // would otherwise look like "content grew under the fold" and snap the view
  // to the bottom — past the unread messages the reader opened the thread for.
  return { scrollTop: unreadAnchor, pin: 'unread' }
}
