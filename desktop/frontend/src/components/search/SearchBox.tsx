import { useRef } from 'react'
import type { KeyboardEvent, ReactNode, RefObject } from 'react'
import { Search, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { SearchOptions, type SearchOptionsProps } from './SearchOptions'

type SearchBoxProps = {
  value: string
  onChange: (value: string) => void
  placeholder: string
  /** The title bar's box: its colors until focused, sized to fit its row. */
  compact?: boolean
  /** Sizing within the parent row. */
  className?: string
  inputRef?: RefObject<HTMLInputElement | null>
  boxRef?: RefObject<HTMLDivElement | null>
  /** Shown between the search icon and the text, e.g. the column a board search is scoped to. */
  chip?: ReactNode
  /** The search options form; left out, the box has no options button. */
  options?: Omit<SearchOptionsProps, 'query' | 'anchorRef' | 'onDismiss' | 'compact'>
  /** Runs before the box's own keys; `preventDefault` skips them. */
  onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void
  /** Escape on an empty box; it blurs by default. */
  onEscapeEmpty?: () => void
  /** Focus entering or leaving the box, its options form included. */
  onFocusChange?: (focused: boolean) => void
}

/**
 * The search box of the thread list and of a kanban board. It sits in the
 * title bar when Meron draws one, and in the list's or board's header otherwise.
 */
export function SearchBox({
  value,
  onChange,
  placeholder,
  compact = false,
  className,
  inputRef: inputRefProp,
  boxRef: boxRefProp,
  chip,
  options,
  onKeyDown,
  onEscapeEmpty,
  onFocusChange,
}: SearchBoxProps) {
  const { t } = useTranslation()
  const ownInputRef = useRef<HTMLInputElement | null>(null)
  const ownBoxRef = useRef<HTMLDivElement | null>(null)
  const inputRef = inputRefProp ?? ownInputRef
  const boxRef = boxRefProp ?? ownBoxRef
  const iconSize = 14
  return (
    <div
      ref={boxRef}
      onFocus={() => onFocusChange?.(true)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) onFocusChange?.(false)
      }}
      className={clsx(
        // Laid out in a row rather than with absolutely centred icons: a
        // transform there would trap the options form's z-index under the
        // board header when the box sits in the title bar.
        'group relative flex min-w-0 items-center gap-2 border border-transparent transition-colors focus-within:bg-chats',
        // Dark mode's white ink stands out more on the near-black title bar than
        // light's dark ink on grey, so its fill is lighter to blend in as much.
        compact
          ? 'h-full rounded-lg bg-sidenav-ink/4 px-2.5 hover:not-focus-within:bg-sidenav-ink/8 focus-within:border-accent dark:bg-sidenav-ink/[0.025] dark:hover:not-focus-within:bg-sidenav-ink/5'
          : 'h-9 rounded-xl bg-hover px-3 focus-within:border-accent/40',
        className,
      )}
    >
      <Search
        size={iconSize}
        className={clsx(
          'shrink-0',
          compact ? 'text-sidenav-ink/60 group-focus-within:text-secondary' : 'text-secondary',
        )}
      />
      {chip}
      <input
        ref={inputRef}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          onKeyDown?.(event)
          if (event.defaultPrevented || event.key !== 'Escape') return
          if (value) onChange('')
          else if (onEscapeEmpty) onEscapeEmpty()
          else event.currentTarget.blur()
        }}
        placeholder={placeholder}
        className={clsx(
          'h-full min-w-0 flex-1 appearance-none border-0 bg-transparent p-0 outline-none',
          compact
            ? 'text-xs text-sidenav-ink placeholder-sidenav-ink/60 group-focus-within:text-primary group-focus-within:placeholder-secondary'
            : 'text-[0.8125rem] text-primary placeholder-secondary',
        )}
      />
      {options && (
        <SearchOptions
          {...options}
          query={value}
          anchorRef={boxRef}
          onDismiss={() => inputRef.current?.focus()}
          compact={compact}
        />
      )}
      {value && (
        <button
          type="button"
          onClick={() => {
            onChange('')
            inputRef.current?.focus()
          }}
          aria-label={t('common.clearSearch')}
          title={t('common.clearSearch')}
          className={clsx(
            'flex shrink-0 cursor-pointer transition-colors',
            compact
              ? 'text-sidenav-ink/60 hover:text-sidenav-ink group-focus-within:text-secondary group-focus-within:hover:text-primary'
              : 'text-secondary hover:text-primary',
          )}
        >
          <X size={iconSize} />
        </button>
      )}
    </div>
  )
}
