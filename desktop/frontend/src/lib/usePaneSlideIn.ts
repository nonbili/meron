import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import type { PresencePhase } from './usePresence'

// Matches the paneSlideOut keyframes in index.css.
const PANE_EASING = 'cubic-bezier(0.16, 1, 0.3, 1)'

/**
 * Slides a pane in from the right edge of its row once it has been painted.
 *
 * A pane is built in the same render that opens it. On a slower machine laying
 * it out and painting it takes several frames, and a CSS animation's clock runs
 * meanwhile, so the first frame on screen showed the pane nearly in place: it
 * popped open. Here the pane waits just off the edge (index.css,
 * `animate-pane-slide-in`; one pixel showing, so its first tiles get painted)
 * for two frames, and only then starts its slide.
 *
 * Returns whether the pane is still lifted over its neighbour: from `entering`
 * until the slide has finished, which can run past the presence phase. The
 * neighbour takes its new width when this turns false.
 */
export function usePaneSlideIn(ref: RefObject<HTMLElement | null>, phase: PresencePhase, ms: number): boolean {
  const [sliding, setSliding] = useState(false)
  // The pending frames and the running slide outlive the `entering` phase: a
  // slow first paint can take longer than the phase itself.
  const pending = useRef({ frame: 0, slide: null as Animation | null })
  const entering = phase === 'entering'
  const closing = phase === 'exiting' || phase === 'closed'

  useLayoutEffect(() => {
    if (!entering) return
    const el = ref.current
    // No Web Animations (tests): just settle.
    if (!el || typeof el.animate !== 'function') return
    const state = pending.current
    stop(state)
    setSliding(true)
    state.frame = requestAnimationFrame(() => {
      state.frame = requestAnimationFrame(() => {
        state.frame = 0
        const slide = el.animate([{ transform: 'translateX(calc(100% - 1px))' }, { transform: 'none' }], {
          duration: ms,
          easing: PANE_EASING,
          fill: 'forwards',
        })
        state.slide = slide
        slide.finished.then(
          () => setSliding(false),
          () => {},
        )
      })
    })
  }, [entering, ms, ref])

  // Closing (or unmounting) stops a slide that is pending or running; the close
  // animation takes over from wherever the pane is.
  useEffect(() => {
    if (!closing) return
    stop(pending.current)
    setSliding(false)
  }, [closing])
  useEffect(() => {
    const state = pending.current
    return () => stop(state)
  }, [])

  return !closing && (entering || sliding)
}

function stop(state: { frame: number; slide: Animation | null }) {
  cancelAnimationFrame(state.frame)
  state.frame = 0
  state.slide?.cancel()
  state.slide = null
}
