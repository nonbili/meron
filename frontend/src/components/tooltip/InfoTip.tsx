import { useState } from 'react'
import { Info } from 'lucide-react'

// Small info icon with a hover/focus tooltip. The bubble is fixed-positioned
// from the icon's rect because settings panels scroll inside an
// overflow-hidden dialog, which would clip an absolutely-positioned bubble.
export function InfoTip({ hint }: { hint?: string }) {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null)

  if (!hint) return null

  const show = (el: HTMLElement) => {
    const rect = el.getBoundingClientRect()
    setPos({ x: rect.left + rect.width / 2, y: rect.top })
  }

  return (
    <span
      tabIndex={0}
      className="inline-flex shrink-0 text-secondary/60 hover:text-secondary focus:text-secondary focus:outline-none cursor-help"
      onMouseEnter={(e) => show(e.currentTarget)}
      onMouseLeave={() => setPos(null)}
      onFocus={(e) => show(e.currentTarget)}
      onBlur={() => setPos(null)}
    >
      <Info size={12} />
      {pos && (
        <span
          role="tooltip"
          className="fixed z-[60] w-max max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-full whitespace-nowrap rounded-xl border border-border bg-raised px-3 py-2 text-[11px] font-normal leading-snug text-primary shadow-lg pointer-events-none"
          style={{ left: pos.x, top: pos.y - 6 }}
        >
          {hint}
        </span>
      )}
    </span>
  )
}
