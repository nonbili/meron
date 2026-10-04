import { useEffect, useRef } from 'react'
import { Search, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { isRssAccount } from '../../lib/threadActions'
import { clsx } from '../../lib/utils'
import { accounts$ } from '../../states/accounts'
import { ui$ } from '../../states/ui'

type ThreadSearchInputProps = {
  /** The title bar's smaller box, sized to fit its row. */
  compact?: boolean
  onFocusChange?: (focused: boolean) => void
}

/**
 * The thread list's search box. It sits in the title bar when Meron draws one,
 * and in the thread list's header otherwise.
 */
export function ThreadSearchInput({ compact = false, onFocusChange }: ThreadSearchInputProps) {
  const { t } = useTranslation()
  const query = useValue(ui$.query)
  const selectedAccount = useValue(ui$.selectedAccount)
  const accounts = useValue(accounts$)
  const rss = isRssAccount(
    accounts.find((account) => account.id === selectedAccount),
    selectedAccount,
  )
  // Focus the search box when ⌘/Ctrl+Shift+F (or the palette) bumps the signal.
  const inputRef = useRef<HTMLInputElement | null>(null)
  const globalSearchFocus = useValue(ui$.globalSearchFocus)
  useEffect(() => {
    if (globalSearchFocus === 0) return
    const input = inputRef.current
    input?.focus()
    input?.select()
  }, [globalSearchFocus])

  const iconSize = compact ? 14 : 15
  return (
    <div className={clsx('relative min-w-0 flex-1', compact && 'h-full')}>
      <input
        ref={inputRef}
        value={query}
        onChange={(event) => ui$.query.set(event.target.value)}
        onFocus={() => onFocusChange?.(true)}
        onBlur={() => onFocusChange?.(false)}
        onKeyDown={(event) => {
          if (event.key !== 'Escape') return
          ui$.query.set('')
          event.currentTarget.blur()
        }}
        placeholder={rss ? t('threads.searchFeeds') : t('threads.searchMessages')}
        className={clsx(
          'peer block w-full appearance-none border transition-all duration-150 focus:border-transparent focus:ring-1 focus:ring-accent',
          compact
            ? 'h-full rounded-lg border-transparent bg-sidenav-ink/4 py-0 pl-8 hover:not-focus:bg-sidenav-ink/8 text-xs text-sidenav-ink placeholder-sidenav-ink/50 focus:bg-chats focus:text-primary focus:placeholder-secondary'
            : 'rounded-xl border-transparent bg-hover py-2 pl-8 text-[0.8125rem] text-primary placeholder-secondary focus:bg-chats',
          // The right padding only has to clear the clear button while there is one.
          query ? 'pr-8' : 'pr-3',
        )}
      />
      {/* After the input so they can follow its focus (peer-focus), when the
        compact box turns from the title bar's colors to the list's. */}
      <Search
        className={clsx(
          'absolute top-1/2 -translate-y-1/2',
          compact ? 'left-2.5 text-sidenav-ink/50 peer-focus:text-secondary' : 'left-2.5 text-secondary',
        )}
        size={iconSize}
      />
      {query && (
        <button
          onClick={() => ui$.query.set('')}
          aria-label={t('common.clearSearch')}
          title={t('common.clearSearch')}
          className={clsx(
            'absolute top-1/2 -translate-y-1/2 cursor-pointer',
            compact
              ? 'right-2.5 text-sidenav-ink/60 hover:text-sidenav-ink peer-focus:text-secondary peer-focus:hover:text-primary'
              : 'right-2.5 text-secondary hover:text-primary',
          )}
        >
          <X size={iconSize} />
        </button>
      )}
    </div>
  )
}
