import { useId, useLayoutEffect, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { clsx } from '../../lib/utils'
import { menuItemBase } from '../menu/menuStyles'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { TextInput, type TextInputProps } from './Field'

/** The picker's step, in minutes. Typing still reaches any minute. */
const STEP = 30
const SLOTS = Array.from({ length: (24 * 60) / STEP }, (_, index) => {
  const minutes = index * STEP
  return `${`${Math.floor(minutes / 60)}`.padStart(2, '0')}:${`${minutes % 60}`.padStart(2, '0')}`
})
const LIST_HEIGHT = 240

/**
 * Marks the slot list, which renders in a portal outside its input: a surface that closes
 * on presses outside itself should count a press here as its own.
 */
export const TIME_SLOTS_ATTRIBUTE = 'data-time-slots'

function slotLabel(slot: string): string {
  const [hours, minutes] = slot.split(':').map(Number)
  return new Date(2000, 0, 1, hours, minutes).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
}

/** The slot at or just before a time, or the first for no time. */
function slotIndex(value: string): number {
  return Math.max(SLOTS.filter((slot) => slot <= value).length - 1, 0)
}

export interface TimeInputProps extends Omit<TextInputProps, 'type' | 'value' | 'onChange'> {
  value: string
  onChange: (value: string) => void
  /** A bare input that takes its look from `className` alone. */
  unstyled?: boolean
}

/**
 * `<input type="time">` with a list of half-hour slots under it, in place of
 * the native picker: WebKitGTK has none, leaving typing as the only way in.
 * A click opens the list and leaves focus in the input for typing. Alt+Down
 * opens it with focus in the list, where Up/Down move and Enter picks.
 */
export function TimeInput({ value, onChange, unstyled, className, onClick, onKeyDown, ...rest }: TimeInputProps) {
  const id = useId()
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState<CSSProperties | null>(null)
  const [focusList, setFocusList] = useState(false)
  const [active, setActive] = useState(0)
  const [container, setContainer] = useState<HTMLElement>(() => document.body)
  const isOpen = !!position
  const optionId = (index: number) => `${id}-slot-${index}`

  const close = () => {
    // Hand focus back from the list rather than dropping it on the body.
    if (list.current?.contains(document.activeElement)) input.current?.focus({ preventScroll: true })
    setPosition(null)
  }

  useDismissOnOutside(
    isOpen,
    (target) => target instanceof Node && (!!input.current?.contains(target) || !!list.current?.contains(target)),
    close,
  )

  const open = (withFocus: boolean) => {
    const rect = input.current?.getBoundingClientRect()
    if (!rect) return
    const below = window.innerHeight - rect.bottom - 8
    const top = below >= Math.min(LIST_HEIGHT, rect.top - 8) ? rect.bottom + 4 : undefined
    setActive(slotIndex(value))
    setFocusList(withFocus)
    // Inside a modal, so assistive tech that keeps to it still finds the list.
    // The dialog's backdrop spans the viewport, so fixed coordinates still hold.
    setContainer(input.current?.closest<HTMLElement>('[data-dialog-surface]') ?? document.body)
    const width = Math.max(rect.width, 112)
    setPosition({
      left: Math.max(8, Math.min(rect.left, window.innerWidth - width - 8)),
      minWidth: width,
      maxHeight: Math.min(LIST_HEIGHT, Math.max(below, rect.top - 8)),
      ...(top === undefined ? { bottom: window.innerHeight - rect.top + 4 } : { top }),
    })
  }

  const pick = (slot: string) => {
    onChange(slot)
    close()
  }

  // On opening, centre the current time's slot and take focus if asked.
  useLayoutEffect(() => {
    if (!isOpen || !list.current) return
    const row = list.current.children[slotIndex(value)] as HTMLElement | undefined
    if (row) list.current.scrollTop = row.offsetTop - list.current.clientHeight / 2 + row.offsetHeight / 2
    if (focusList) list.current.focus()
  }, [isOpen])

  // Keep the highlighted slot in view as the keyboard moves it.
  useLayoutEffect(() => {
    if (!isOpen) return
    const row = list.current?.children[active] as HTMLElement | undefined
    row?.scrollIntoView?.({ block: 'nearest' })
  }, [isOpen, active])

  const shared = {
    ...rest,
    ref: input,
    type: 'time',
    value,
    // The slot list stands in for Chromium's own picker (WebView2).
    className: clsx('[&::-webkit-calendar-picker-indicator]:hidden', className),
    'aria-haspopup': 'listbox' as const,
    'aria-expanded': isOpen,
    'aria-controls': isOpen ? `${id}-slots` : undefined,
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => onChange(event.target.value),
    onClick: (event: React.MouseEvent<HTMLInputElement>) => {
      onClick?.(event)
      if (!isOpen) open(false)
    },
    onKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => {
      onKeyDown?.(event)
      if (event.defaultPrevented) return
      if (event.altKey && event.key === 'ArrowDown') {
        event.preventDefault()
        if (isOpen) list.current?.focus()
        else open(true)
      } else if (isOpen && event.key === 'Enter') {
        // Enter answers the open list, not the form around the input, and picks
        // nothing: slots are picked by click or from the list itself. Typing
        // closes the list first, so Enter after a typed time still submits.
        event.preventDefault()
        event.stopPropagation()
        close()
      } else if (isOpen && event.key !== 'Escape') {
        // Typing takes over from the list.
        close()
      }
    },
  }

  return (
    <>
      {unstyled ? <input {...shared} /> : <TextInput {...shared} />}
      {position &&
        createPortal(
          <div
            ref={list}
            id={`${id}-slots`}
            role="listbox"
            tabIndex={-1}
            aria-label={rest['aria-label']}
            aria-activedescendant={optionId(active)}
            {...{ [TIME_SLOTS_ATTRIBUTE]: '' }}
            // A press here must not start dragging the row the input sits in.
            data-no-drag=""
            className="menu-surface fixed z-[60] overflow-y-auto outline-none"
            style={position}
            // Keep focus where it is, so the input's blur handlers don't see a pick as leaving.
            onMouseDown={(event) => event.preventDefault()}
            onKeyDown={(event) => {
              // With focus here, bare keys are meant for the list, not the app's
              // single-key shortcuts; chords such as Ctrl+Q still reach the app.
              // Escape never arrives: the window's Escape layers take it first.
              if (!event.ctrlKey && !event.metaKey && !event.altKey) event.stopPropagation()
              if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                event.preventDefault()
                const step = event.key === 'ArrowDown' ? 1 : -1
                setActive((current) => Math.min(Math.max(current + step, 0), SLOTS.length - 1))
              } else if (event.key === 'Home' || event.key === 'End') {
                event.preventDefault()
                setActive(event.key === 'Home' ? 0 : SLOTS.length - 1)
              } else if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                pick(SLOTS[active])
              } else if (event.key === 'Tab') {
                event.preventDefault()
                close()
              }
            }}
          >
            {SLOTS.map((slot, index) => (
              <div
                key={slot}
                id={optionId(index)}
                role="option"
                aria-selected={slot === value}
                className={clsx(
                  menuItemBase,
                  'hover:bg-hover',
                  index === active && 'bg-hover',
                  slot === value ? 'font-medium text-accent' : 'text-primary',
                )}
                onClick={() => pick(slot)}
              >
                {slotLabel(slot)}
              </div>
            ))}
          </div>,
          container,
        )}
    </>
  )
}
