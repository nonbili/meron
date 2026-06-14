import type { LucideIcon } from 'lucide-react'
import { Check, Pencil, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { ThemeDef } from '../../lib/themes'

// One selectable theme tile: a mini app mock (sidebar / panel / bubbles) drawn
// with the theme's OWN token values via inline styles — never the live CSS
// vars, so every swatch previews its theme regardless of the active one.

function MiniAction({ icon: Icon, label, onClick }: { icon: LucideIcon; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      title={label}
      onClick={(event) => {
        event.stopPropagation()
        onClick()
      }}
      className="flex h-5 w-5 items-center justify-center rounded-md bg-black/45 text-white hover:bg-black/65 cursor-pointer transition-colors"
    >
      <Icon size={10.5} />
    </button>
  )
}

export function ThemeSwatch({
  theme,
  selected,
  onSelect,
  onEdit,
  onDelete,
  large,
}: {
  theme: ThemeDef
  selected: boolean
  onSelect: () => void
  onEdit?: () => void
  onDelete?: () => void
  /** Bigger mock + label, for the theme picker dialog grid. */
  large?: boolean
}) {
  const { t: translate } = useTranslation()
  const t = theme.tokens
  return (
    <div
      role="button"
      tabIndex={0}
      title={theme.name}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onSelect()
        }
      }}
      className={`group relative overflow-hidden rounded-xl border cursor-pointer transition-all ${
        selected ? 'border-accent ring-2 ring-accent/25' : 'border-border hover:border-secondary/40'
      }`}
    >
      <div className={large ? 'flex h-20' : 'flex h-12'} style={{ background: t.bgApp }}>
        <div className={`shrink-0 ${large ? 'w-4' : 'w-2.5'}`} style={{ background: t.bgSidebar }} />
        <div
          className={`shrink-0 border-r ${large ? 'w-12' : 'w-7'}`}
          style={{ background: t.bgChats, borderColor: t.border }}
        />
        <div className={`flex min-w-0 flex-1 flex-col justify-center ${large ? 'gap-1.5 px-2.5' : 'gap-1 px-1.5'}`}>
          <span
            className={`max-w-full self-start rounded-full ${large ? 'h-2.5 w-14' : 'h-1.5 w-8'}`}
            style={{ background: t.bubbleIn, boxShadow: `inset 0 0 0 1px ${t.border}` }}
          />
          <span
            className={`max-w-full self-end rounded-full ${large ? 'h-2.5 w-14' : 'h-1.5 w-8'}`}
            style={{ background: t.bubbleOut }}
          />
          <span
            className={`self-start rounded-full ${large ? 'h-2.5 w-9' : 'h-1.5 w-5'}`}
            style={{ background: t.accent }}
          />
        </div>
      </div>
      <div
        className={`flex items-center justify-between gap-1 border-t border-border/70 bg-chats ${large ? 'px-2.5 py-2' : 'px-2 py-1.5'}`}
      >
        <span
          className={`truncate font-bold ${large ? 'text-[11px]' : 'text-[10px]'} ${selected ? 'text-accent' : 'text-primary'}`}
        >
          {theme.name}
        </span>
        {selected && <Check size={large ? 12 : 11} className="shrink-0 text-accent" />}
      </div>
      {(onEdit || onDelete) && (
        <div className="absolute right-1 top-1 hidden gap-0.5 group-hover:flex">
          {onEdit && <MiniAction icon={Pencil} label={translate('theme.edit')} onClick={onEdit} />}
          {onDelete && <MiniAction icon={Trash2} label={translate('theme.delete')} onClick={onDelete} />}
        </div>
      )}
    </div>
  )
}
