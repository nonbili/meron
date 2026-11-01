import { AlertTriangle, Pencil } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { Dialog } from './Dialog'
import { settleConfirm, ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { TextInput } from '../field/Field'

export function AppConfirm() {
  const confirm = useValue(ui$.confirm)

  if (!confirm) return null

  const isDanger = confirm.tone === 'danger'
  const input = confirm.input
  const Icon = input ? Pencil : AlertTriangle

  return (
    <Dialog
      onClose={() => settleConfirm(false)}
      backdropClassName="z-[120] bg-black/45 dark:bg-black/45 backdrop-blur-none"
      role="alertdialog"
      aria-labelledby="app-confirm-title"
      aria-describedby="app-confirm-message"
      className="max-w-sm rounded-xl p-4"
    >
      <div className="mb-3 flex items-start gap-3">
        <div
          className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl ${
            isDanger ? 'bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-400' : 'bg-accent/10 text-accent'
          }`}
        >
          <Icon size={17} />
        </div>
        <div className="min-w-0">
          <h2 id="app-confirm-title" className="text-sm font-bold text-primary">
            {confirm.title}
          </h2>
          {confirm.message && (
            <p id="app-confirm-message" className="mt-1 whitespace-pre-line text-sm leading-relaxed text-secondary">
              {confirm.message}
            </p>
          )}
        </div>
      </div>
      {input && (
        <form
          className="mb-3"
          onSubmit={(event) => {
            event.preventDefault()
            if (input.value.trim()) settleConfirm(true)
          }}
        >
          <TextInput
            autoFocus
            value={input.value}
            placeholder={input.placeholder}
            aria-labelledby="app-confirm-title"
            onChange={(event) => ui$.confirm.input.value.set(event.target.value)}
            onFocus={(event) => event.currentTarget.select()}
            className="h-8 w-full rounded-lg px-2"
          />
        </form>
      )}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={() => settleConfirm(false)}>
          {confirm.cancelLabel}
        </Button>
        <Button
          autoFocus={!input}
          disabled={!!input && !input.value.trim()}
          variant={isDanger ? 'danger' : 'primary'}
          size="sm"
          onClick={() => settleConfirm(true)}
        >
          {confirm.confirmLabel}
        </Button>
      </div>
    </Dialog>
  )
}
