// Pure date helpers shared across the UI and state layers. The sidecar sends
// `date` as Unix epoch seconds (0 when unknown); these format it for display in
// the user's local time.

/** Convert epoch seconds to a Date, or null when unknown (0/falsy). */
function fromEpochSeconds(epochSeconds: number): Date | null {
  if (!epochSeconds) return null
  return new Date(epochSeconds * 1000)
}

/** Compact thread-list timestamp: time today, "Yesterday", weekday this week, else "MMM D". */
export function formatThreadDate(epochSeconds: number): string {
  const date = fromEpochSeconds(epochSeconds)
  if (!date) return ''
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })
  }
  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)
  if (date.toDateString() === yesterday.toDateString()) {
    return 'Yesterday'
  }
  const diffDays = (now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24)
  if (diffDays < 7 && diffDays > 0) {
    const days = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
    return days[date.getDay()]
  }
  return date.toLocaleDateString([], { month: 'short', day: 'numeric' })
}
