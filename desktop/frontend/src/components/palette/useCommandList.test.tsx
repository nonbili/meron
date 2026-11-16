import { afterEach, expect, it } from 'bun:test'
import { cleanup, renderHook, waitFor } from '@testing-library/react'
import { ui$ } from '../../states/ui'
import { accounts$ } from '../../states/accounts'
import type { Account } from '../../types'
import { filterCommands } from './paletteCommands'
import { useCommandList } from './useCommandList'

const previousGo = (window as any).go
afterEach(() => {
  cleanup()
  ui$.paletteOpen.set(false)
  ui$.settingsOpen.set(false)
  ui$.accountSettingsId.set('')
  ui$.settingsReveal.set('')
  accounts$.set([])
  ;(window as any).go = previousGo
})

it('finds Open EML by file type and closes the palette before opening its picker', async () => {
  const calls: { command: string; paletteOpen: boolean }[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string) => {
          calls.push({ command, paletteOpen: ui$.paletteOpen.peek() })
          return { ok: true }
        },
      },
    },
  }
  const { result } = renderHook(() => useCommandList())
  const command = filterCommands(result.current, 'eml').find((item) => item.id === 'eml.open')!
  expect(command.label).toBe('Open EML file…')
  expect(command.shortcut).toBe('eml.open')
  ui$.paletteOpen.set(true)
  command.run()
  await waitFor(() => expect(calls).toContainEqual({ command: 'eml.open', paletteOpen: false }))
})

it('finds an individual setting and opens Settings at its row', () => {
  const { result } = renderHook(() => useCommandList())
  expect(filterCommands(result.current, '').some((item) => item.id.startsWith('setting.'))).toBe(false)
  const command = filterCommands(result.current, 'spell')[0]
  expect(command.label).toBe('Settings: Spell check')
  ui$.paletteOpen.set(true)
  command.run()
  expect(ui$.paletteOpen.peek()).toBe(false)
  expect(ui$.settingsOpen.peek()).toBe(true)
  expect(ui$.accountSettingsId.peek()).toBe('composer')
  expect(ui$.settingsReveal.peek()).toBe('Spell check')
})

it('lists per-account settings after the app-wide one, only for accounts that have them', () => {
  accounts$.set([
    { id: 'a1', email: 'user1@mail.localhost', display_name: 'Personal', provider: 'custom', auth_type: 'password' },
    { id: 'rss-1', email: '', display_name: 'Feeds', provider: 'rss', auth_type: 'rss' },
  ] as Account[])
  const { result } = renderHook(() => useCommandList())
  expect(filterCommands(result.current, 'signature').map((item) => item.label)).toEqual([
    'Settings: Signature',
    'Settings: Personal › Signature',
  ])
  const command = filterCommands(result.current, 'personal sig')[0]
  expect(command.label).toBe('Settings: Personal › Signature')
  command.run()
  expect(ui$.settingsOpen.peek()).toBe(true)
  expect(ui$.accountSettingsId.peek()).toBe('a1')
  expect(ui$.settingsReveal.peek()).toBe('Signature')
})
