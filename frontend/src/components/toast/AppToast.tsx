import { AlertCircle, Check } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { ui$, runToastUndo } from '../../states/ui'

// The floating status toast, with an optional Undo action. Reads its state
// directly from ui$ and renders nothing when there's no active toast.
export function AppToast() {
  const toast = useValue(ui$.toast)
  const toastTone = useValue(ui$.toastTone)
  const toastUndo = useValue(ui$.toastUndo)

  if (!toast) return null

  return (
    <div
      className={`fixed bottom-6 left-1/2 -translate-x-1/2 animate-slide-up flex items-center gap-2 rounded-full bg-black/80 py-2 pl-4 text-xs font-semibold text-white shadow-xl z-50 ${
        toastUndo ? 'pr-2' : 'pr-4'
      }`}
    >
      {toastTone === 'error' ? (
        <AlertCircle size={14} className="text-rose-400" />
      ) : (
        <Check size={14} className="text-emerald-400" />
      )}
      <span>{toast}</span>
      {toastUndo && (
        <button
          onClick={runToastUndo}
          className="ml-1 rounded-full bg-white/15 px-2.5 py-1 font-bold text-white hover:bg-white/25 transition-colors cursor-pointer"
        >
          Undo
        </button>
      )}
    </div>
  )
}
