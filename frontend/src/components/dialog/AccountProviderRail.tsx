import type { SetupMode } from '../../states/ui'
import { PROVIDERS } from './providerIcons'

// The dialog-variant provider picker: a vertical rail (mirrors the Settings
// dialog nav) listing the four account kinds. Selecting one swaps the form shown
// in the panel to its right, so "pick a type → fill in details" reads as one flow.
export function AccountProviderRail({ mode, setMode }: { mode: SetupMode; setMode: (mode: SetupMode) => void }) {
  return (
    <nav className="w-44 shrink-0 border-r border-border/70 p-3 flex flex-col gap-1 bg-raised overflow-y-auto">
      {PROVIDERS.map((p) => {
        const active = mode === p.id
        return (
          <button
            key={p.id}
            onClick={() => setMode(p.id)}
            className={`flex items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition-colors cursor-pointer ${
              active ? 'bg-accent/10' : 'hover:bg-hover'
            }`}
          >
            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border border-border/50 bg-raised">
              {p.icon(16)}
            </span>
            <span className="flex min-w-0 flex-col leading-tight">
              <span className={`truncate text-[12px] font-bold ${active ? 'text-accent' : 'text-primary'}`}>
                {p.label}
              </span>
              <span className="truncate text-[9.5px] font-medium text-secondary">{p.description}</span>
            </span>
          </button>
        )
      })}
    </nav>
  )
}
