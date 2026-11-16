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
  /** Listed only once something is typed, so a blank palette stays short. */
  searchOnly?: boolean
  run: () => void
}

// Scores 0–3 are direct matches; weaker matches are used only as a fallback.
function commandMatchScore(command: Command, query: string): number {
  const q = query.trim().toLowerCase()
  if (!q) return 0
  const label = command.label.toLowerCase()
  if (label === q) return 0
  if (label.includes(q)) return 1
  const keywords = (command.keywords ?? '').toLowerCase()
  // Match phrases and word starts, including domains after an email's @.
  for (let index = keywords.indexOf(q); index !== -1; index = keywords.indexOf(q, index + 1)) {
    if (index === 0 || /[^a-z0-9]/.test(keywords[index - 1])) return 2
  }

  const compactHaystack = label.replace(/[^a-z0-9]/g, '')
  const compactQuery = q.replace(/[^a-z0-9]/g, '')
  if (!compactQuery) return 3
  if (compactHaystack.includes(compactQuery)) return 3
  if (keywords.includes(q)) return 4
  if (isSubsequence(compactHaystack, compactQuery)) return 5

  const compactKeywords = keywords.replace(/[^a-z0-9]/g, '')
  if (compactKeywords.includes(compactQuery)) return 6
  if (isSubsequence(compactKeywords, compactQuery)) return 7
  return Infinity
}

function isSubsequence(text: string, query: string): boolean {
  let queryIndex = 0
  for (const char of text) {
    if (char === query[queryIndex]) queryIndex += 1
    if (queryIndex === query.length) return true
  }
  return false
}

export function filterCommands(commands: Command[], query: string): Command[] {
  if (!query.trim()) return commands.filter((command) => !command.searchOnly)
  const matches = commands
    .map((command) => ({ command, score: commandMatchScore(command, query) }))
    .filter(({ score }) => Number.isFinite(score))
  const hasDirectMatch = matches.some(({ score }) => score <= 3)
  // Stable sorting preserves the original order for equally strong matches.
  return matches
    .filter(({ score }) => !hasDirectMatch || score <= 3)
    .sort((a, b) => a.score - b.score)
    .map(({ command }) => command)
}
