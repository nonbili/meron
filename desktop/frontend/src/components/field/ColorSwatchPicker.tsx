import { useEffect, useRef, useState } from 'react'
import { Ban, Check, Pipette } from 'lucide-react'
import { COLOR_PRESETS, normalizeHexColor } from '../../lib/colorPresets'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'

const swatchClass =
  'relative flex h-5 w-5 shrink-0 cursor-pointer items-center justify-center rounded-full transition-transform hover:scale-110 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-accent'

// Shown on the custom swatch until it holds a colour of its own.
const RAINBOW = 'conic-gradient(#ef4444, #f59e0b, #22c55e, #14b8a6, #3b82f6, #8b5cf6, #ec4899, #ef4444)'

/**
 * One-click preset swatches, plus a custom swatch that opens the system colour
 * picker for any other colour. `value` is `#rrggbb`, or null for none.
 *
 * `onChange` gets the colour once it is chosen: a preset click, or the system
 * picker closing on a colour. `onPreview`, when given, also follows the picker
 * while it is being dragged, for choices that are cheap to apply live.
 */
export function ColorSwatchPicker({
  value,
  onChange,
  onPreview,
  label,
  noneLabel,
}: {
  value: string | null
  onChange: (color: string | null) => void
  onPreview?: (color: string) => void
  label: string
  /** Offers a "no colour" swatch, labelled with this, that picks null. */
  noneLabel?: string
}) {
  const { t } = useTranslation()
  const current = normalizeHexColor(value)
  const isPreset = COLOR_PRESETS.some((preset) => preset.hex === current)
  // The colour the system picker is on while it is open, before it commits.
  const [draft, setDraft] = useState<string | null>(null)
  const customColor = draft ?? (current && !isPreset ? current : null)
  const customSelected = !!customColor

  const inputRef = useRef<HTMLInputElement>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange

  useEffect(() => setDraft(null), [current])

  // React's onChange is the native `input` event, which fires on every drag
  // step; the native `change` event fires once, when the picker commits.
  useEffect(() => {
    const input = inputRef.current
    if (!input) return
    const commit = () => {
      const color = normalizeHexColor(input.value)
      if (color) onChangeRef.current(color)
    }
    input.addEventListener('change', commit)
    return () => input.removeEventListener('change', commit)
  }, [])

  const pick = (color: string | null) => {
    if (color !== current) onChange(color)
  }

  return (
    <div role="radiogroup" aria-label={label} className="flex flex-wrap items-center gap-1.5">
      {noneLabel !== undefined && (
        <button
          type="button"
          role="radio"
          aria-checked={current === null && !draft}
          aria-label={noneLabel}
          title={noneLabel}
          onClick={() => pick(null)}
          className={clsx(swatchClass, 'text-secondary', current === null && !draft && 'ring-2 ring-accent')}
        >
          <Ban size={16} />
        </button>
      )}
      {COLOR_PRESETS.map(({ id, hex }) => {
        const name = t(`colors.${id}`)
        const selected = current === hex && !draft
        return (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={name}
            title={name}
            onClick={() => pick(hex)}
            className={clsx(swatchClass, 'text-white', selected && 'ring-2 ring-primary/40')}
            style={{ backgroundColor: hex }}
          >
            {selected && <Check size={12} strokeWidth={3} />}
          </button>
        )
      })}
      <label
        title={t('colors.custom')}
        className={clsx(swatchClass, 'text-white', customSelected && 'ring-2 ring-primary/40')}
        style={{ background: customColor ?? RAINBOW }}
      >
        {customSelected ? <Check size={12} strokeWidth={3} /> : <Pipette size={11} strokeWidth={2.6} />}
        <input
          ref={inputRef}
          type="color"
          role="radio"
          aria-checked={customSelected}
          aria-label={t('colors.custom')}
          value={customColor ?? current ?? COLOR_PRESETS[0].hex}
          onChange={(event) => {
            const color = normalizeHexColor(event.target.value)
            if (!color) return
            setDraft(color)
            onPreview?.(color)
          }}
          // Closing the picker without choosing leaves no `change` to clear
          // the draft, so it is dropped once the picker lets go of focus.
          onBlur={() => setDraft(null)}
          className="sr-only"
        />
      </label>
    </div>
  )
}
