import { useEffect } from 'react'
import { AlertTriangle } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { settleConfirm, ui$ } from '../../states/ui'
import { Button } from '../button/Button'

export function AppConfirm() {
  const confirm = useValue(ui$.confirm)

  useEffect(() => {
    if (!confirm) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') settleConfirm(false)
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [confirm])

  if (!confirm) return null

  const isDanger = confirm.tone === 'danger'

  return (
    <div
      className="fixed inset-0 z-[120] flex items-center justify-center bg-black/45 p-4"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) settleConfirm(false)
      }}
    >
      <section
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="app-confirm-title"
        aria-describedby="app-confirm-message"
        className="w-full max-w-sm rounded-xl border border-border bg-chats p-4 shadow-2xl"
      >
        <div className="mb-3 flex items-start gap-3">
          <div
            className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl ${
              isDanger ? 'bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-400' : 'bg-accent/10 text-accent'
            }`}
          >
            <AlertTriangle size={17} />
          </div>
          <div className="min-w-0">
            <h2 id="app-confirm-title" className="text-sm font-bold text-primary">
              {confirm.title}
            </h2>
            <p id="app-confirm-message" className="mt-1 text-sm leading-relaxed text-secondary">
              {confirm.message}
            </p>
          </div>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={() => settleConfirm(false)}>
            {confirm.cancelLabel}
          </Button>
          <Button autoFocus variant={isDanger ? 'danger' : 'primary'} size="sm" onClick={() => settleConfirm(true)}>
            {confirm.confirmLabel}
          </Button>
        </div>
      </section>
    </div>
  )
}
