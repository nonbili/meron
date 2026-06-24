import type { RefObject } from 'react'
import { ChevronDown, ChevronUp, Search, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { thread$ } from '../../states/thread'

// The narrow-viewport in-thread search bar (the desktop equivalent lives inside
// ConversationHeader). Rendered only while thread search is open.
export function ThreadSearchBarMobile({
  searchMatches,
  activeSearchIndex,
  goToSearchMatch,
  inputRef,
}: {
  searchMatches: string[]
  activeSearchIndex: number
  goToSearchMatch: (direction: -1 | 1) => void
  inputRef: RefObject<HTMLInputElement | null>
}) {
  const { t } = useTranslation()
  const threadSearch = useValue(thread$.search)
  const normalizedThreadSearch = threadSearch.trim().toLowerCase()

  return (
    <div className="min-[900px]:hidden flex h-11 shrink-0 items-center gap-1 border-b border-border bg-header px-3 z-10">
      <div className="flex flex-1 items-center gap-2 rounded-xl bg-hover px-2 py-1.5 border border-transparent focus-within:border-accent/40 focus-within:bg-chats">
        <Search size={14} className="text-secondary shrink-0" />
        <input
          ref={inputRef}
          value={threadSearch}
          onChange={(event) => thread$.search.set(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              goToSearchMatch(event.shiftKey ? -1 : 1)
            }
            if (event.key === 'Escape') {
              thread$.search.set('')
              thread$.searchOpen.set(false)
            }
          }}
          placeholder={t('chat.searchThread')}
          className="min-w-0 flex-1 bg-transparent text-xs text-primary placeholder-secondary border-none outline-none"
        />
        <button
          onClick={() => {
            thread$.search.set('')
            thread$.searchOpen.set(false)
          }}
          className="flex h-5 w-5 items-center justify-center rounded-full text-secondary hover:text-primary cursor-pointer"
          title={t('chat.closeThreadSearch')}
        >
          <X size={12} />
        </button>
        <span className="w-12 text-center text-[10px] font-semibold text-secondary">
          {normalizedThreadSearch ? `${searchMatches.length ? activeSearchIndex + 1 : 0}/${searchMatches.length}` : ''}
        </span>
      </div>
      <button
        onClick={() => goToSearchMatch(-1)}
        disabled={searchMatches.length === 0}
        className="flex h-8 w-8 items-center justify-center rounded-lg text-secondary hover:bg-hover disabled:opacity-35 disabled:cursor-not-allowed cursor-pointer"
        title={t('chat.previousMatch')}
      >
        <ChevronUp size={15} />
      </button>
      <button
        onClick={() => goToSearchMatch(1)}
        disabled={searchMatches.length === 0}
        className="flex h-8 w-8 items-center justify-center rounded-lg text-secondary hover:bg-hover disabled:opacity-35 disabled:cursor-not-allowed cursor-pointer"
        title={t('chat.nextMatch')}
      >
        <ChevronDown size={15} />
      </button>
    </div>
  )
}
