import { afterEach, beforeEach, expect, test } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import type { Account } from '../../types'
import { McpApprovalDialog } from './McpApprovalDialog'

let calls: { command: string; payload: any }[]
let requests: any[]
let settle: (() => void) | undefined
let events: Record<string, () => void>
let failing = false
beforeEach(() => {
  calls = []
  settle = undefined
  failing = false
  accounts$.set([{ id: 'work', email: 'work@example.com' } as Account])
  requests = [
    {
      id: 'request-1',
      client: 'My assistant',
      account: 'work',
      tool: 'send_message',
      expires_at: new Date(Date.now() + 300000).toISOString(),
      preview: {
        kind: 'send',
        to: 'to@example.com',
        cc: 'cc@example.com',
        bcc: 'bcc@example.com',
        subject: 'The exact subject',
        body: '<script>untrusted email text</script>\nComplete message body.',
      },
    },
  ]
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
          if (command === 'mcp.pending') return requests
          if (command === 'mcp.resolve') {
            await new Promise<void>((resolve) => {
              settle = resolve
            })
            requests = []
            if (failing) return { status: 'failed', error: 'Sending failed: mailbox rejected the recipient' }
            return { status: payload.approve ? 'completed' : 'denied' }
          }
          return {}
        },
      },
    },
  }
})
afterEach(cleanup)

test('shows the complete send preview as text and approves once through Wails', async () => {
  const view = render(<McpApprovalDialog />)
  await view.findByRole('alertdialog')
  // The prompt arrives from a poll outside act(), so flush its effects
  // (initial focus, the Escape handler) before asserting on them.
  await act(async () => {})
  expect(view.getByText('My assistant')).toBeTruthy()
  expect(view.getByText('Account: work@example.com')).toBeTruthy()
  expect(view.getByText('Bcc: bcc@example.com')).toBeTruthy()
  expect(view.getByText('The exact subject')).toBeTruthy()
  expect(view.container.querySelector('script')).toBeNull()
  expect(view.container.textContent).toContain('<script>untrusted email text</script>')
  expect(calls.filter((c) => c.command === 'mcp.resolve')).toHaveLength(0)
  expect(document.activeElement).toBe(view.getByText('Reject'))
  fireEvent.click(view.getByText('Send message'))
  expect(calls.filter((c) => c.command === 'mcp.resolve')).toEqual([
    { command: 'mcp.resolve', payload: { id: 'request-1', approve: true } },
  ])
  expect((view.getByText('Working...') as HTMLButtonElement).disabled).toBe(true)
  await act(async () => settle?.())
  await waitFor(() => expect(view.queryByRole('alertdialog')).toBeNull())
})

test('shows exact deletion selection and Escape rejects it', async () => {
  requests[0].preview = {
    kind: 'delete',
    selection: {
      folder: 'Trash',
      messages: [{ uid: 7, subject: 'Delete only this', from: 'sender@example.com', date: 1700000000 }],
    },
  }
  const view = render(<McpApprovalDialog />)
  await view.findByRole('alertdialog')
  // The prompt arrives from a poll outside act(), so flush its effects
  // (initial focus, the Escape handler) before asserting on them.
  await act(async () => {})
  expect(view.getByText('Delete only this')).toBeTruthy()
  expect(view.getByText('Messages: 1')).toBeTruthy()
  expect(view.getByText('Folder: Trash')).toBeTruthy()
  fireEvent.keyDown(window, { key: 'Escape' })
  expect(calls.find((c) => c.command === 'mcp.resolve')?.payload).toEqual({ id: 'request-1', approve: false })
  await act(async () => settle?.())
  await waitFor(() => expect(view.queryByRole('alertdialog')).toBeNull())
})

test('keeps a failed operation visible after the request stops being pending', async () => {
  failing = true
  const view = render(<McpApprovalDialog />)
  await view.findByRole('alertdialog')
  await act(async () => {})
  fireEvent.click(view.getByText('Send message'))
  await act(async () => settle?.())
  // The backend no longer lists the operation; the reason must survive that.
  await waitFor(() => expect(view.getByText('Sending failed: mailbox rejected the recipient')).toBeTruthy())
  await act(async () => events['mcp.approvals']?.())
  expect(view.getByText('Sending failed: mailbox rejected the recipient')).toBeTruthy()
  expect(view.getByText('My assistant · send_message')).toBeTruthy()
  fireEvent.click(view.getByText('Close'))
  await waitFor(() => expect(view.queryByRole('alertdialog')).toBeNull())
})

test('a prompt arrives on the backend event, without polling for it', async () => {
  const waiting = requests
  requests = []
  const view = render(<McpApprovalDialog />)
  await act(async () => {})
  expect(view.queryByRole('alertdialog')).toBeNull()
  const asked = calls.length
  // Nothing is asked of the backend while there is nothing to approve.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 300))
  })
  expect(calls.length).toBe(asked)

  requests = waiting
  await act(async () => events['mcp.approvals']?.())
  await view.findByRole('alertdialog')
  expect(view.getByText('My assistant')).toBeTruthy()
})
