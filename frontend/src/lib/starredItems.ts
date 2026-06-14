import type { Message } from '../types'

// Rows for the unified Starred view: individual starred messages / feed items
// across every account, newest first. The view is one un-paginated fetch, so
// the thread-list search box is applied client-side here.
export function mergeStarredItems(items: Message[], query: string): Message[] {
  // `date` is epoch seconds; newest first, unknown (0) last.
  const sorted = [...items].sort((a, b) => b.date - a.date)
  const q = query.trim().toLowerCase()
  if (!q) return sorted
  return sorted.filter((item) =>
    [item.subject, item.preview, item.from_name, item.from_addr].some((field) =>
      (field ?? '').toLowerCase().includes(q),
    ),
  )
}
