import { useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react'
import { usePresence } from '../../lib/usePresence'

/** Matches the pane-open/pane-close keyframes in index.css. */
export const PANE_ANIMATION_MS = 200

// The board's conversation pane. It slides in from the right edge over the
// board, which narrows once when it is in place (index.css, animate-pane-open):
// the board is not reflowed on every frame. The preferred width is in `vw`,
// capped at the board row's measured width so Tasks and narrow windows cannot
// push the conversation outside the board. Measuring avoids a CSS size container,
// which would trap fixed-position menus and dialogs. The conversation — HTML
// frames included — is laid out once at its final width and slides in whole.
// Closing clears the conversation from state before the pane goes, so the
// pane slides out as an empty panel instead of an empty-state view.
export function KanbanConversationPane({
  open,
  widthPercent,
  resizeTitle,
  onResizeStart,
  children,
}: {
  open: boolean
  widthPercent: number
  resizeTitle: string
  onResizeStart: (event: ReactPointerEvent<HTMLDivElement>) => void
  children: ReactNode
}) {
  const phase = usePresence(open, PANE_ANIMATION_MS)
  const paneRef = useRef<HTMLDivElement>(null)
  const [boardWidth, setBoardWidth] = useState<number | null>(null)
  const mounted = phase !== 'closed'
  useLayoutEffect(() => {
    const row = paneRef.current?.parentElement
    if (!row) return
    const measure = () => setBoardWidth(row.getBoundingClientRect().width)
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(row)
    return () => observer.disconnect()
  }, [mounted])
  if (phase === 'closed') return null

  const animation = phase === 'entering' ? 'animate-pane-open' : phase === 'exiting' ? 'animate-pane-close' : ''
  const preferredWidth = `max(320px, ${widthPercent}vw)`
  const width = boardWidth === null ? preferredWidth : `min(${preferredWidth}, ${boardWidth}px)`

  return (
    <div
      ref={paneRef}
      data-pane-phase={phase}
      className={`relative flex shrink-0 overflow-hidden border-l border-border/60 bg-chat shadow-[-4px_0_6px_-2px_rgba(0,0,0,0.08)] ${animation}`}
      style={{ width, maxWidth: '100%' }}
    >
      {phase !== 'exiting' && (
        <>
          <div
            className="absolute left-0 top-0 z-30 h-full w-2 -translate-x-1 cursor-col-resize"
            onPointerDown={onResizeStart}
            title={resizeTitle}
          >
            <div className="mx-auto h-full w-px bg-transparent hover:bg-accent" />
          </div>
          <div className="flex h-full shrink-0" style={{ width: `max(0px, calc(${width} - 1px))` }}>
            {children}
          </div>
        </>
      )}
    </div>
  )
}
