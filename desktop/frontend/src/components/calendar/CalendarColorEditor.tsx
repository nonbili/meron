import { useRef, useState, type ReactNode } from 'react'
import { useTranslation } from '../../lib/i18n'
import { errorMessage } from '../../lib/errors'
import { clsx } from '../../lib/utils'
import { updateCalendar } from '../../states/caldav'
import { showToast } from '../../states/ui'
import { ColorSwatchPicker } from '../field/ColorSwatchPicker'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'

/** A synced calendar's colour dot, which opens its colours under the row it
 * sits in. Choosing one changes it on the server, or here when the server
 * will not take it. Escape or a click elsewhere closes the colours. */
export function CalendarColorEditor({
  calendarId,
  color,
  dimmed = false,
  className,
  children,
}: {
  calendarId: string
  color: string
  dimmed?: boolean
  className?: string
  /** The row, given the dot to place in it. */
  children: (dot: ReactNode) => ReactNode
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useDismissOnOutside(
    open,
    (target) => Boolean(ref.current?.contains(target as Node)),
    () => setOpen(false),
  )
  const label = t('settings.calendars.color')
  const dot = (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-expanded={open}
      onClick={() => setOpen(!open)}
      className="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-full hover:bg-active"
    >
      <span
        aria-hidden
        className={clsx('size-2.5 rounded-full transition-opacity', dimmed && 'opacity-40')}
        style={{ backgroundColor: color }}
      />
    </button>
  )
  return (
    <div ref={ref} className={clsx('rounded-lg', open && 'bg-hover/60', className)}>
      {children(dot)}
      {open && (
        <div className="px-2 pb-2 pt-1">
          <ColorSwatchPicker
            label={label}
            value={color}
            onChange={(next) => {
              if (!next) return
              setOpen(false)
              updateCalendar(calendarId, { color: next }).catch((err) =>
                showToast(errorMessage(err, t('settings.calendars.saveFailed')), 'error'),
              )
            }}
          />
        </div>
      )}
    </div>
  )
}
