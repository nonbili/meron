import { useEffect, useLayoutEffect, useMemo } from 'react'
import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import { resolveConversationMode, thread$ } from '../../states/thread'
import type { Message } from '../../types'
import { messageSearchText } from './messageHelpers'
import { messageMatchCount } from './threadSearchMatches'

/** One highlighted occurrence: the message it sits in, and which of that
 *  message's occurrences it is (-1 for a message that matches only in its
 *  subject or sender, which has no <mark> of its own to step to). */
type SearchOccurrence = { id: string; offset: number }

// In-thread find: matches the current query against the loaded messages and
// publishes the count and active match to thread$, where the desktop header
// search, the mobile search bar, and the message bodies all read them.
// Navigation is per occurrence, not per message — a message holding three hits
// is three stops, and the counter says so. Callers handle scrolling the active
// match into view. `accountId` is the thread's account, the fallback for a
// message that carries none of its own.
export function useThreadSearch(messages: Message[], accountId?: string) {
  const threadSearch = useValue(thread$.search)
  const threadSearchOpen = useValue(thread$.searchOpen)
  const activeSearchIndex = useValue(thread$.activeSearchIndex)
  const accounts = useValue(accounts$)
  const modeOverrides = useValue(thread$.conversationModeOverrides)

  const normalizedThreadSearch = threadSearch.trim().toLowerCase()
  const occurrences = useMemo(() => {
    if (!normalizedThreadSearch) return [] as SearchOccurrence[]
    const list: SearchOccurrence[] = []
    for (const message of messages) {
      // Resolved per message, exactly as useMessageView renders it, so the
      // counter can't promise a stop the body doesn't mark.
      const mode = resolveConversationMode(accounts, modeOverrides, message.account_id || accountId)
      const useHtmlBody = mode === 'html' && !!message.body_html
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
  }, [messages, normalizedThreadSearch, accounts, modeOverrides, accountId])

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

  // The search bars read the count from the store; a layout effect so their
  // counter never paints a frame behind the query.
  useLayoutEffect(() => {
    thread$.searchMatchCount.set(matchCount)
  }, [matchCount])

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
  }
}
