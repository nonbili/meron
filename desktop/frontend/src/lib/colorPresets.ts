// Colours offered as one-click swatches wherever a colour is picked (account
// colours, the app icon), labelled by `colors.<id>`. Mobile offers the same set
// (AccountColorUi.kt). Any `#rrggbb` is accepted besides these, through the
// picker's custom swatch, so the lists only need to agree for the swatches to
// match.
export const COLOR_PRESETS = [
  { id: 'red', hex: '#ef4444' },
  { id: 'orange', hex: '#f97316' },
  { id: 'amber', hex: '#f59e0b' },
  { id: 'green', hex: '#22c55e' },
  { id: 'teal', hex: '#14b8a6' },
  { id: 'blue', hex: '#3b82f6' },
  { id: 'violet', hex: '#8b5cf6' },
  { id: 'pink', hex: '#ec4899' },
] as const

/** Lowercase `#rrggbb`, or null for anything else. */
export function normalizeHexColor(value: unknown): string | null {
  return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value.trim()) ? value.trim().toLowerCase() : null
}
