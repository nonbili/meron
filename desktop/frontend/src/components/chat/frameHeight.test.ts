import { describe, expect, it } from 'bun:test'
import { MIN_FRAME_HEIGHT, measureFrameHeight, type FrameMetrics } from './frameHeight'

// A document laid out in a frame `viewport` tall whose in-flow content is
// `content` tall, plus optional out-of-flow content reaching `overflow`.
function metrics({
  viewport,
  content,
  overflow = 0,
  bodyTop = 0,
}: {
  viewport: number
  content: number
  overflow?: number
  bodyTop?: number
}): FrameMetrics {
  return {
    bodyTop,
    bodyHeight: Math.max(0, content - bodyTop),
    rootTop: 0,
    rootHeight: content,
    // The root is the scrolling element: its scroll area spans the content, the
    // out-of-flow boxes, and — always — the viewport itself.
    scrollHeight: Math.max(viewport, content, overflow),
    clientHeight: viewport,
  }
}

describe('measureFrameHeight', () => {
  it('shrinks to short content laid out in a taller frame', () => {
    // The placeholder frame is 120 tall; a one-line email is not.
    expect(measureFrameHeight(metrics({ viewport: 120, content: 24 })).height).toBe(24)
  })

  it('grows to content taller than the frame', () => {
    expect(measureFrameHeight(metrics({ viewport: 120, content: 900 })).height).toBe(900)
  })

  it('does not treat ordinary tall content as out-of-flow', () => {
    // It overflows the placeholder viewport like out-of-flow content does, but
    // the boxes account for all of it, so there is nothing to carry forward.
    expect(measureFrameHeight(metrics({ viewport: 120, content: 900 })).overflowExtent).toBe(0)
  })

  it('gives height back when tall content reflows shorter', () => {
    // Regression: a tall mail retained as an overflow floor could never shrink,
    // so widening the frame (text reflowing 900 -> 500) left 400px of blank.
    const first = measureFrameHeight(metrics({ viewport: 120, content: 900 }))
    const second = measureFrameHeight(metrics({ viewport: 900, content: 500 }), first.overflowExtent)
    expect(second.height).toBe(500)
  })

  it('carries only a scroll area the boxes cannot account for', () => {
    // 900 of in-flow content with an out-of-flow block reaching 1000.
    const measurement = measureFrameHeight(metrics({ viewport: 120, content: 900, overflow: 1000 }))
    expect(measurement.height).toBe(1000)
    expect(measurement.overflowExtent).toBe(1000)
  })

  it('never measures below the floor', () => {
    expect(measureFrameHeight(metrics({ viewport: 120, content: 0 })).height).toBe(MIN_FRAME_HEIGHT)
  })

  it('rounds fractional content up', () => {
    expect(measureFrameHeight(metrics({ viewport: 120, content: 42.3 })).height).toBe(43)
  })

  it("counts the body's margin above it", () => {
    expect(measureFrameHeight(metrics({ viewport: 120, content: 60, bodyTop: 16 })).height).toBe(60)
  })

  it('covers out-of-flow content the box rects do not span', () => {
    // A body holding only an absolutely positioned 200px block: the boxes
    // beneath it measure empty, and only the root's scroll area sees it.
    const measurement = measureFrameHeight(metrics({ viewport: 120, content: 0, overflow: 200 }))
    expect(measurement.height).toBe(200)
    expect(measurement.overflowExtent).toBe(200)
  })

  it('holds an out-of-flow height steady once the frame has grown to fit it', () => {
    // Regression: re-deriving the overflow each pass made the frame oscillate.
    // Grown to 200, the scroll area no longer overflows, so the signal is gone
    // — and the empty boxes beneath would otherwise measure back to the floor.
    const first = measureFrameHeight(metrics({ viewport: 120, content: 0, overflow: 200 }))
    const second = measureFrameHeight(metrics({ viewport: 200, content: 0, overflow: 200 }), first.overflowExtent)
    expect(second.height).toBe(200)

    const third = measureFrameHeight(metrics({ viewport: 200, content: 0, overflow: 200 }), second.overflowExtent)
    expect(third.height).toBe(200)
  })

  it('still grows when out-of-flow content outgrows the extent already seen', () => {
    const first = measureFrameHeight(metrics({ viewport: 120, content: 0, overflow: 200 }))
    const second = measureFrameHeight(metrics({ viewport: 200, content: 0, overflow: 320 }), first.overflowExtent)
    expect(second.height).toBe(320)
  })

  it('lets in-flow content outgrow a carried-over overflow extent', () => {
    const measurement = measureFrameHeight(metrics({ viewport: 200, content: 480, overflow: 200 }), 200)
    expect(measurement.height).toBe(480)
  })

  it('carries no extent for a document that never overflowed', () => {
    expect(measureFrameHeight(metrics({ viewport: 120, content: 24 })).overflowExtent).toBe(0)
  })

  it('ignores a sub-pixel scroll area over the viewport', () => {
    const measurement = measureFrameHeight({
      bodyTop: 0,
      bodyHeight: 24,
      rootTop: 0,
      rootHeight: 24,
      scrollHeight: 120.5,
      clientHeight: 120,
    })
    expect(measurement.height).toBe(24)
    expect(measurement.overflowExtent).toBe(0)
  })
})
