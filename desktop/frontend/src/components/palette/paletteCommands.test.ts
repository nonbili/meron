import { describe, expect, test } from 'bun:test'
import { filterCommands, type Command } from './paletteCommands'

const commands: Command[] = [
  { label: 'Search current thread', keywords: 'find conversation in' },
  { label: 'Search all messages', keywords: 'find global mailbox' },
  { label: 'Keyboard shortcuts', keywords: 'keys cheat sheet help bindings' },
  { label: 'Open settings', keywords: 'preferences config' },
  { label: 'Go to: Work', keywords: 'account switch alice@example.com' },
  { label: 'Theme: Indigo' },
  { label: 'Theme: Indigo Dark' },
].map((command) => ({ ...command, id: command.label, icon: null, run: () => {} }))

const labels = (query: string) => filterCommands(commands, query).map((command) => command.label)

describe('command palette search', () => {
  test('direct theme matches suppress unrelated keyword fallbacks', () => {
    expect(labels('indi')).toEqual(['Theme: Indigo', 'Theme: Indigo Dark'])
  })

  test('hidden keyword phrases and email fragments remain searchable', () => {
    expect(labels('cheat sheet')).toContain('Keyboard shortcuts')
    expect(labels('example.com')).toContain('Go to: Work')
    expect(labels('ample')).toContain('Go to: Work')
  })

  test('compact and fuzzy keyword searches work as fallbacks', () => {
    expect(labels('cheatsheet')).toContain('Keyboard shortcuts')
    expect(labels('prefs')).toContain('Open settings')
  })

  test('blank searches preserve order and unmatched queries return no results', () => {
    expect(filterCommands(commands, '  ')).toEqual(commands)
    expect(labels('zzzzzz')).toEqual([])
  })

  test('search-only commands stay off the blank list but match a query', () => {
    const withSetting = [
      ...commands,
      { id: 'setting', label: 'Settings: Spell check', icon: null, searchOnly: true, run: () => {} },
    ]
    expect(filterCommands(withSetting, '')).toEqual(commands)
    expect(filterCommands(withSetting, 'spell').map((command) => command.label)).toEqual(['Settings: Spell check'])
  })
})
