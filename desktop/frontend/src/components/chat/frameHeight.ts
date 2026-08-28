// How tall a self-sizing message frame should be, given what its document
// measures. Kept apart from the frame component so the arithmetic — which no
// jsdom test can reach through a real iframe, since jsdom has no layout — is
// directly testable. The mobile apps run the same rules in the height-reporting
// script injected by `MessageBubbleUi.kt`; keep the two in step.

/** Floor for a measured frame. Just enough to keep an empty body clickable. */
export const MIN_FRAME_HEIGHT = 20

/** Slack absorbing sub-pixel rounding when comparing against the viewport. */
const OVERFLOW_EPSILON = 1

export interface FrameMetrics {
  /** `body.getBoundingClientRect()`: `top` carries the body's margin. */
  bodyTop: number
  bodyHeight: number
  /** `documentElement.getBoundingClientRect()`. */
  rootTop: number
  rootHeight: number
  /** `documentElement.scrollHeight` — never less than the frame's viewport. */
  scrollHeight: number
  /** `documentElement.clientHeight` — the frame's own viewport. */
  clientHeight: number
}

export interface FrameMeasurement {
  height: number
  /** Feed back into the next measurement of the same document. */
  overflowExtent: number
}

// The root element is the document's scrolling element, so its scrollHeight is
// floored at the frame's own viewport height. Sizing off it makes the frame a
// ratchet — a one-line email measures as whatever height it was first laid out
// at and can only ever grow — so the box rects, whose heights are auto and so
// track the content, are what size the frame.
//
// That leaves content the box rects don't span: an absolutely positioned block
// whose containing block is the initial one contributes to the root's scroll
// area and to nothing else. scrollHeight is the only handle on it, and it only
// says anything while the frame is still shorter than that content — once the
// frame grows to fit, scrollHeight and clientHeight agree again and the signal
// is indistinguishable from "the document is empty". So an overflow extent,
// once seen, is carried forward as a floor for the rest of the document's life
// rather than re-derived every pass; re-deriving it is what would make the
// frame oscillate between the overflow height and the empty box beneath it.
//
// A floor is only worth carrying for a scroll area the boxes cannot account
// for. A tall ordinary email overflows the placeholder viewport too, and
// retaining *that* would pin the frame to its first layout: text that reflows
// shorter when the frame widens could never give the height back.
export function measureFrameHeight(metrics: FrameMetrics, previousOverflowExtent = 0): FrameMeasurement {
  const boxExtent = Math.max(metrics.bodyTop + metrics.bodyHeight, metrics.rootTop + metrics.rootHeight)
  const outOfFlow =
    metrics.scrollHeight > metrics.clientHeight + OVERFLOW_EPSILON &&
    metrics.scrollHeight > boxExtent + OVERFLOW_EPSILON
  const overflowExtent = outOfFlow ? Math.max(previousOverflowExtent, metrics.scrollHeight) : previousOverflowExtent

  return { height: Math.max(MIN_FRAME_HEIGHT, Math.ceil(Math.max(boxExtent, overflowExtent))), overflowExtent }
}

/** Reads the metrics `measureFrameHeight` needs out of a live frame document. */
export function frameMetrics(doc: Document): FrameMetrics {
  // Every term is in the frame's own coordinate space: rects always include
  // zoom, and the root element is never zoomed (only the body is, for the
  // message text size). The body's scrollHeight / offsetHeight are deliberately
  // left out — engines disagree on whether those report the zoomed or the
  // unzoomed box, so mixing them in would size the frame off by the zoom factor
  // in one direction or the other.
  const root = doc.documentElement
  const bodyRect = doc.body?.getBoundingClientRect()
  const rootRect = root?.getBoundingClientRect()

  return {
    bodyTop: bodyRect?.top ?? 0,
    bodyHeight: bodyRect?.height ?? 0,
    rootTop: rootRect?.top ?? 0,
    rootHeight: rootRect?.height ?? 0,
    scrollHeight: root?.scrollHeight ?? 0,
    clientHeight: root?.clientHeight ?? 0,
  }
}
