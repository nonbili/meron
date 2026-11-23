import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { SourceWindow } from './SourceWindow'

const previousGo = (window as any).go
afterEach(() => {
  cleanup()
  ;(window as any).go = previousGo
})

const SOURCE = {
  size: 5 * 1024 * 1024,
  source: 'From: Alice <alice@example.com>\r\nSubject: Hello\r\n\r\nBody text\r\n',
  truncated: true,
  headers: [
    { name: 'From', value: 'Alice <alice@example.com>' },
    { name: 'Subject', value: 'Hello' },
  ],
  shown: 2 * 1024 * 1024,
  auth: [
    { method: 'spf', result: 'pass', domain: 'example.com' },
    { method: 'dkim', result: 'pass', domain: 'example.com' },
    { method: 'dkim', result: 'fail', domain: 'mailer.example.net' },
  ],
  authServer: 'mx.example.org',
  lossy: false,
}

function backend(handle: (command: string, payload: any) => unknown) {
  const calls: { command: string; payload: any }[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'app.prefsGet') return { prefs: {} }
          if (command === 'system.appearance') return { dark: false }
          return handle(command, payload)
        },
      },
    },
  }
  return calls
}

it('shows the header summary, the verdicts and the source, and saves through its own session', async () => {
  const calls = backend((command) => (command === 'source.get' ? SOURCE : { saved: true }))
  const view = render(<SourceWindow session="source-1" />)
  await waitFor(() => expect(view.getByText('Body text', { exact: false })).toBeTruthy())
  expect(view.getByText('Alice <alice@example.com>')).toBeTruthy()
  expect(view.getByText('SPF (example.com): pass')).toBeTruthy()
  // Each signature has its own verdict.
  expect(view.getByText('DKIM (example.com): pass')).toBeTruthy()
  expect(view.getByText('DKIM (mailer.example.net): fail')).toBeTruthy()
  expect(view.queryByText(/DMARC/)).toBeNull()
  // The verdicts say who claims them.
  expect(view.getByText("Claimed in the headers as from mx.example.org. Meron doesn't verify these.")).toBeTruthy()
  expect(view.queryByText(/can't be shown or copied exactly/)).toBeNull()
  // Only part of a large message is shown; saving gets all of it.
  expect(view.getByText(/first 2(\.0)? MB of 5(\.0)? MB/)).toBeTruthy()

  fireEvent.click(view.getByRole('button', { name: 'Save as .eml' }))
  await waitFor(() => expect(calls.some((call) => call.command === 'source.save')).toBe(true))
  for (const call of calls.filter((call) => call.command.startsWith('source.')))
    expect(call.payload).toEqual({ session: 'source-1' })
})

it('says why when the server cannot be reached, and tries again', async () => {
  let fail = true
  backend((command) => {
    if (command !== 'source.get') return {}
    if (fail) throw new Error('sidecar messages.saveRaw error: offline')
    return SOURCE
  })
  const view = render(<SourceWindow session="source-2" />)
  await waitFor(() => expect(view.getByRole('alert').textContent).toContain('offline'))
  expect(view.getByRole('button', { name: 'Save as .eml' }).hasAttribute('disabled')).toBe(true)

  fail = false
  fireEvent.click(view.getByRole('button', { name: 'Retry' }))
  await waitFor(() => expect(view.getByText('Body text', { exact: false })).toBeTruthy())
  expect(view.queryByRole('alert')).toBeNull()
})

it('says when the shown text is not byte for byte the original', async () => {
  backend((command) => (command === 'source.get' ? { ...SOURCE, truncated: false, lossy: true, authServer: '' } : {}))
  const view = render(<SourceWindow session="source-3" />)
  await waitFor(() => expect(view.getByText(/can't be shown or copied exactly/)).toBeTruthy())
  expect(view.queryByText(/Claimed in the headers/)).toBeNull()
})

it('takes one save at a time', async () => {
  let finish = () => {}
  const calls = backend((command) => {
    if (command === 'source.get') return SOURCE
    if (command === 'source.save') return new Promise((resolve) => (finish = () => resolve({ saved: true })))
    return {}
  })
  const view = render(<SourceWindow session="source-4" />)
  const save = await waitFor(() => {
    const button = view.getByRole('button', { name: 'Save as .eml' })
    expect(button.hasAttribute('disabled')).toBe(false)
    return button
  })
  fireEvent.click(save)
  fireEvent.click(save)
  await waitFor(() => expect(save.hasAttribute('disabled')).toBe(true))
  expect(calls.filter((call) => call.command === 'source.save')).toHaveLength(1)
  finish()
  await waitFor(() => expect(save.hasAttribute('disabled')).toBe(false))
})
