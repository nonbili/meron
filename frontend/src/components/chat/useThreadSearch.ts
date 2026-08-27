import { useCallback, useEffect, useMemo } from 'react'
import { useValue } from '@legendapp/state/react'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import { messageSearchText } from './messageHelpers'
import { messageMatchCount } from './threadSearchMatches'
import { useConversationMode } from './useMessageView'

/** One highlighted occurrence: the message it sits in, and which of that
 *  message's occurrences it is (-1 for a message that matches only in its
 *  subject or sender, which has no <mark> of its own to step to). */
type SearchOccurrence = { id: string; offset: number }

// In-thread find: matches the current query against the loaded messages and
// exposes prev/next navigation. The matching list and active index live here so
// the desktop header search, the mobile search bar, and the message list all
// read the same source. Navigation is per occurrence, not per message — a
// message holding three hits is three stops, and the counter says so. Callers
// handle scrolling the active match into view.
export function useThreadSearch(messages: Message[]) {
  const threadSearch = useValue(thread$.search)
  const threadSearchOpen = useValue(thread$.searchOpen)
  const activeSearchIndex = useValue(thread$.activeSearchIndex)
  const conversationMode = useConversationMode()

  const normalizedThreadSearch = threadSearch.trim().toLowerCase()
  const occurrences = useMemo(() => {
    if (!normalizedThreadSearch) return [] as SearchOccurrence[]
    const list: SearchOccurrence[] = []
    for (const message of messages) {
      const useHtmlBody = conversationMode === 'html' && !!message.body_html
      const count = messageMatchCount(message, normalizedThreadSearch, useHtmlBody)
      if (count === 0) {
        // Still a match when the query is in the subject or the sender — it just
        // has nothing to highlight, so the whole message is the one stop.
        if (messageSearchText(message).includes(normalizedThreadSearch)) list.push({ id: message.id, offset: -1 })
        continue
      }
      for (let offset = 0; offset < count; offset += 1) list.push({ id: message.id, offset })
    }
    return list
  }, [messages, normalizedThreadSearch, conversationMode])

  // Message ids in match order, for the list's per-message affordances.
  const searchMatches = useMemo(() => [...new Set(occurrences.map((match) => match.id))], [occurrences])
  const matchCount = occurrences.length
  const active = occurrences[activeSearchIndex]
  const activeSearchId = active?.id ?? ''
  const activeSearchOffset = active?.offset ?? -1

  // Publish the active match so each MessageBubble can decide whether it's the
  // highlighted one without needing it as a prop. The bodies scroll to their own
  // active <mark> once this lands, so nothing here depends on the DOM having
  // caught up with the index this render derived.
  useEffect(() => {
    thread$.activeSearchId.set(activeSearchId)
    thread$.activeSearchOffset.set(activeSearchOffset)
  }, [activeSearchId, activeSearchOffset])

  const goToSearchMatch = useCallback(
    (direction: -1 | 1) => {
      if (matchCount === 0) return
      const next = activeSearchIndex + direction
      const wrapped = next < 0 ? matchCount - 1 : next >= matchCount ? 0 : next
      thread$.activeSearchIndex.set(wrapped)
    },
    [matchCount, activeSearchIndex],
  )

  useEffect(() => {
    thread$.activeSearchIndex.set(0)
  }, [normalizedThreadSearch])

  useEffect(() => {
    if (activeSearchIndex >= matchCount) {
      thread$.activeSearchIndex.set(0)
    }
  }, [activeSearchIndex, matchCount])

  return {
    threadSearch,
    threadSearchOpen,
    normalizedThreadSearch,
    searchMatches,
    matchCount,
    activeSearchIndex,
    activeSearchId,
    activeSearchOffset,
    goToSearchMatch,
  }
}
