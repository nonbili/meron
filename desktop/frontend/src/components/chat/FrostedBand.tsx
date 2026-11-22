import type { CSSProperties } from 'react'

// The mask softening the edge that shows in the open, over its first 1rem.
const FADE_MASK = {
  top: '[mask-image:linear-gradient(to_bottom,transparent,black_1rem)]',
  bottom: '[mask-image:linear-gradient(to_top,transparent,black_1rem)]',
} as const

/**
 * Frosts what scrolls on under floating controls: messages stay visible as
 * movement without their text competing with the controls. `fade` softens
 * an edge that shows in the open; leave it out when both are covered. Place it with
 * `className`, stopping short of the right edge (`right-3`) to stay off the
 * scrollbar. It takes the pointer, so the slivers under it can't be clicked.
 */
export function FrostedBand({
  fade,
  className,
  style,
}: {
  fade?: 'top' | 'bottom'
  className: string
  style?: CSSProperties
}) {
  return (
    <div
      style={style}
      className={`pointer-events-auto absolute bg-chat/30 backdrop-blur-md ${fade ? FADE_MASK[fade] : ''} ${className}`}
    />
  )
}
