import { afterEach, beforeEach, expect, test } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { settleConfirm, ui$ } from '../../states/ui'
import type { Account } from '../../types'
import { McpSettingsPanel } from './McpSettingsPanel'

const account = (id: string) =>
  ({ id, email: `${id}@example.com`, display_name: id, provider: 'custom', auth_type: 'password' }) as Account
let calls: { command: string; payload: any }[]
let events: Record<string, () => void>
let status: any
beforeEach(() => {
  calls = []
  accounts$.set([account('work'), account('personal')])
  ui$.confirm.set(null)
  status = {
    enabled: true,
    running: true,
    port: 43827,
    url: 'http://127.0.0.1:43827/mcp',
    clients: [],
    activity: [],
    error: '',
  }
  events = {}
  ;(window as any).runtime = {
    EventsOn: (name: string, handler: () => void) => {
      events[name] = handler
      return () => delete events[name]
    },
  }
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'mcp.clientSave') {
            status = { ...status, clients: [{ ...payload, id: 'client' }] }
            return { status, token: 'one-time-secret' }
          }
          if (command === 'mcp.clientRevoke') status = { ...status, clients: [] }
          if (command === 'mcp.clientRegenerate') return { status, token: 'second-secret' }
          if (command === 'mcp.setPort')
            status = { ...status, port: payload.port, url: `http://127.0.0.1:${payload.port}/mcp` }
          return status
        },
      },
    },
  }
})
afterEach(cleanup)

// The approval form is a step, not a permanent section: nothing about a client
// is on screen until the user asks to approve one.
async function openEditor(view: ReturnType<typeof render>) {
  const add = await view.findByText('Add a client')
  expect(view.queryByLabelText('Client name')).toBeNull()
  fireEvent.click(add)
  return view.getByLabelText('Client name')
}

test('the editor is a sub-page and leaving it saves nothing', async () => {
  const view = render(<McpSettingsPanel />)
  const name = await openEditor(view)
  // The sub-page replaces the panel rather than growing it.
  expect(view.queryByText('Add a client', { selector: 'button' })).toBeNull()
  expect(view.queryByText('Recent activity since launch')).toBeNull()
  expect(view.queryByRole('spinbutton')).toBeNull()
  fireEvent.change(name, { target: { value: 'Assistant' } })
  fireEvent.click(view.getByLabelText('Back'))
  expect(calls.some((call) => call.command === 'mcp.clientSave')).toBe(false)
  expect(view.getByText('Recent activity since launch')).toBeTruthy()
  // A cancelled draft does not survive into the next approval.
  fireEvent.click(view.getByText('Add a client'))
  expect((view.getByLabelText('Client name') as HTMLInputElement).value).toBe('')
})

test('the port is editable and only committed when it really changes', async () => {
  const view = render(<McpSettingsPanel />)
  const port = (await view.findByRole('spinbutton')) as HTMLInputElement
  expect(port.value).toBe('43827')
  fireEvent.blur(port)
  expect(calls.some((call) => call.command === 'mcp.setPort')).toBe(false)
  fireEvent.change(port, { target: { value: '80' } })
  fireEvent.blur(port)
  expect(calls.some((call) => call.command === 'mcp.setPort')).toBe(false)
  fireEvent.change(port, { target: { value: '51000' } })
  fireEvent.blur(port)
  await waitFor(() => expect(calls.find((call) => call.command === 'mcp.setPort')?.payload).toEqual({ port: 51000 }))
  await waitFor(() => expect(view.getByText('http://127.0.0.1:51000/mcp')).toBeTruthy())
})

test('approval starts with no accounts and read only, and shows a credential once', async () => {
  const view = render(<McpSettingsPanel />)
  const name = await openEditor(view)
  const work = view.getByRole('switch', { name: 'work@example.com' })
  const drafts = view.getByRole('switch', { name: 'Draft' })
  expect((drafts.closest('fieldset') as HTMLFieldSetElement).disabled).toBe(true)
  expect(work.getAttribute('aria-checked')).toBe('false')
  expect(drafts.getAttribute('aria-checked')).toBe('false')
  fireEvent.change(name, { target: { value: 'Assistant' } })
  fireEvent.click(work)
  expect((drafts.closest('fieldset') as HTMLFieldSetElement).disabled).toBe(false)
  fireEvent.click(view.getByText('Create credential'))
  await waitFor(() => expect(view.getByLabelText('Client configuration')).toBeTruthy())
  const saved = calls.find((call) => call.command === 'mcp.clientSave')!
  expect(saved.payload).toEqual({
    id: '',
    name: 'Assistant',
    accounts: ['work'],
    all_accounts: false,
    drafts: false,
    manage_accounts: false,
    manage_settings: false,
    organize: false,
    send: false,
    delete: false,
    send_without_confirmation: false,
    delete_without_confirmation: false,
  })
  expect((view.getByLabelText('Client configuration') as HTMLTextAreaElement).value).toContain('Bearer one-time-secret')
  // The form closes on approval; the new client is listed with its permissions.
  expect(view.queryByLabelText('Client name')).toBeNull()
  expect(view.getByText('Assistant')).toBeTruthy()
  expect(view.getByText('work@example.com')).toBeTruthy()
  fireEvent.click(view.getByText('Hide secret'))
  expect(view.queryByLabelText('Client configuration')).toBeNull()
})

test('a setup-only client can be approved without mail accounts', async () => {
  accounts$.set([])
  const view = render(<McpSettingsPanel />)
  const name = await openEditor(view)
  fireEvent.change(name, { target: { value: 'Setup' } })
  expect((view.getByText('Create credential') as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(view.getByRole('switch', { name: 'Manage accounts' }))
  fireEvent.click(view.getByText('Create credential'))
  await waitFor(() => expect(view.getByLabelText('Client configuration')).toBeTruthy())
  expect(calls.find((call) => call.command === 'mcp.clientSave')?.payload).toMatchObject({
    accounts: [],
    manage_accounts: true,
    manage_settings: false,
  })
})

test('all-account scope hides selections and allows approval before accounts exist', async () => {
  accounts$.set([])
  const view = render(<McpSettingsPanel />)
  const name = await openEditor(view)
  fireEvent.change(name, { target: { value: 'Every account' } })
  fireEvent.click(view.getByRole('switch', { name: 'All accounts, including future accounts' }))
  expect(view.queryByText('Select all current accounts')).toBeNull()
  fireEvent.click(view.getByText('Create credential'))
  await waitFor(() => expect(view.getByLabelText('Client configuration')).toBeTruthy())
  expect(calls.find((call) => call.command === 'mcp.clientSave')?.payload).toMatchObject({
    accounts: [],
    all_accounts: true,
    drafts: false,
    send: false,
    delete: false,
    manage_accounts: false,
    manage_settings: false,
  })
  expect(view.getByText('All accounts, including future accounts')).toBeTruthy()
})

test('switching back from all accounts preserves individual selections', async () => {
  const view = render(<McpSettingsPanel />)
  await openEditor(view)
  fireEvent.click(view.getByRole('switch', { name: 'work@example.com' }))
  const scope = view.getByRole('switch', { name: 'All accounts, including future accounts' })
  fireEvent.click(scope)
  expect(view.queryByRole('switch', { name: 'work@example.com' })).toBeNull()
  fireEvent.click(scope)
  expect(view.getByRole('switch', { name: 'work@example.com' }).getAttribute('aria-checked')).toBe('true')
  expect(view.getByRole('switch', { name: 'personal@example.com' }).getAttribute('aria-checked')).toBe('false')
})

test('revoking asks first and only then drops the grant', async () => {
  status = { ...status, clients: [{ ...emptyGrant(), id: 'client', name: 'Assistant', accounts: ['work'] }] }
  const view = render(<McpSettingsPanel />)
  fireEvent.click(await view.findByLabelText('Revoke'))
  await waitFor(() => expect(ui$.confirm.get()?.title).toBe('Revoke Assistant?'))
  await act(async () => settleConfirm(false))
  expect(calls.some((call) => call.command === 'mcp.clientRevoke')).toBe(false)

  fireEvent.click(view.getByLabelText('Revoke'))
  await waitFor(() => expect(ui$.confirm.get()).toBeTruthy())
  await act(async () => settleConfirm(true))
  await waitFor(() => expect(view.getByText('No clients have access.')).toBeTruthy())
  expect(calls.find((call) => call.command === 'mcp.clientRevoke')?.payload).toEqual({ id: 'client' })
})

test('replacing a credential keeps the grant and shows the new secret once', async () => {
  const grant = { ...emptyGrant(), id: 'client', name: 'Assistant', accounts: ['work'], drafts: true }
  status = { ...status, clients: [grant] }
  const view = render(<McpSettingsPanel />)
  fireEvent.click(await view.findByLabelText('Replace credential'))
  await waitFor(() => expect(ui$.confirm.get()?.title).toBe('Replace the credential for Assistant?'))
  await act(async () => settleConfirm(false))
  expect(calls.some((call) => call.command === 'mcp.clientRegenerate')).toBe(false)

  fireEvent.click(view.getByLabelText('Replace credential'))
  await waitFor(() => expect(ui$.confirm.get()).toBeTruthy())
  await act(async () => settleConfirm(true))
  await waitFor(() => expect(view.getByLabelText('Client configuration')).toBeTruthy())
  expect(calls.find((call) => call.command === 'mcp.clientRegenerate')?.payload).toEqual({ id: 'client' })
  expect((view.getByLabelText('Client configuration') as HTMLTextAreaElement).value).toContain('Bearer second-secret')
  // The grant itself is untouched: same client, same accounts, same permissions.
  expect(view.getByText('Assistant')).toBeTruthy()
  expect(view.getByText('work@example.com')).toBeTruthy()
  expect(view.getByText('Draft')).toBeTruthy()
})

test('select all includes only current accounts', async () => {
  const view = render(<McpSettingsPanel />)
  await openEditor(view)
  fireEvent.click(view.getByText('Select all current accounts'))
  await act(async () => accounts$.set([...accounts$.peek(), account('new')]))
  expect(view.getByRole('switch', { name: 'work@example.com' }).getAttribute('aria-checked')).toBe('true')
  expect(view.getByRole('switch', { name: 'new@example.com' }).getAttribute('aria-checked')).toBe('false')
})

test('all write permissions are independent and confirmation defaults to asking', async () => {
  const view = render(<McpSettingsPanel />)
  await openEditor(view)
  expect(view.getByText('Always')).toBeTruthy()
  const send = view.getByRole('switch', { name: 'Send' })
  const organize = view.getByRole('switch', { name: 'Organize' })
  const deletion = view.getByRole('switch', { name: 'Permanently delete' })
  expect(send.getAttribute('aria-checked')).toBe('false')
  expect(organize.getAttribute('aria-checked')).toBe('false')
  expect(deletion.getAttribute('aria-checked')).toBe('false')
  // A confirmation choice only exists once the permission it belongs to is on.
  expect(view.queryByText('Sending confirmation')).toBeNull()
  fireEvent.click(send)
  expect(view.getByText('Sending confirmation')).toBeTruthy()
  expect(selected(view, 'Ask every time')).toBe(true)
  fireEvent.click(view.getByText('Allow without asking'))
  expect(selected(view, 'Allow without asking')).toBe(true)
  // Turning the permission off and on again returns to asking.
  fireEvent.click(send)
  fireEvent.click(send)
  expect(selected(view, 'Ask every time')).toBe(true)
  expect(deletion.getAttribute('aria-checked')).toBe('false')
  expect(view.queryByText('Permanent deletion confirmation')).toBeNull()
  fireEvent.click(deletion)
  expect(view.getByText('Permanent deletion confirmation')).toBeTruthy()
  expect(organize.getAttribute('aria-checked')).toBe('false')
})

function selected(view: ReturnType<typeof render>, label: string) {
  return view.getByText(label).className.includes('text-accent')
}

function emptyGrant() {
  return {
    accounts: [],
    drafts: false,
    organize: false,
    send: false,
    delete: false,
    send_without_confirmation: false,
    delete_without_confirmation: false,
  }
}
