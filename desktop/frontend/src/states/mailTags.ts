import { invoke } from '../lib/bridge'
import { columnReadsTags, loadKanbanColumn } from '../lib/kanbanData'
import { getKanbanColumns, kanban$, kanbanColumnKey } from './kanban'
import { mail$, updateKanbanThread } from './mail'
import { ui$ } from './ui'

export type TagCount = { tag: string; count: number }

export function openTagDialog(threadId: string) {
  ui$.tagThreadId.set(threadId)
}

// What a thread is tagged with right now, from whichever view holds it: a list
// row, or its open messages, any one of which may carry a given tag.
export function threadTags(threadId: string): string[] {
  const listed =
    mail$.threads.peek().find((thread) => thread.thread_id === threadId) ??
    Object.values(kanban$.threads.peek())
      .flat()
      .find((thread) => thread.thread_id === threadId)
  const messages = mail$.messages.peek().filter((message) => message.thread_id === threadId)
  return [...new Set([...(listed?.tags ?? []), ...messages.flatMap((message) => message.tags ?? [])])].sort()
}

// The core saves a tag without touching the network and rejects one it cannot
// store, so views update from its answer rather than optimistically.
export async function setThreadTags(threadId: string, add: string[], remove: string[]): Promise<string[]> {
  const { tags } = await invoke<{ tags: string[] }>('mail.setTags', { thread_id: threadId, add, remove })
  mail$.threads.set(mail$.threads.get().map((thread) => (thread.thread_id === threadId ? { ...thread, tags } : thread)))
  mail$.messages.set(
    mail$.messages.get().map((message) => (message.thread_id === threadId ? { ...message, tags } : message)),
  )
  updateKanbanThread(threadId, (thread) => ({ ...thread, tags }))
  // A tag column may gain or lose this thread, which no folder sync announces.
  const query = kanban$.searchQuery.peek().trim()
  const scope = kanban$.searchScope.peek()
  for (const column of getKanbanColumns(kanban$.activeBoardId.peek())) {
    if (!columnReadsTags(column)) continue
    void loadKanbanColumn(column, false, scope === 'all' || scope === kanbanColumnKey(column) ? query : '')
  }
  return tags
}

export async function loadTagCounts(): Promise<TagCount[]> {
  const { tags } = await invoke<{ tags: TagCount[] }>('mail.tagList', {})
  return tags ?? []
}
