import { observable } from '@legendapp/state'
import { mail$ } from './mailState'
import { ui$ } from './ui'
import type { Message } from '../types'

export interface ThreadData {
  messages: Message[]
  cursor: string
  loading: boolean
  loadingMore: boolean
  error: boolean
}

// Each mounted conversation owns a page independently of the active-thread
// interface used by actions and the single quick-reply composer.
export const threadData$ = observable({} as Record<string, ThreadData>)
export function ensureThreadData(threadId: string) {
  if (!threadData$[threadId].peek()) {
    threadData$[threadId].set({
      messages: mail$.messages.peek().filter((message) => message.thread_id === threadId),
      cursor: '',
      loading: false,
      loadingMore: false,
      error: false,
    })
  }
  return threadData$[threadId]
}

// Actions update the active message array, including optimistic sends, draft
// hydration and read/star flags. Mirror those edits without retargeting them
// when selection changes before the next page arrives.
let previousThreads = new Set<string>()
mail$.messages.onChange(({ value }) => {
  const ids = new Set(value.map((message) => message.thread_id))
  for (const id of new Set([...previousThreads, ...ids])) {
    if ((ids.has(id) || (ids.size === 0 && ui$.selectedThread.peek() === id)) && threadData$[id].peek())
      threadData$[id].messages.set(value.filter((message) => message.thread_id === id))
  }
  previousThreads = ids
})

const readers = new Map<string, number>()
export const threadRequests = new Map<string, object>()
export function retainThreadData(threadId: string) {
  readers.set(threadId, (readers.get(threadId) ?? 0) + 1)
  return () => {
    readers.set(threadId, (readers.get(threadId) ?? 1) - 1)
    // A pane moving between branches can acquire the same thread this commit.
    queueMicrotask(() => {
      if (readers.get(threadId) !== 0) return
      readers.delete(threadId)
      threadRequests.delete(threadId)
      threadData$[threadId].delete()
    })
  }
}
