import { useRef, type ReactNode } from 'react'
import { usePresence } from '../../lib/usePresence'
import { PANE_ANIMATION_MS } from '../kanban/KanbanConversationPane'
import { TASKS_PANEL_WIDTH } from './TasksPanel'

// The Tasks panel slides in and out like the board's conversation pane
// (KanbanConversationPane): the frame grows from and narrows to zero width
// while the panel inside keeps its full width, so the list is revealed rather
// than reflowed every frame. The list stays on screen while it slides out.
export function TasksSlide({ open, children }: { open: boolean; children: ReactNode }) {
  const phase = usePresence(open, PANE_ANIMATION_MS)
  // Deleting the last list closes the panel and takes its content away in the
  // same render; the last panel shown slides out instead of an empty frame.
  const lastChildren = useRef(children)
  if (children != null) lastChildren.current = children
  if (phase === 'closed') return null

  const animation = phase === 'entering' ? ' animate-pane-open' : phase === 'exiting' ? ' animate-pane-close' : ''

  return (
    <div
      data-pane-phase={phase}
      // Hidden on a narrow window, like the panel itself.
      className={`flex min-h-0 shrink-0 overflow-hidden max-[900px]:hidden${animation}`}
      style={{ width: TASKS_PANEL_WIDTH }}
    >
      {children ?? lastChildren.current}
    </div>
  )
}
