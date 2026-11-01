import type { Rule } from './mailRules'
import { invoke } from '../lib/bridge'
import { columnReadsTags, loadKanbanColumn } from '../lib/kanbanData'
import { getKanbanColumns, kanban$, kanbanColumnKey, kanbanColumnQueries, renameKanbanColumnQueries } from './kanban'
import { loadThreads, mail$, updateKanbanThread } from './mail'
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
  reloadTagColumns()
  return tags
}

const tagToggles = new Map<string, Promise<unknown>>()

function threadLoaded(threadId: string) {
  const holds = (thread: { thread_id: string }) => thread.thread_id === threadId
  return (
    mail$.threads.peek().some(holds) ||
    mail$.messages.peek().some(holds) ||
    Object.values(kanban$.threads.peek()).some((threads) => threads.some(holds))
  )
}

// Flip one tag, answering with the thread's tags, or null if the core refused.
// One at a time per thread, each from the previous answer, however many menus
// came and went: quick clicks on a tag must cancel out. A column reload can take
// the thread out of every view while its menu stays open, so `shown` is that
// menu's own latest answer, for when no view can say.
export function toggleThreadTag(threadId: string, tag: string, shown: () => string[]): Promise<string[] | null> {
  const flip = async () => {
    const on = (threadLoaded(threadId) ? threadTags(threadId) : shown()).includes(tag)
    return setThreadTags(threadId, on ? [] : [tag], on ? [tag] : []).catch(() => null)
  }
  const done = (tagToggles.get(threadId) ?? Promise.resolve()).then(flip)
  tagToggles.set(threadId, done)
  void done.then(() => {
    if (tagToggles.get(threadId) === done) tagToggles.delete(threadId)
  })
  return done
}

// A tag column may gain or lose threads, which no folder sync announces. With
// `every`, the rest load too, which supersedes any read from before a change.
function reloadTagColumns(every = false) {
  const query = kanban$.searchQuery.peek().trim()
  const scope = kanban$.searchScope.peek()
  for (const column of getKanbanColumns(kanban$.activeBoardId.peek())) {
    if (!every && !columnReadsTags({ ...column, query: `${column.query ?? ''} ${query}` })) continue
    void loadKanbanColumn(column, false, scope === 'all' || scope === kanbanColumnKey(column) ? query : '')
  }
}

// Rename a tag everywhere, or delete it with no `to`. The core handles mail and
// rules, answering with the name it kept and the rules as they now stand; boards
// live here, so their saved searches go through it to be rewritten. Whatever is
// on screen follows, as no sync announces this either.
export async function replaceTag(tag: string, to?: string): Promise<Rule[]> {
  const queries = kanbanColumnQueries()
  const result = await invoke<{ tag?: string | null; rules?: Rule[]; queries?: string[] }>('mail.replaceTag', {
    tag,
    to,
    queries,
  })
  const next = result.tag ?? undefined
  const swap = <T extends { tags?: string[] }>(item: T): T => {
    if (!item.tags?.includes(tag)) return item
    const tags = item.tags.flatMap((name) => (name !== tag ? [name] : next ? [next] : []))
    return { ...item, tags: [...new Set(tags)].sort() }
  }
  mail$.threads.set(mail$.threads.get().map(swap))
  mail$.messages.set(mail$.messages.get().map(swap))
  kanban$.threads.set(
    Object.fromEntries(Object.entries(kanban$.threads.get()).map(([key, threads]) => [key, threads.map(swap)])),
  )
  renameKanbanColumnQueries(new Map(queries.map((query, at) => [query, result.queries?.[at] ?? query])))
  reloadTagColumns(true)
  // Read the list again, which also supersedes a read from before this change. A
  // search may gain or lose results, so it runs fresh: a background refresh keeps
  // rows it does not find.
  void loadThreads(ui$.query.peek().trim() !== '')
  return result.rules ?? []
}

export async function loadTagCounts(): Promise<TagCount[]> {
  const { tags } = await invoke<{ tags: TagCount[] }>('mail.tagList', {})
  return tags ?? []
}
