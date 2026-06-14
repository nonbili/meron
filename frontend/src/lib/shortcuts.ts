// Global keyboard shortcuts. Meron has no backend keybinding config (unlike the
// terminal it borrows the palette idea from), so the chords live here as a small
// static table. `mod` is ⌘ on macOS and Ctrl elsewhere — the platform-native
// "command" modifier.

export const RAIL_SHORTCUT_IDS = [
  'view.rail1',
  'view.rail2',
  'view.rail3',
  'view.rail4',
  'view.rail5',
  'view.rail6',
  'view.rail7',
  'view.rail8',
  'view.rail9',
] as const

export type RailShortcutId = (typeof RAIL_SHORTCUT_IDS)[number]

export type ShortcutId =
  | 'palette.open'
  | 'compose.new'
  | 'settings.open'
  | 'mail.sync'
  | 'view.toggle'
  | 'search.thread'
  | 'search.global'
  | 'compose.replyFull'
  | 'shortcuts.help'
  | 'tab.close'
  | 'thread.next'
  | 'thread.prev'
  | 'thread.archive'
  | 'thread.star'
  | 'thread.unread'
  | 'thread.delete'
  | 'thread.details'
  | 'reply.focus'
  | RailShortcutId

/** True for single-key shortcuts (no ⌘/Ctrl/Alt). These only fire when the user
 * isn't typing and no modal is open — so they don't hijack text entry. */
export function isBareShortcut(id: ShortcutId): boolean {
  const chord = SHORTCUTS[id]
  return !chord.mod && !chord.alt
}

type Chord = {
  mod?: boolean
  shift?: boolean
  alt?: boolean
  key: string
}

export const SHORTCUTS: Record<ShortcutId, Chord> = {
  'palette.open': { mod: true, key: 'k' },
  'compose.new': { mod: true, key: 'n' },
  'settings.open': { mod: true, key: ',' },
  'mail.sync': { mod: true, shift: true, key: 'r' },
  'view.toggle': { mod: true, shift: true, key: 'v' },
  'view.rail1': { mod: true, key: '1' },
  'view.rail2': { mod: true, key: '2' },
  'view.rail3': { mod: true, key: '3' },
  'view.rail4': { mod: true, key: '4' },
  'view.rail5': { mod: true, key: '5' },
  'view.rail6': { mod: true, key: '6' },
  'view.rail7': { mod: true, key: '7' },
  'view.rail8': { mod: true, key: '8' },
  'view.rail9': { mod: true, key: '9' },
  // VSCode-style: find in current thread / find across all messages.
  'search.thread': { mod: true, key: 'f' },
  'search.global': { mod: true, shift: true, key: 'f' },
  // Expand the current thread's quick reply into the full-window editor.
  'compose.replyFull': { mod: true, key: 'e' },
  // ⌘/Ctrl+? — "?" already implies Shift on most layouts.
  'shortcuts.help': { mod: true, shift: true, key: '?' },
  'tab.close': { mod: true, key: 'w' },
  // Gmail-style single-key thread shortcuts (only when not typing).
  'thread.next': { key: 'j' },
  'thread.prev': { key: 'k' },
  'thread.archive': { key: 'e' },
  'thread.star': { key: 's' },
  'thread.unread': { key: 'u' },
  'thread.delete': { shift: true, key: '#' },
  'thread.details': { key: 'i' },
  'reply.focus': { key: 'r' },
}

/** Human-readable name for each shortcut, shown in the help overlay. */
export const SHORTCUT_LABELS: Record<ShortcutId, string> = {
  'palette.open': 'Command palette',
  'shortcuts.help': 'Keyboard shortcuts',
  'settings.open': 'Open settings',
  'compose.new': 'Compose new message',
  'compose.replyFull': 'Reply in full editor',
  'reply.focus': 'Reply (focus quick reply)',
  'mail.sync': 'Sync mailbox',
  'search.thread': 'Search current thread',
  'search.global': 'Search all messages',
  'view.toggle': 'Toggle Mail / Kanban board',
  'view.rail1': 'Go to sidebar item 1',
  'view.rail2': 'Go to sidebar item 2',
  'view.rail3': 'Go to sidebar item 3',
  'view.rail4': 'Go to sidebar item 4',
  'view.rail5': 'Go to sidebar item 5',
  'view.rail6': 'Go to sidebar item 6',
  'view.rail7': 'Go to sidebar item 7',
  'view.rail8': 'Go to sidebar item 8',
  'view.rail9': 'Go to sidebar item 9',
  'tab.close': 'Close tab',
  'thread.next': 'Next thread',
  'thread.prev': 'Previous thread',
  'thread.archive': 'Archive thread',
  'thread.star': 'Toggle star',
  'thread.unread': 'Mark unread',
  'thread.delete': 'Delete thread',
  'thread.details': 'Toggle details sidebar',
}

/** Grouping for the help overlay, in display order. */
export const SHORTCUT_GROUPS: { title: string; ids: ShortcutId[] }[] = [
  { title: 'General', ids: ['palette.open', 'shortcuts.help', 'settings.open'] },
  {
    title: 'Threads',
    ids: [
      'thread.next',
      'thread.prev',
      'thread.archive',
      'thread.star',
      'thread.unread',
      'thread.delete',
      'thread.details',
    ],
  },
  {
    title: 'Mail',
    ids: ['compose.new', 'reply.focus', 'compose.replyFull', 'mail.sync', 'search.thread', 'search.global'],
  },
  { title: 'View', ids: ['view.toggle', 'view.rail1', 'view.rail2', 'view.rail3', 'tab.close'] },
]

export const isMac = /mac|iphone|ipad|ipod/i.test(navigator.userAgent + ' ' + (navigator.platform ?? ''))

/** Identify which shortcut, if any, a keydown event matches. Returns null when
 * nothing matches so callers can let the event through. */
export function matchShortcut(event: KeyboardEvent): ShortcutId | null {
  const mod = isMac ? event.metaKey : event.ctrlKey
  // The "other" command key must be up, so ⌃K on macOS doesn't fire ⌘K.
  const otherMod = isMac ? event.ctrlKey : event.metaKey
  if (otherMod) return null

  const key = event.key.toLowerCase()
  for (const id of Object.keys(SHORTCUTS) as ShortcutId[]) {
    const chord = SHORTCUTS[id]
    if (!!chord.mod !== mod) continue
    if (!!chord.shift !== event.shiftKey) continue
    if (!!chord.alt !== event.altKey) continue
    if (key !== chord.key.toLowerCase()) continue
    return id
  }
  return null
}

/** Render a shortcut as display parts, e.g. ["⌘", "K"] or ["Ctrl", "Shift", "R"]. */
export function formatShortcut(id: ShortcutId): string[] {
  const chord = SHORTCUTS[id]
  const parts: string[] = []
  if (chord.mod) parts.push(isMac ? '⌘' : 'Ctrl')
  if (chord.alt) parts.push(isMac ? '⌥' : 'Alt')
  // "?" and "#" already carry Shift on most layouts, so don't double it up.
  if (chord.shift && chord.key !== '?' && chord.key !== '#') {
    parts.push(isMac ? '⇧' : 'Shift')
  }
  parts.push(chord.key.length === 1 ? chord.key.toUpperCase() : chord.key)
  return parts
}
