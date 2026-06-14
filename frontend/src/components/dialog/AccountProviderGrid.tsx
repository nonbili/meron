import { useTranslation } from 'react-i18next'
import type { SetupMode } from '../../states/ui'
import { PROVIDERS } from './providerIcons'

// The provider picker: a 2×2 grid of branded cards replacing the old text-only
// segmented control. Each card carries the provider glyph, a name, and a short
// descriptor; the active one is highlighted with the accent ring + check.
export function AccountProviderGrid({
  mode,
  setMode,
  isSetup,
}: {
  mode: SetupMode
  setMode: (mode: SetupMode) => void
  isSetup: boolean
}) {
  const { t } = useTranslation()
  const iconSize = isSetup ? 24 : 20
  return (
    <div className={`grid grid-cols-2 ${isSetup ? 'gap-3' : 'gap-2'}`}>
      {PROVIDERS.map((p) => {
        const active = mode === p.id
        return (
          <button
            key={p.id}
            type="button"
            onClick={() => setMode(p.id)}
            className={`group relative flex items-center text-left transition-all cursor-pointer ${
              isSetup ? 'gap-3.5 rounded-2xl p-4' : 'gap-2.5 rounded-2xl p-2.5'
            } border ${
              active
                ? 'border-accent bg-accent/5 shadow-sm ring-1 ring-accent/30'
                : 'border-border/60 hover:border-border hover:bg-hover'
            }`}
          >
            <span
              className={`flex shrink-0 items-center justify-center rounded-xl border border-border/50 bg-raised ${
                isSetup ? 'h-11 w-11' : 'h-9 w-9'
              }`}
            >
              {p.icon(iconSize)}
            </span>
            <span className="flex min-w-0 flex-col">
              <span className={`font-bold leading-tight text-primary ${isSetup ? 'text-[15px]' : 'text-[12px]'}`}>
                {p.label}
              </span>
              <span className={`truncate font-medium text-secondary ${isSetup ? 'text-[12px]' : 'text-[10px]'}`}>
                {t(p.descriptionKey)}
              </span>
            </span>
            {/* A radio-style dot — this is a single-choice picker, so the mark
                means "selected", not "done" (a check would imply completion). */}
            <span
              className={`absolute right-2 top-2 flex h-4 w-4 items-center justify-center rounded-full border transition-colors ${
                active ? 'border-accent' : 'border-border/70'
              }`}
            >
              {active && <span className="h-2 w-2 rounded-full bg-accent" />}
            </span>
          </button>
        )
      })}
    </div>
  )
}
