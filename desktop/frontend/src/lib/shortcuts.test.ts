import { afterEach, describe, expect, it } from 'bun:test'
import {
  chordFromEvent,
  DEFAULT_SHORTCUTS,
  formatShortcut,
  isBareShortcut,
  isMac,
  isShortcutCustomized,
  matchShortcut,
  sanitizeShortcutOverrides,
  SHORTCUT_GROUPS,
  setShortcutOverrides,
  shortcutChord,
  shortcutConflict,
  shortcutForChord,
  SHORTCUT_IDS,
} from './shortcuts'

const keydown = (key: string, mods: Partial<KeyboardEvent> = {}): KeyboardEvent =>
  ({
    key,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    ...mods,
  }) as KeyboardEvent

const modKey = isMac ? { metaKey: true } : { ctrlKey: true }
const otherModKey = isMac ? { ctrlKey: true } : { metaKey: true }
const modLabel = isMac ? '⌘' : 'Ctrl'
const shiftLabel = isMac ? '⇧' : 'Shift'

describe('matchShortcut', () => {
  it('matches a mod chord', () => {
    expect(matchShortcut(keydown('k', modKey))).toBe('palette.open')
  })

  it('matches a mod+shift chord', () => {
    expect(matchShortcut(keydown('R', { ...modKey, shiftKey: true }))).toBe('mail.sync')
  })

  it('matches bare single-key shortcuts', () => {
    expect(matchShortcut(keydown('j'))).toBe('thread.next')
    expect(matchShortcut(keydown('#', { shiftKey: true }))).toBe('thread.delete')
  })

  it('does not fire when the other command key is held', () => {
    expect(matchShortcut(keydown('k', { ...modKey, ...otherModKey }))).toBeNull()
  })

  it('returns null when nothing matches', () => {
    expect(matchShortcut(keydown('q'))).toBeNull()
    expect(matchShortcut(keydown('j', modKey))).toBeNull()
  })
})

describe('isBareShortcut', () => {
  it('is true for single-key and shift-only chords, false for mod chords', () => {
    expect(isBareShortcut('thread.next')).toBe(true)
    expect(isBareShortcut('thread.delete')).toBe(true)
    expect(isBareShortcut('palette.open')).toBe(false)
  })
})

describe('formatShortcut', () => {
  it('renders mod chords with the platform command modifier', () => {
    expect(formatShortcut('palette.open')).toEqual([modLabel, 'K'])
    expect(formatShortcut('mail.sync')).toEqual([modLabel, shiftLabel, 'R'])
  })

  it('names Space, whose key is a blank string', () => {
    setShortcutOverrides({ 'thread.star': { key: ' ' } })
    expect(formatShortcut('thread.star')).toEqual(['Space'])
    setShortcutOverrides({})
  })

  it('does not double Shift for keys that imply it', () => {
    expect(formatShortcut('shortcuts.help')).toEqual([modLabel, '?'])
    expect(formatShortcut('thread.delete')).toEqual(['#'])
  })

  it('covers every defined shortcut without throwing', () => {
    for (const id of SHORTCUT_IDS) {
      expect(formatShortcut(id).length).toBeGreaterThan(0)
    }
  })
})

describe('custom bindings', () => {
  afterEach(() => setShortcutOverrides({}))

  it('resolves, matches and formats an override in place of the default', () => {
    setShortcutOverrides({ 'thread.next': { key: 'n' } })
    expect(shortcutChord('thread.next')).toEqual({ key: 'n' })
    expect(matchShortcut(keydown('n'))).toBe('thread.next')
    expect(matchShortcut(keydown('j'))).toBeNull()
    expect(formatShortcut('thread.next')).toEqual(['N'])
    expect(isShortcutCustomized('thread.next')).toBe(true)
    expect(isShortcutCustomized('thread.prev')).toBe(false)
  })

  it('re-evaluates bareness from the bound chord', () => {
    setShortcutOverrides({ 'thread.next': { mod: true, key: 'j' }, 'palette.open': { key: 'p' } })
    expect(isBareShortcut('thread.next')).toBe(false)
    expect(isBareShortcut('palette.open')).toBe(true)
  })

  it('reports the shortcut already holding a chord', () => {
    expect(shortcutConflict('thread.next', { key: 'k' })).toBe('thread.prev')
    expect(shortcutConflict('thread.next', { key: 'j' })).toBeNull()
    expect(shortcutConflict('thread.next', { key: 'z' })).toBeNull()
    // Conflicts are checked against the live bindings, not the defaults.
    setShortcutOverrides({ 'thread.prev': { key: 'z' } })
    expect(shortcutConflict('thread.next', { key: 'k' })).toBeNull()
    expect(shortcutConflict('thread.next', { key: 'z' })).toBe('thread.prev')
  })
})

describe('shortcutForChord', () => {
  afterEach(() => setShortcutOverrides({}))

  it('resolves a hand-built chord, so forwarded keys match the same table', () => {
    expect(shortcutForChord({ key: 'j' })).toBe('thread.next')
    expect(shortcutForChord({ key: 'ArrowDown' })).toBeNull()
    setShortcutOverrides({ 'thread.archive': { key: 'ArrowDown' } })
    expect(shortcutForChord({ key: 'ArrowDown' })).toBe('thread.archive')
  })
})

describe('chordFromEvent', () => {
  it('captures modifiers and lowercases printable keys', () => {
    expect(chordFromEvent(keydown('J', { ...modKey, shiftKey: true }))).toEqual({
      key: 'j',
      mod: true,
      shift: true,
    })
    expect(chordFromEvent(keydown('ArrowDown'))).toEqual({ key: 'ArrowDown' })
  })

  it('rejects keystrokes that cannot be bound', () => {
    expect(chordFromEvent(keydown('Shift', { shiftKey: true }))).toBeNull()
    expect(chordFromEvent(keydown('Escape'))).toBeNull()
    expect(chordFromEvent(keydown('Tab'))).toBeNull()
    expect(chordFromEvent(keydown('k', otherModKey))).toBeNull()
  })
})

describe('sanitizeShortcutOverrides', () => {
  it('keeps well-formed entries and drops the rest', () => {
    expect(
      sanitizeShortcutOverrides({
        'thread.next': { key: 'n', mod: true, shift: 'yes' },
        'thread.prev': { key: '' },
        'not.a.shortcut': { key: 'x' },
        'thread.star': 'nope',
        'thread.unread': { key: 'Escape' },
      }),
    ).toEqual({ 'thread.next': { key: 'n', mod: true } })
  })

  it('drops overrides that just repeat the default', () => {
    expect(sanitizeShortcutOverrides({ 'thread.next': DEFAULT_SHORTCUTS['thread.next'] })).toEqual({})
  })

  it('returns null for values that are not an object map', () => {
    expect(sanitizeShortcutOverrides(null)).toBeNull()
    expect(sanitizeShortcutOverrides([])).toBeNull()
  })
})

describe('SHORTCUT_GROUPS', () => {
  it('lists every shortcut exactly once, so all of them are visible and rebindable', () => {
    const listed = SHORTCUT_GROUPS.flatMap((group) => group.ids)
    expect([...listed].sort()).toEqual([...SHORTCUT_IDS].sort())
  })
})
