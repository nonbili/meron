import { useTranslation } from '../../lib/i18n'
import type { SetupMode } from '../../states/ui'
import { PROVIDERS } from './providerIcons'

// The setup provider picker is a three-option tab group. Keep the labels and
// icons prominent; long helper text lives in the selected panel, not in the tab.
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
  const iconSize = isSetup ? 18 : 20
  if (isSetup) {
    return (
      <div
        className="grid grid-cols-3 gap-1 rounded-2xl border border-border/80 bg-raised p-1 shadow-inner max-[640px]:grid-cols-1"
        role="tablist"
        aria-label={t('accounts.setup.chooseProvider')}
      >
        {PROVIDERS.map((p) => {
          const active = p.isActive(mode)
          return (
            <button
              key={p.id}
              type="button"
              role="tab"
              aria-selected={active}
              onClick={() => setMode(p.mode)}
              className={`group flex min-w-0 items-center justify-center gap-2 rounded-xl px-3 py-2.5 text-center transition-all cursor-pointer max-[640px]:justify-start max-[640px]:text-left ${
                active
                  ? 'bg-chats text-primary shadow-sm ring-1 ring-border/80'
                  : 'text-secondary hover:bg-chats/60 hover:text-primary'
              }`}
            >
              <span className="shrink-0">{p.icon(iconSize)}</span>
              <span className="min-w-0 text-[13px] font-semibold leading-tight">{p.label}</span>
            </button>
          )
        })}
      </div>
    )
  }

  return (
    <div className="grid grid-cols-3 gap-2">
      {PROVIDERS.map((p) => {
        const active = p.isActive(mode)
        return (
          <button
            key={p.id}
            type="button"
            onClick={() => setMode(p.mode)}
            className={`group relative flex items-center text-left transition-all cursor-pointer gap-2.5 rounded-2xl p-2.5 border ${
              active
                ? 'border-accent bg-accent/5 shadow-sm ring-1 ring-accent/30'
                : 'border-border/60 hover:border-border hover:bg-hover'
            }`}
          >
            <span className="flex shrink-0 items-center justify-center rounded-xl border border-border/50 bg-raised h-9 w-9">
              {p.icon(iconSize)}
            </span>
            <span className="flex min-w-0 flex-col">
              <span className="font-bold leading-tight text-primary text-[12px]">{p.label}</span>
              <span className="truncate font-medium text-secondary text-[10px]">
                {t(p.descriptionKey, { defaultValue: p.defaultDescription })}
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
