import { afterEach, beforeEach, expect, test } from 'bun:test'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { newerSchema$ } from '../../states/newerSchema'
import { ui$ } from '../../states/ui'
import { NewerSchemaDialog } from './NewerSchemaDialog'

let calls: Array<[string, unknown]>
let fail: string | null
beforeEach(() => {
  calls = []
  fail = null
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push([command, payload])
          if (fail) throw new Error(fail)
          if (command === 'store.openNewer') {
            return { backups: payload.backup ? ['/profile/meron.db.bak-20261121-1530'] : [] }
          }
          return { ok: true }
        },
      },
    },
  }
})

afterEach(() => {
  cleanup()
  newerSchema$.set(null)
  ui$.toast.set(null as any)
  delete (window as any).go
})

test('stays hidden until the core holds a newer store', () => {
  render(<NewerSchemaDialog />)
  expect(screen.queryByRole('alertdialog')).toBeNull()
})

test('names both schemas and backs up before opening by default', async () => {
  newerSchema$.set({ found: 40, supported: 34 })
  render(<NewerSchemaDialog />)
  expect(screen.getByRole('alertdialog').textContent).toContain('schema 40; this version supports 34')
  fireEvent.click(screen.getByRole('button', { name: 'Back up and open' }))
  await waitFor(() => expect(ui$.toast.get()).toContain('meron.db.bak-20261121-1530'))
  expect(calls).toEqual([['store.openNewer', { backup: true }]])
  // Held until the core reports: the buttons stay off rather than offer the
  // choice twice.
  expect((screen.getByRole('button', { name: 'Open anyway' }) as HTMLButtonElement).disabled).toBe(true)
})

test('opens without a copy, or quits', async () => {
  newerSchema$.set({ found: 40, supported: 34 })
  render(<NewerSchemaDialog />)
  fireEvent.click(screen.getByRole('button', { name: 'Quit' }))
  expect(calls).toEqual([['app.exit', {}]])
  fireEvent.click(screen.getByRole('button', { name: 'Open anyway' }))
  await waitFor(() => expect(calls[1]).toEqual(['store.openNewer', { backup: false }]))
})

test('a reply never clears a newer prompt that came in meanwhile', async () => {
  newerSchema$.set({ found: 40, supported: 34 })
  render(<NewerSchemaDialog />)
  fireEvent.click(screen.getByRole('button', { name: 'Open anyway' }))
  // The core found 41 on disk, answered, and asks again.
  act(() => newerSchema$.set({ found: 41, supported: 34 }))
  await waitFor(() => expect(calls.length).toBe(1))
  await waitFor(() =>
    expect((screen.getByRole('button', { name: 'Open anyway' }) as HTMLButtonElement).disabled).toBe(false),
  )
  expect(newerSchema$.get()).toEqual({ found: 41, supported: 34 })
  expect(screen.getByRole('alertdialog').textContent).toContain('schema 41')
})

test('a failure stays in the dialog, so the user can choose again', async () => {
  newerSchema$.set({ found: 40, supported: 34 })
  fail = 'disk full'
  render(<NewerSchemaDialog />)
  fireEvent.click(screen.getByRole('button', { name: 'Back up and open' }))
  await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('disk full'))
  expect(newerSchema$.get()).toEqual({ found: 40, supported: 34 })
  expect((screen.getByRole('button', { name: 'Back up and open' }) as HTMLButtonElement).disabled).toBe(false)
})
