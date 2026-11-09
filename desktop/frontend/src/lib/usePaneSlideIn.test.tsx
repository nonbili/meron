import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
import { useRef } from 'react'
import { usePaneSlideIn } from './usePaneSlideIn'
import type { PresencePhase } from './usePresence'

function Probe({ phase }: { phase: PresencePhase }) {
  const ref = useRef<HTMLDivElement>(null)
  const lifted = usePaneSlideIn(ref, phase, 100)
  return <div ref={ref} data-testid="pane" data-lifted={String(lifted)} />
}

// Frames and slides run only when a test says so.
let frames: Map<number, FrameRequestCallback>
let nextFrame: number
let slides: { keyframes: Keyframe[]; cancelled: boolean; finish: () => void }[]
const originals = {
  raf: globalThis.requestAnimationFrame,
  caf: globalThis.cancelAnimationFrame,
  animate: (HTMLElement.prototype as any).animate,
}

function runFrame() {
  const pending = [...frames.values()]
  frames.clear()
  act(() => pending.forEach((callback) => callback(performance.now())))
}

const lifted = (view: ReturnType<typeof render>) => view.getByTestId('pane').dataset.lifted

describe('usePaneSlideIn', () => {
  beforeEach(() => {
    frames = new Map()
    nextFrame = 1
    slides = []
    globalThis.requestAnimationFrame = (callback) => {
      const id = nextFrame++
      frames.set(id, callback)
      return id
    }
    globalThis.cancelAnimationFrame = (id) => {
      frames.delete(id)
    }
    ;(HTMLElement.prototype as any).animate = function (keyframes: Keyframe[]) {
      let resolve = () => {}
      const finished = new Promise<void>((done) => (resolve = done))
      const slide = { keyframes, cancelled: false, finish: () => resolve() }
      slides.push(slide)
      return {
        finished,
        cancel: () => {
          slide.cancelled = true
        },
      }
    }
  })

  afterEach(() => {
    cleanup()
    globalThis.requestAnimationFrame = originals.raf
    globalThis.cancelAnimationFrame = originals.caf
    ;(HTMLElement.prototype as any).animate = originals.animate
  })

  it('waits two frames before sliding, so the first paint is not part of the slide', () => {
    const view = render(<Probe phase="closed" />)
    view.rerender(<Probe phase="entering" />)
    expect(lifted(view)).toBe('true')

    runFrame()
    expect(slides).toHaveLength(0)
    runFrame()
    expect(slides).toHaveLength(1)
    expect(slides[0].keyframes[0].transform).toBe('translateX(calc(100% - 1px))')
  })

  it('stays lifted past the entering phase until the slide has finished', async () => {
    const view = render(<Probe phase="closed" />)
    view.rerender(<Probe phase="entering" />)
    runFrame()
    runFrame()

    view.rerender(<Probe phase="open" />)
    expect(lifted(view)).toBe('true')
    expect(slides[0].cancelled).toBe(false)

    await act(async () => slides[0].finish())
    expect(lifted(view)).toBe('false')
    // Held in place until the pane closes; then it would outrank the close animation's CSS.
    expect(slides[0].cancelled).toBe(false)
    view.rerender(<Probe phase="exiting" />)
    expect(slides[0].cancelled).toBe(true)
  })

  it('still slides when the first frames come after the entering phase is over', async () => {
    // A slow first paint: the presence timer settles before either frame runs.
    // Cancelling the frames then would leave the pane lifted for good.
    const view = render(<Probe phase="closed" />)
    view.rerender(<Probe phase="entering" />)
    view.rerender(<Probe phase="open" />)

    runFrame()
    runFrame()
    expect(slides).toHaveLength(1)
    await act(async () => slides[0].finish())
    expect(lifted(view)).toBe('false')
  })

  it('stops a pending or running slide when the pane closes', () => {
    const view = render(<Probe phase="closed" />)
    view.rerender(<Probe phase="entering" />)
    runFrame()
    runFrame()

    view.rerender(<Probe phase="exiting" />)
    expect(slides[0].cancelled).toBe(true)
    expect(lifted(view)).toBe('false')

    view.rerender(<Probe phase="entering" />)
    view.rerender(<Probe phase="exiting" />)
    runFrame()
    runFrame()
    expect(slides).toHaveLength(1)
  })

  it('only lifts for the entering phase where Web Animations are missing', () => {
    ;(HTMLElement.prototype as any).animate = undefined
    const view = render(<Probe phase="closed" />)
    view.rerender(<Probe phase="entering" />)
    expect(lifted(view)).toBe('true')
    view.rerender(<Probe phase="open" />)
    expect(lifted(view)).toBe('false')
  })
})
