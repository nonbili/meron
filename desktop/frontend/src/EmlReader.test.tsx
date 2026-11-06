import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { EmlReader } from './EmlReader'
import { SETTINGS_DB_KEYS } from './states/settings'

const previousGo = (window as any).go
afterEach(() => {
  cleanup()
  window.history.replaceState(null, '', '/')
  ;(window as any).go = previousGo
})

it('reads without an account, saves through its own session and closes from an HTML frame', async () => {
  window.history.replaceState(null, '', '/?eml=reader-1')
  const calls: { command: string; payload: any }[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'eml.get')
            return {
              subject: 'Saved message',
              from_name: 'Alice',
              from_addr: 'alice@example.com',
              to: 'bob@example.com',
              cc: '',
              bcc: '',
              reply_to: '',
              date: 0,
              body: 'Plain body',
              body_html:
                '<html><head><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src \'self\'"></head><body><p>HTML body</p></body></html>',
              attachments: [
                { key: '_eml/reader-1/1/0.pdf', filename: 'report.pdf', mime: 'application/pdf', size: 4, url: null },
              ],
            }
          if (command === 'app.prefsGet') {
            expect(payload).toEqual({ keys: SETTINGS_DB_KEYS })
            return { prefs: { close_to_tray: false } }
          }
          if (command === 'system.appearance') return { dark: false }
          return { ok: true }
        },
      },
    },
  }
  const view = render(<EmlReader session="reader-1" />)
  await waitFor(() => expect(view.getByRole('heading', { name: 'Saved message' })).toBeTruthy())
  expect(view.getByText('alice@example.com', { exact: false })).toBeTruthy()
  const remote = view.getByRole('checkbox') as HTMLInputElement
  expect(remote.checked).toBe(false)
  fireEvent.click(remote)
  expect(remote.checked).toBe(true)
  fireEvent.click(view.getByRole('button', { name: 'Plain' }))
  expect(view.getByText('Plain body')).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'report.pdf' }))
  await waitFor(() =>
    expect(
      calls.some(
        ({ command, payload }) =>
          command === 'eml.saveAttachment' && payload.session === 'reader-1' && payload.key === '_eml/reader-1/1/0.pdf',
      ),
    ).toBe(true),
  )
  const event = new CustomEvent('meron.frameKeyDown', { cancelable: true, detail: { key: 'w', mod: true } })
  window.dispatchEvent(event)
  expect(event.defaultPrevented).toBe(true)
  expect(calls.some(({ command, payload }) => command === 'eml.close' && payload.session === 'reader-1')).toBe(true)
  expect(
    calls.some(
      ({ command }) => command === 'account.list' || command.startsWith('mail.') || command.startsWith('window.'),
    ),
  ).toBe(false)
})
