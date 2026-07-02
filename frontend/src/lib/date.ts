// Pure date helpers shared across the UI and state layers. The sidecar sends
// `date` as Unix epoch seconds (0 when unknown); these format it for display in
// the user's local time.

/** Convert epoch seconds to a Date, or null when unknown (0/falsy). */
function fromEpochSeconds(epochSeconds: number): Date | null {
  if (!epochSeconds) return null
  return new Date(epochSeconds * 1000)
}

/** Gmail-style thread-list timestamp: time today, month/day this year, else month/day/year. */
export function formatThreadDate(epochSeconds: number): string {
  const date = fromEpochSeconds(epochSeconds)
  if (!date) return ''
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })
  }
  const options: Intl.DateTimeFormatOptions =
    date.getFullYear() === now.getFullYear()
      ? { month: 'short', day: 'numeric' }
      : { month: 'short', day: 'numeric', year: 'numeric' }
  return date.toLocaleDateString([], options)
}
