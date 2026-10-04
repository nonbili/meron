import { useEffect, useMemo, useRef, useState } from 'react'
import { Search, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { getKanbanColumns, kanban$, kanbanBoardColumnKey, kanbanColumnKey } from '../../states/kanban'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { IconButton } from '../button/IconButton'
import { SearchScopeDropdown } from './SearchScopeDropdown'

/**
 * A kanban board's search box and its column scope. It sits in the title bar
 * when Meron draws one, always open there; in the board's header otherwise,
 * collapsed to an icon until it is wanted.
 */
export function KanbanSearch({ boardId, compact = false }: { boardId: string; compact?: boolean }) {
  const { t } = useTranslation()
  const boards = useValue(settings$.kanbanBoards)
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)
  const globalSearchFocus = useValue(ui$.globalSearchFocus)
  const visibleColumns = useMemo(() => getKanbanColumns(boardId), [boards, boardId])
  // The header's bar is collapsed to an icon by default; it expands on click (or
  // the search hotkey) and folds back once it's empty and loses focus. Start open
  // if a query is already active so a persisted search stays visible.
  const [expanded, setExpanded] = useState(() => !!kanban$.searchQuery.peek().trim())
  const open = compact || expanded
  const inputRef = useRef<HTMLInputElement | null>(null)
  const barRef = useRef<HTMLDivElement | null>(null)

  // Bumped by the search hotkey, the palette and a column's "search this column".
  useEffect(() => {
    if (globalSearchFocus === 0) return
    setExpanded(true)
    requestAnimationFrame(() => {
      inputRef.current?.focus()
      inputRef.current?.select()
    })
  }, [globalSearchFocus])

  // Focus the input whenever the header's bar expands so it's immediately typeable.
  useEffect(() => {
    if (expanded && !compact) inputRef.current?.focus()
  }, [expanded, compact])

  // Collapse the header's bar when the user clicks away, but only if it's empty —
  // an active query keeps the bar (and its results) visible.
  useEffect(() => {
    if (!expanded || compact) return
    const onPointerDown = (event: MouseEvent) => {
      if (barRef.current?.contains(event.target as Node)) return
      if (kanban$.searchQuery.peek().trim()) return
      setExpanded(false)
    }
    document.addEventListener('mousedown', onPointerDown)
    return () => document.removeEventListener('mousedown', onPointerDown)
  }, [expanded, compact])

  if (!open) {
    return <IconButton icon={Search} label={t('kanban.searchBoardAction')} onClick={() => setExpanded(true)} />
  }

  const iconSize = 14
  return (
    <div
      ref={barRef}
      className={clsx(
        'group flex min-w-0 items-center overflow-visible border focus-within:bg-chats',
        compact
          ? 'h-full w-full rounded-lg border-transparent bg-sidenav-ink/4 hover:not-focus-within:bg-sidenav-ink/8 focus-within:border-accent'
          : 'h-9 basis-72 shrink rounded-xl border-transparent bg-hover focus-within:border-accent/40',
      )}
    >
      <div className="relative h-full min-w-0 flex-1">
        <Search
          className={clsx(
            'absolute top-1/2 -translate-y-1/2',
            compact ? 'left-2.5 text-sidenav-ink/50 group-focus-within:text-secondary' : 'left-3 text-secondary',
          )}
          size={iconSize}
        />
        <input
          ref={inputRef}
          value={searchQuery}
          onChange={(event) => kanban$.searchQuery.set(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== 'Escape') return
            if (searchQuery) kanban$.searchQuery.set('')
            else if (compact) event.currentTarget.blur()
            else setExpanded(false)
          }}
          placeholder={t('kanban.searchBoard')}
          className={clsx(
            'block h-full w-full appearance-none border-0 bg-transparent pr-8 text-xs outline-none transition-all',
            compact
              ? 'py-0 pl-8 text-sidenav-ink placeholder-sidenav-ink/50 group-focus-within:text-primary group-focus-within:placeholder-secondary'
              : 'py-1.5 pl-9.5 text-primary placeholder-secondary',
          )}
        />
        {searchQuery && (
          <button
            onClick={() => {
              kanban$.searchQuery.set('')
              inputRef.current?.focus()
            }}
            className={clsx(
              'absolute top-1/2 -translate-y-1/2 cursor-pointer transition-colors',
              compact
                ? 'right-2.5 text-sidenav-ink/60 hover:text-sidenav-ink group-focus-within:text-secondary group-focus-within:hover:text-primary'
                : 'right-2.5 text-secondary hover:text-primary',
            )}
            aria-label={t('common.clearSearch')}
            title={t('common.clearSearch')}
          >
            <X size={iconSize} />
          </button>
        )}
      </div>
      <SearchScopeDropdown
        value={searchScope}
        onChange={(scope) => {
          kanban$.searchScope.set(scope)
          expandSearchScope(boardId, scope)
          inputRef.current?.focus()
        }}
        visibleColumns={visibleColumns}
        compact={compact}
      />
    </div>
  )
}

// Matches are drawn in the column body, so scoping a search to a collapsed
// column would hide the very results it just loaded behind the strip.
export function expandSearchScope(boardId: string, scope: string) {
  const column = getKanbanColumns(boardId).find((item) => kanbanColumnKey(item) === scope)
  if (!column) return
  settings$.kanbanMinimizedColumns[kanbanBoardColumnKey(boardId, column)].set(false)
}
