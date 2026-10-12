// Global keyboard shortcuts. The defaults live here as a small static table;
// users can rebind any of them from the shortcuts dialog, and those overrides
// are persisted in settings (`shortcut_overrides`) and pushed back in here via
// setShortcutOverrides. `mod` is ⌘ on macOS and Ctrl elsewhere — the
// platform-native "command" modifier.

import { isMacPlatform } from './platform'

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
  | 'thread.viewMode'
  | 'reply.focus'
  | 'app.quit'
  | RailShortcutId

/** True for a single-key keystroke (no ⌘/Ctrl/Alt). The shortcuts these fire
 * only run when the user isn't typing and no modal is open — so they don't
 * hijack text entry. Judged from the keystroke rather than the shortcut's bound
 * chord, which an alias (see SHORTCUT_ALIASES) doesn't share. */
export function isBareKeystroke(event: KeyboardEvent): boolean {
  return !(isMac ? event.metaKey : event.ctrlKey) && !event.altKey
}

/** Is the user currently typing into a field? Bare single-key shortcuts must
 * stand down so they don't clobber text entry. */
export function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el) return false
  return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable
}

export type Chord = {
  mod?: boolean
  shift?: boolean
  alt?: boolean
  key: string
}

/** User rebindings, keyed by shortcut id. Missing ids use the default chord. */
export type ShortcutOverrides = Partial<Record<ShortcutId, Chord>>

export const DEFAULT_SHORTCUTS: Record<ShortcutId, Chord> = {
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
  // Linux/Windows only: Ctrl+Q acts like the close button (hides to the tray
  // with close-to-tray on). macOS keeps its native ⌘Q.
  'app.quit': { mod: true, key: 'q' },
  // Gmail-style single-key thread shortcuts (only when not typing).
  'thread.next': { key: 'j' },
  'thread.prev': { key: 'k' },
  'thread.archive': { key: 'e' },
  'thread.star': { key: 's' },
  'thread.unread': { key: 'u' },
  'thread.delete': { shift: true, key: '#' },
  'thread.details': { key: 'i' },
  // H for HTML: switches the open conversation between HTML and plain text.
  'thread.viewMode': { key: 'h' },
  'reply.focus': { key: 'r' },
}

export const SHORTCUT_IDS = Object.keys(DEFAULT_SHORTCUTS) as ShortcutId[]

// Live overrides. states/settings owns the persisted copy and pushes it here on
// hydrate and on every edit; keeping the mirror local means this module stays
// free of state imports (and testable without the settings/bridge stack).
let overrides: ShortcutOverrides = {}

/** Replace the active rebindings. Called by states/settings. */
export function setShortcutOverrides(next: ShortcutOverrides) {
  overrides = next
}

/** The chord currently bound to a shortcut: the user's override, else the default. */
export function shortcutChord(id: ShortcutId): Chord {
  return overrides[id] ?? DEFAULT_SHORTCUTS[id]
}

/** Whether a shortcut is rebound (so the UI can offer to reset just that row). */
export function isShortcutCustomized(id: ShortcutId): boolean {
  return !!overrides[id]
}

export function chordEquals(a: Chord, b: Chord): boolean {
  return (
    !!a.mod === !!b.mod && !!a.shift === !!b.shift && !!a.alt === !!b.alt && a.key.toLowerCase() === b.key.toLowerCase()
  )
}

// Keys that can't stand alone as a binding: bare modifiers produce a keydown of
// their own, Escape cancels the recorder, and Tab has to keep moving focus.
const UNBINDABLE_KEYS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'CapsLock', 'Dead', 'Escape', 'Tab'])

/**
 * The chord a keydown represents, for the rebinding recorder. Returns null for
 * keystrokes that can't be bound — a lone modifier, Escape/Tab, or the "other"
 * command key (⌃ on macOS, ⌘ elsewhere), which chords here never use.
 */
export function chordFromEvent(event: KeyboardEvent): Chord | null {
  if (UNBINDABLE_KEYS.has(event.key)) return null
  const otherMod = isMac ? event.ctrlKey : event.metaKey
  if (otherMod) return null

  const chord: Chord = { key: event.key.length === 1 ? event.key.toLowerCase() : event.key }
  if (isMac ? event.metaKey : event.ctrlKey) chord.mod = true
  if (event.shiftKey) chord.shift = true
  if (event.altKey) chord.alt = true
  return chord
}

/** The shortcut already using this chord, if any — so a rebind can warn first. */
export function shortcutConflict(id: ShortcutId, chord: Chord): ShortcutId | null {
  for (const other of SHORTCUT_IDS) {
    if (other === id) continue
    if (chordEquals(shortcutChord(other), chord)) return other
  }
  return null
}

/**
 * Coerce persisted overrides into the canonical shape, dropping unknown ids and
 * malformed chords. Returns null for anything unrecognizable so hydration
 * leaves the current value alone.
 */
export function sanitizeShortcutOverrides(raw: unknown): ShortcutOverrides | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const out: ShortcutOverrides = {}
  for (const [id, value] of Object.entries(raw as Record<string, unknown>)) {
    if (!(id in DEFAULT_SHORTCUTS)) continue
    if (!value || typeof value !== 'object' || Array.isArray(value)) continue
    const chord = value as Record<string, unknown>
    if (typeof chord.key !== 'string' || !chord.key || UNBINDABLE_KEYS.has(chord.key)) continue
    const next: Chord = { key: chord.key }
    if (chord.mod === true) next.mod = true
    if (chord.shift === true) next.shift = true
    if (chord.alt === true) next.alt = true
    // A rebinding that matches the default is redundant; drop it so the row
    // doesn't read as customized.
    if (chordEquals(next, DEFAULT_SHORTCUTS[id as ShortcutId])) continue
    out[id as ShortcutId] = next
  }
  return out
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
  'view.rail1': 'Go to side navigation item 1',
  'view.rail2': 'Go to side navigation item 2',
  'view.rail3': 'Go to side navigation item 3',
  'view.rail4': 'Go to side navigation item 4',
  'view.rail5': 'Go to side navigation item 5',
  'view.rail6': 'Go to side navigation item 6',
  'view.rail7': 'Go to side navigation item 7',
  'view.rail8': 'Go to side navigation item 8',
  'view.rail9': 'Go to side navigation item 9',
  'tab.close': 'Close tab',
  'thread.next': 'Next thread',
  'thread.prev': 'Previous thread',
  'thread.archive': 'Archive thread',
  'thread.star': 'Toggle star',
  'thread.unread': 'Mark unread',
  'thread.delete': 'Delete thread',
  'thread.details': 'Toggle details sidebar',
  'thread.viewMode': 'Toggle HTML / plain text',
  'app.quit': 'Quit Meron',
}

export const isMac = isMacPlatform(navigator.userAgent, navigator.platform ?? '')

/** Grouping for the help overlay, in display order. */
export const SHORTCUT_GROUPS: { title: string; ids: ShortcutId[] }[] = [
  {
    title: 'General',
    // No quit row on macOS: the native ⌘Q owns that chord, so a rebind would do nothing.
    ids: ['palette.open', 'shortcuts.help', 'settings.open', ...(isMac ? [] : (['app.quit'] as const))],
  },
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
      'thread.viewMode',
    ],
  },
  {
    title: 'Mail',
    ids: ['compose.new', 'reply.focus', 'compose.replyFull', 'mail.sync', 'search.thread', 'search.global'],
  },
  { title: 'View', ids: ['view.toggle', 'tab.close'] },
  // Every rail slot gets a row, so all nine are visible and rebindable.
  { title: 'Side navigation', ids: [...RAIL_SHORTCUT_IDS] },
]

/** Identify which shortcut, if any, a keydown event matches. Returns null when
 * nothing matches so callers can let the event through. */
export function matchShortcut(event: KeyboardEvent): ShortcutId | null {
  // The "other" command key must be up, so ⌃K on macOS doesn't fire ⌘K.
  const otherMod = isMac ? event.ctrlKey : event.metaKey
  if (otherMod) return null

  return shortcutForChord({
    key: event.key,
    mod: isMac ? event.metaKey : event.ctrlKey,
    shift: event.shiftKey,
    alt: event.altKey,
  })
}

/** Same lookup for a chord reconstructed by hand — e.g. a keystroke forwarded
 * out of a message iframe, which arrives as data rather than a KeyboardEvent. */
export function shortcutForChord(chord: Chord): ShortcutId | null {
  // User rebindings first, so a newly added default can't take over a chord the
  // user already claimed for another shortcut.
  for (const id of SHORTCUT_IDS) {
    const override = overrides[id]
    if (override && chordEquals(override, chord)) return id
  }
  for (const id of SHORTCUT_IDS) {
    if (!overrides[id] && chordEquals(DEFAULT_SHORTCUTS[id], chord)) return id
  }
  for (const [id, alias] of SHORTCUT_ALIASES) {
    if (chordEquals(alias, chord)) return id
  }
  return null
}

// Fixed secondary chords, matched only when no bound shortcut claims the
// keystroke — so a user rebinding onto one still wins. ⌘/Ctrl+Shift+K opens
// the palette too, for muscle memory from apps that use it.
const SHORTCUT_ALIASES: [ShortcutId, Chord][] = [['palette.open', { mod: true, shift: true, key: 'k' }]]

/** Render a shortcut as display parts, e.g. ["⌘", "K"] or ["Ctrl", "Shift", "R"]. */
export function formatShortcut(id: ShortcutId): string[] {
  return formatChord(shortcutChord(id))
}

function formatChord(chord: Chord): string[] {
  const parts: string[] = []
  if (chord.mod) parts.push(isMac ? '⌘' : 'Ctrl')
  if (chord.alt) parts.push(isMac ? '⌥' : 'Alt')
  // "?" and "#" already carry Shift on most layouts, so don't double it up.
  if (chord.shift && chord.key !== '?' && chord.key !== '#') {
    parts.push(isMac ? '⇧' : 'Shift')
  }
  // Space's KeyboardEvent.key is " ", which would render as an empty badge.
  if (chord.key === ' ') parts.push('Space')
  else parts.push(chord.key.length === 1 ? chord.key.toUpperCase() : chord.key)
  return parts
}
