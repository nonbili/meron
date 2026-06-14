import { describe, expect, it } from 'bun:test'
import { formatShortcut, isBareShortcut, matchShortcut, SHORTCUTS } from './shortcuts'

// Bun reports a non-mac userAgent, so the module resolves the command modifier
// to Ctrl; these tests assume that.
const keydown = (key: string, mods: Partial<KeyboardEvent> = {}): KeyboardEvent =>
  ({
    key,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    ...mods,
  }) as KeyboardEvent

describe('matchShortcut', () => {
  it('matches a mod chord', () => {
    expect(matchShortcut(keydown('k', { ctrlKey: true }))).toBe('palette.open')
  })

  it('matches a mod+shift chord', () => {
    expect(matchShortcut(keydown('R', { ctrlKey: true, shiftKey: true }))).toBe('mail.sync')
  })

  it('matches bare single-key shortcuts', () => {
    expect(matchShortcut(keydown('j'))).toBe('thread.next')
    expect(matchShortcut(keydown('#', { shiftKey: true }))).toBe('thread.delete')
  })

  it('does not fire when the other command key is held', () => {
    // On non-mac, metaKey is the "other" modifier and must veto the match.
    expect(matchShortcut(keydown('k', { ctrlKey: true, metaKey: true }))).toBeNull()
  })

  it('returns null when nothing matches', () => {
    expect(matchShortcut(keydown('q'))).toBeNull()
    expect(matchShortcut(keydown('j', { ctrlKey: true }))).toBeNull()
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
  it('renders mod chords with Ctrl on non-mac', () => {
    expect(formatShortcut('palette.open')).toEqual(['Ctrl', 'K'])
    expect(formatShortcut('mail.sync')).toEqual(['Ctrl', 'Shift', 'R'])
  })

  it('does not double Shift for keys that imply it', () => {
    expect(formatShortcut('shortcuts.help')).toEqual(['Ctrl', '?'])
    expect(formatShortcut('thread.delete')).toEqual(['#'])
  })

  it('covers every defined shortcut without throwing', () => {
    for (const id of Object.keys(SHORTCUTS) as (keyof typeof SHORTCUTS)[]) {
      expect(formatShortcut(id).length).toBeGreaterThan(0)
    }
  })
})
