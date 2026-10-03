import type { PointerEvent as ReactPointerEvent, ReactNode } from 'react'
import { usePresence } from '../../lib/usePresence'

/** Matches the pane-open/pane-close keyframes in index.css. */
export const PANE_ANIMATION_MS = 200

// The board's conversation pane. It slides open by growing from zero width
// while its content keeps the final width. Both are in `vw` (a share of the
// window, which <main> spans; the pane sits inside the board, under its
// header, so a % would be of the board instead; a size container would trap
// its fixed-position menus and dialogs), so the conversation — HTML frames that measure their own height
// included — is laid out once and revealed rather than reflowed every frame.
// Closing clears the conversation from state before the pane goes, so the
// pane collapses as an empty panel instead of sliding out an empty-state view.
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
  if (phase === 'closed') return null

  const animation = phase === 'entering' ? ' animate-pane-open' : phase === 'exiting' ? ' animate-pane-close' : ''
  const width = `max(320px, ${widthPercent}vw)`

  return (
    <div
      data-pane-phase={phase}
      className={`relative flex shrink-0 overflow-hidden border-l border-border/60 bg-chat${animation}`}
      style={{ width }}
    >
      {phase !== 'exiting' && (
        <>
          <div
            className="absolute left-0 top-0 z-20 h-full w-2 -translate-x-1 cursor-col-resize"
            onPointerDown={onResizeStart}
            title={resizeTitle}
          >
            <div className="mx-auto h-full w-px bg-transparent hover:bg-accent" />
          </div>
          <div className="flex h-full shrink-0" style={{ width: `max(320px, ${widthPercent}vw)` }}>
            {children}
          </div>
        </>
      )}
    </div>
  )
}
