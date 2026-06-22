import { describe, expect, it } from 'bun:test'
import { formatShortcut, isBareShortcut, isMac, matchShortcut, SHORTCUTS } from './shortcuts'

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

  it('does not double Shift for keys that imply it', () => {
    expect(formatShortcut('shortcuts.help')).toEqual([modLabel, '?'])
    expect(formatShortcut('thread.delete')).toEqual(['#'])
  })

  it('covers every defined shortcut without throwing', () => {
    for (const id of Object.keys(SHORTCUTS) as (keyof typeof SHORTCUTS)[]) {
      expect(formatShortcut(id).length).toBeGreaterThan(0)
    }
  })
})
