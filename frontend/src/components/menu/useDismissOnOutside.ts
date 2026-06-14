import { useEffect, useRef } from 'react'

// Closes a popup whenever the user interacts outside it. Listens to
// capture-phase pointerdown instead of bubbled click so that (a) another
// menu's trigger calling stopPropagation() can't keep this one open, and
// (b) right-clicks dismiss it too — only one menu should be visible at a
// time. Also closes on outside scroll and on resize.
export function useDismissOnOutside(
  active: boolean,
  isInside: (target: EventTarget | null) => boolean,
  close: () => void,
) {
  const isInsideRef = useRef(isInside)
  const closeRef = useRef(close)
  isInsideRef.current = isInside
  closeRef.current = close

  useEffect(() => {
    if (!active) return
    const closeIfOutside = (event: Event) => {
      if (isInsideRef.current(event.target)) return
      closeRef.current()
    }
    const close = () => closeRef.current()
    window.addEventListener('pointerdown', closeIfOutside, true)
    window.addEventListener('scroll', closeIfOutside, true)
    window.addEventListener('resize', close)
    return () => {
      window.removeEventListener('pointerdown', closeIfOutside, true)
      window.removeEventListener('scroll', closeIfOutside, true)
      window.removeEventListener('resize', close)
    }
  }, [active])
}

// Containment check for popups marked with a data attribute (used when the
// popup renders in a fixed-position layer, e.g. the thread context menu).
export function targetWithin(target: EventTarget | null, selector: string) {
  return Boolean((target as Element | null)?.closest?.(selector))
}
