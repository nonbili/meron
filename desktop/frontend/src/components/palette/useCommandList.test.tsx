import { afterEach, expect, it } from 'bun:test'
import { cleanup, renderHook, waitFor } from '@testing-library/react'
import { ui$ } from '../../states/ui'
import { filterCommands } from './paletteCommands'
import { useCommandList } from './useCommandList'

const previousGo = (window as any).go
afterEach(() => {
  cleanup()
  ui$.paletteOpen.set(false)
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
