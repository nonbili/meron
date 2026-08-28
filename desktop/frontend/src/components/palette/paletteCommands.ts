import type { ReactNode } from 'react'
import type { ShortcutId } from '../../lib/shortcuts'

export type Command = {
  id: string
  label: string
  icon: ReactNode
  /** Extra words matched by the search box but not shown. */
  keywords?: string
  shortcut?: ShortcutId
  /** Marks the command that reflects the current state (shows a check). */
  active?: boolean
  run: () => void
}

// Fuzzy-ish match: substring on the full haystack, then on an alphanumeric-only
// compaction, then a subsequence test so "mau" can hit "Mark all unread".
export function matchesCommand(command: Command, query: string): boolean {
  const q = query.trim().toLowerCase()
  if (!q) return true
  const haystack = `${command.label} ${command.keywords ?? ''}`.toLowerCase()
  if (haystack.includes(q)) return true

  const compactHaystack = haystack.replace(/[^a-z0-9]/g, '')
  const compactQuery = q.replace(/[^a-z0-9]/g, '')
  if (!compactQuery) return true
  if (compactHaystack.includes(compactQuery)) return true

  let queryIndex = 0
  for (const char of compactHaystack) {
    if (char === compactQuery[queryIndex]) queryIndex += 1
    if (queryIndex === compactQuery.length) return true
  }
  return false
}
