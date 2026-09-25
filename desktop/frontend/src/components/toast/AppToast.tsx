import { AlertCircle, Check } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { ui$, runToastUndo } from '../../states/ui'
import { TASKS_PANEL_WIDTH } from '../tasks/TasksPanel'

// The floating status toast, with an optional Undo action. Reads its state
// directly from ui$ and renders nothing when there's no active toast.
export function AppToast() {
  const toast = useValue(ui$.toast)
  const toastTone = useValue(ui$.toastTone)
  const toastUndo = useValue(ui$.toastUndo)
  const toastPlacement = useValue(ui$.toastPlacement)
  const tasksPanelOpen = useValue(ui$.tasksPanelOpen)
  // Over the Tasks panel: centred on its column, no wider than it, and lifted
  // clear of its pinned Completed heading. Only where the panel shows — it
  // hides at 900px and below, and there the toast falls back to the window
  // centre.
  const overTasks = toastPlacement === 'tasks' && tasksPanelOpen

  if (!toast) return null

  return (
    <div
      role="status"
      aria-live="polite"
      className={`fixed bottom-6 left-1/2 -translate-x-1/2 animate-slide-up flex items-center gap-2 rounded-full bg-black/80 py-2 pl-4 text-xs font-semibold text-white shadow-xl z-50 ${
        toastUndo ? 'pr-2' : 'pr-4'
      } ${
        overTasks
          ? 'min-[901px]:bottom-12 min-[901px]:left-auto min-[901px]:right-[var(--tasks-toast-right)] min-[901px]:translate-x-1/2 min-[901px]:max-w-[var(--tasks-toast-width)]'
          : ''
      }`}
      style={
        overTasks
          ? ({
              '--tasks-toast-right': `${TASKS_PANEL_WIDTH / 2}px`,
              '--tasks-toast-width': `${TASKS_PANEL_WIDTH - 24}px`,
            } as React.CSSProperties)
          : undefined
      }
    >
      {toastTone === 'error' ? (
        <AlertCircle size={14} className="shrink-0 text-rose-400" />
      ) : (
        <Check size={14} className="shrink-0 text-emerald-400" />
      )}
      {/* In the narrow panel a long list name truncates rather than pushing
          Undo off-screen. */}
      <span className={overTasks ? 'min-w-0 min-[901px]:truncate' : undefined} title={overTasks ? toast : undefined}>
        {toast}
      </span>
      {toastUndo && (
        <button
          onClick={runToastUndo}
          className="ml-1 shrink-0 rounded-full bg-white/15 px-2.5 py-1 font-bold text-white hover:bg-white/25 transition-colors cursor-pointer"
        >
          Undo
        </button>
      )}
    </div>
  )
}
