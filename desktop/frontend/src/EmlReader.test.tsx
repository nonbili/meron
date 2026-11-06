import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { EmlReader } from './EmlReader'
import { isMac, setShortcutOverrides } from './lib/shortcuts'
import { SETTINGS_DB_KEYS } from './states/settings'

const previousGo = (window as any).go
afterEach(() => {
  cleanup()
  setShortcutOverrides({})
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
              cc: 'Carol <carol@example.com>',
              bcc: 'Hidden <hidden@example.com>',
              reply_to: 'Replies <replies@example.com>',
              date: 0,
              body: 'Plain body',
              body_html:
                '<html><head><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src \'self\'"></head><body><p>HTML body</p><img src="https://example.com/image.png"></body></html>',
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
  const details = view.getByRole('button', { name: 'Show details' })
  expect(details.getAttribute('aria-expanded')).toBe('false')
  expect(view.queryByTitle('Hidden <hidden@example.com>')).toBeNull()
  fireEvent.click(details)
  expect(view.getByRole('button', { name: 'Hide details' }).getAttribute('aria-expanded')).toBe('true')
  expect(view.getByTitle('Carol <carol@example.com>')).toBeTruthy()
  expect(view.getByTitle('Hidden <hidden@example.com>')).toBeTruthy()
  expect(view.getByTitle('Replies <replies@example.com>')).toBeTruthy()
  expect(view.queryByRole('button', { name: 'Open contact' })).toBeNull()
  fireEvent.click(view.getByRole('button', { name: 'Hide details' }))
  expect(view.queryByTitle('Hidden <hidden@example.com>')).toBeNull()
  expect(view.queryByRole('checkbox')).toBeNull()
  expect(view.getByRole('button', { name: 'HTML' }).getAttribute('aria-pressed')).toBe('true')
  fireEvent.click(view.getByRole('button', { name: 'Remote content blocked' }))
  fireEvent.click(view.getByText('Show remote content'))
  expect(view.queryByRole('button', { name: 'Remote content blocked' })).toBeNull()
  fireEvent.click(view.getByRole('button', { name: 'Plain' }))
  expect(view.getByText('Plain body')).toBeTruthy()
  expect(view.getByRole('button', { name: 'Plain' }).getAttribute('aria-pressed')).toBe('true')
  fireEvent.click(view.getByRole('button', { name: 'report.pdf' }))
  await waitFor(() =>
    expect(
      calls.some(
        ({ command, payload }) =>
          command === 'eml.saveAttachment' && payload.session === 'reader-1' && payload.key === '_eml/reader-1/1/0.pdf',
      ),
    ).toBe(true),
  )
  await act(async () => {
    fireEvent.keyDown(document.body, { key: 'o', ctrlKey: !isMac, metaKey: isMac, repeat: true })
    expect(calls.filter(({ command }) => command === 'eml.open')).toHaveLength(0)
    fireEvent.keyDown(document.body, { key: 'o', ctrlKey: !isMac, metaKey: isMac })
  })
  expect(calls.filter(({ command }) => command === 'eml.open')).toEqual([
    { command: 'eml.open', payload: { session: 'reader-1' } },
  ])
  setShortcutOverrides({ 'eml.open': { mod: true, shift: true, key: 'o' } })
  fireEvent.keyDown(document.body, { key: 'o', ctrlKey: !isMac, metaKey: isMac })
  expect(calls.filter(({ command }) => command === 'eml.open')).toHaveLength(1)
  const openEvent = new CustomEvent('meron.frameKeyDown', {
    cancelable: true,
    detail: { key: 'o', mod: true, shift: true },
  })
  window.dispatchEvent(openEvent)
  expect(openEvent.defaultPrevented).toBe(true)
  expect(calls.filter(({ command }) => command === 'eml.open')).toEqual([
    { command: 'eml.open', payload: { session: 'reader-1' } },
    { command: 'eml.open', payload: { session: 'reader-1' } },
  ])
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
