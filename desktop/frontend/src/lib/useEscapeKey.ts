import { useEffect, useRef } from 'react'

// Escape handlers form a stack so only the topmost layer closes: a submenu
// opened over a menu, or a menu opened inside a dialog, swallows the key and
// leaves whatever is underneath it open.
const handlers: Array<() => void> = []

function onWindowKeyDown(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  const top = handlers[handlers.length - 1]
  if (!top) return
  // Capture-phase, so the app-wide Escape shortcut never sees the key while a
  // dialog or menu is open.
  event.preventDefault()
  event.stopPropagation()
  top()
}

/**
 * Closes a dialog or popup on Escape. Listens on the window so the key works
 * no matter which element inside holds focus.
 */
export function useEscapeKey(onEscape: () => void, enabled = true) {
  const onEscapeRef = useRef(onEscape)
  onEscapeRef.current = onEscape

  useEffect(() => {
    if (!enabled) return
    const handler = () => onEscapeRef.current()
    if (handlers.length === 0) window.addEventListener('keydown', onWindowKeyDown, true)
    handlers.push(handler)
    return () => {
      const index = handlers.lastIndexOf(handler)
      if (index !== -1) handlers.splice(index, 1)
      if (handlers.length === 0) window.removeEventListener('keydown', onWindowKeyDown, true)
    }
  }, [enabled])
}
