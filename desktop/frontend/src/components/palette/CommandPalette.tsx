import { useEffect, useMemo, useRef } from 'react'
import { Check, SquareChevronRight } from 'lucide-react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { Dialog } from '../dialog/Dialog'
import { ui$, closeCommandPalette } from '../../states/ui'
import { formatShortcut, isMac } from '../../lib/shortcuts'
import { useCommandList } from './useCommandList'
import { filterCommands } from './paletteCommands'

export function CommandPalette() {
  const { t } = useTranslation()
  const open = useValue(ui$.paletteOpen)
  const query = useValue(ui$.paletteQuery)
  const index = useValue(ui$.paletteIndex)
  const inputRef = useRef<HTMLInputElement | null>(null)

  const commands = useCommandList()
  const filtered = useMemo(() => filterCommands(commands, query), [commands, query])

  // Keep the highlighted index in range as the filtered list shrinks.
  useEffect(() => {
    if (!open) return
    const max = Math.max(0, filtered.length - 1)
    if (index > max) ui$.paletteIndex.set(max)
  }, [open, filtered.length, index])

  if (!open) return null

  const move = (delta: number) => {
    if (filtered.length === 0) return
    ui$.paletteIndex.set((index + delta + filtered.length) % filtered.length)
  }

  const onKeyDown = (event: ReactKeyboardEvent) => {
    const key = event.key.toLowerCase()

    if (event.key === 'ArrowDown' || (event.ctrlKey && key === 'n')) {
      event.preventDefault()
      event.stopPropagation()
      move(1)
    } else if (event.key === 'ArrowUp' || (event.ctrlKey && key === 'p')) {
      event.preventDefault()
      event.stopPropagation()
      move(-1)
    } else if (event.key === 'Enter') {
      event.preventDefault()
      event.stopPropagation()
      filtered[index]?.run()
    }
  }

  // Above every other overlay: Ctrl/⌘K opens the palette even with a dialog
  // already up, and it is then the newest layer — so it has to paint on top as
  // well as be the layer Escape closes (see useEscapeKey).
  return (
    <Dialog
      onClose={closeCommandPalette}
      initialFocusRef={inputRef}
      backdropClassName="z-[130] items-start pt-[12vh] dark:bg-black/40 backdrop-blur-sm"
      className="max-w-[560px] overflow-hidden rounded-xl"
      aria-label={t('palette.label')}
      onKeyDown={onKeyDown}
    >
      <div className="flex items-center gap-2.5 border-b border-border px-4 py-3">
        <SquareChevronRight size={16} className="shrink-0 text-secondary" />
        <input
          ref={inputRef}
          value={query}
          onChange={(event) => {
            ui$.paletteQuery.set(event.target.value)
            ui$.paletteIndex.set(0)
          }}
          placeholder={t('palette.placeholder')}
          className="w-full border-0 bg-transparent text-sm text-primary outline-none placeholder:text-secondary"
        />
      </div>

      <div className="max-h-[min(50vh,380px)] overflow-y-auto p-2">
        {filtered.length === 0 ? (
          <div className="px-3 py-6 text-center text-xs text-secondary">{t('palette.noMatches')}</div>
        ) : (
          filtered.map((command, i) => {
            const selected = i === index
            const shortcut = command.shortcut ? formatShortcut(command.shortcut) : []
            return (
              <button
                key={command.id}
                onMouseEnter={() => ui$.paletteIndex.set(i)}
                onClick={() => command.run()}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-[0.8125rem] transition-colors cursor-pointer ${
                  selected ? 'bg-accent/10 text-primary' : 'text-primary hover:bg-hover'
                }`}
              >
                <span className="shrink-0 text-secondary">{command.icon}</span>
                <span className="flex-1 truncate">{command.label}</span>
                {command.active && <Check size={14} className="shrink-0 text-accent" />}
                {shortcut.length > 0 && (
                  <kbd className="shrink-0 rounded border border-border bg-app px-1.5 py-0.5 text-[0.625rem] font-medium text-secondary">
                    {shortcut.join(isMac ? '' : '+')}
                  </kbd>
                )}
              </button>
            )
          })
        )}
      </div>

      <div className="flex items-center gap-4 border-t border-border px-4 py-2 text-[0.625rem] text-secondary">
        <Hint keys="Enter" label={t('palette.hints.run')} />
        <Hint keys="↑↓" label={t('palette.hints.navigate')} />
        <Hint keys="Esc" label={t('palette.hints.close')} />
      </div>
    </Dialog>
  )
}

function Hint({ keys, label }: { keys: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <kbd className="rounded border border-border bg-app px-1.5 py-0.5 font-medium">{keys}</kbd>
      <span className="uppercase tracking-wide">{label}</span>
    </span>
  )
}
