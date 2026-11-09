import { useEffect, useState } from 'react'

/**
 * The side panels (SidePanelSlide) and the details panel grow from and collapse
 * to zero width in this long. Matches the pane-open/pane-close keyframes in index.css.
 */
export const PANE_ANIMATION_MS = 200

export type PresencePhase = 'closed' | 'entering' | 'open' | 'exiting'

function prefersReducedMotion() {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches
}

/**
 * Mount/unmount phases for an element that animates in and out. `entering` and
 * `exiting` each last `ms`, so the caller can keep the element mounted while its
 * close animation plays. Timers rather than `animationend`: a phase can never
 * get stuck when no animation runs (reduced motion, a hidden window). What is
 * already open at mount starts settled — only changes animate.
 *
 * The phase is derived during render from the last `open` seen, so the render
 * that flips `open` already carries the new phase, and StrictMode's doubled
 * mount effects cannot replay an animation. A change made with `animate` false
 * settles at once, for an element that should just be there (or not).
 */
export function usePresence(open: boolean, ms: number, animate = true): PresencePhase {
  const [state, setState] = useState(() => ({ open, phase: (open ? 'open' : 'closed') as PresencePhase }))

  let phase = state.phase
  if (state.open !== open) {
    phase = !animate || prefersReducedMotion() ? (open ? 'open' : 'closed') : open ? 'entering' : 'exiting'
    setState({ open, phase })
  }

  useEffect(() => {
    if (phase !== 'entering' && phase !== 'exiting') return
    const timer = setTimeout(
      () => setState((s) => (s.open === open ? { open, phase: open ? 'open' : 'closed' } : s)),
      ms,
    )
    return () => clearTimeout(timer)
  }, [phase, open, ms])

  return phase
}
