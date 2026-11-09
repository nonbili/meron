import { useRef, type ReactNode } from 'react'
import { usePresence } from '../../lib/usePresence'
import { PANE_ANIMATION_MS } from '../kanban/KanbanConversationPane'

/**
 * Fixed, like the panels it is modelled on. The thread list and conversation
 * already own the resizable widths in this window; a third draggable edge buys
 * little and costs the layout a lot.
 */
export const SIDE_PANEL_WIDTH = 340

// The right-hand slot (Tasks or the calendar panel) slides in and out like the
// board's conversation pane (KanbanConversationPane): the frame grows from and
// narrows to zero width while the panel inside keeps its full width, so it is
// revealed rather than reflowed every frame. The panel stays on screen while
// it slides out.
// With `animate` false, a change shows or hides the slot at once.
export function SidePanelSlide({
  open,
  animate = true,
  children,
}: {
  open: boolean
  animate?: boolean
  children: ReactNode
}) {
  const phase = usePresence(open, PANE_ANIMATION_MS, animate)
  // Closing the slot (or deleting the last task list) takes its content away
  // in the same render; the last panel shown slides out instead of an empty frame.
  const lastChildren = useRef(children)
  if (children != null) lastChildren.current = children
  if (phase === 'closed') return null

  const animation = phase === 'entering' ? ' animate-pane-open' : phase === 'exiting' ? ' animate-pane-close' : ''

  return (
    <div
      data-pane-phase={phase}
      // Hidden on a narrow window, like the panels themselves.
      className={`flex min-h-0 shrink-0 overflow-hidden max-[900px]:hidden${animation}`}
      style={{ width: SIDE_PANEL_WIDTH }}
    >
      {children ?? lastChildren.current}
    </div>
  )
}
