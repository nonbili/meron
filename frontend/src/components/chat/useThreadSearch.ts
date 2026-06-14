import { useCallback, useEffect, useMemo } from 'react'
import { useValue } from '@legendapp/state/react'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import { messageSearchText } from './messageHelpers'

// In-thread find: matches the current query against the loaded messages and
// exposes prev/next navigation. The matching list and active index live here so
// the desktop header search, the mobile search bar, and the message list all
// read the same source. Callers handle scrolling the active match into view.
export function useThreadSearch(messages: Message[]) {
  const threadSearch = useValue(thread$.search)
  const threadSearchOpen = useValue(thread$.searchOpen)
  const activeSearchIndex = useValue(thread$.activeSearchIndex)

  const normalizedThreadSearch = threadSearch.trim().toLowerCase()
  const searchMatches = useMemo(() => {
    if (!normalizedThreadSearch) return []
    return messages
      .filter((message) => messageSearchText(message).includes(normalizedThreadSearch))
      .map((message) => message.id)
  }, [messages, normalizedThreadSearch])
  const activeSearchId = searchMatches[activeSearchIndex] ?? ''

  // Publish the active match so each MessageBubble can decide whether it's the
  // highlighted one without needing it as a prop.
  useEffect(() => {
    thread$.activeSearchId.set(activeSearchId)
  }, [activeSearchId])

  const goToSearchMatch = useCallback(
    (direction: -1 | 1) => {
      if (searchMatches.length === 0) return
      const next = activeSearchIndex + direction
      const wrapped = next < 0 ? searchMatches.length - 1 : next >= searchMatches.length ? 0 : next
      thread$.activeSearchIndex.set(wrapped)
    },
    [searchMatches.length, activeSearchIndex],
  )

  useEffect(() => {
    thread$.activeSearchIndex.set(0)
  }, [normalizedThreadSearch])

  useEffect(() => {
    if (activeSearchIndex >= searchMatches.length) {
      thread$.activeSearchIndex.set(0)
    }
  }, [activeSearchIndex, searchMatches.length])

  return {
    threadSearch,
    threadSearchOpen,
    normalizedThreadSearch,
    searchMatches,
    activeSearchIndex,
    activeSearchId,
    goToSearchMatch,
  }
}
