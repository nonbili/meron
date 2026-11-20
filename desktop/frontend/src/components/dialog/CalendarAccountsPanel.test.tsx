import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { caldav$ } from '../../states/caldav'
import { ui$ } from '../../states/ui'
import type { Account, SystemCheck } from '../../types'
import { CalendarAccountsPanel } from './CalendarAccountsPanel'

const mailbox: Account = {
  id: 'custom',
  email: 'alex@example.test',
  display_name: 'Alex',
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'mail.example.test',
  imap_port: 993,
  smtp_host: 'mail.example.test',
  smtp_port: 465,
  tls: true,
}
const google: Account = {
  ...mailbox,
  id: 'google',
  email: 'sam@example.test',
  provider: 'gmail',
  auth_type: 'gmail_oauth',
}
const discoveredServer = 'https://dav.example.test/calendars/'

describe('finding calendars for existing mail accounts', () => {
  let calls: { method: string; payload: unknown }[]
  let server: string | null
  let previousGo: PropertyDescriptor | undefined

  beforeEach(() => {
    accounts$.set([mailbox, google, { ...mailbox, id: 'rss', email: 'feed@example.test', auth_type: 'rss' }])
    caldav$.accounts.set([])
    ui$.selectedAccount.set(mailbox.id)
    ui$.system.set({ gmail_oauth_configured: true } as SystemCheck)
    calls = []
    server = discoveredServer
    previousGo = Object.getOwnPropertyDescriptor(window, 'go')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (method: string, payload: unknown) => {
            calls.push({ method, payload })
            if (method === 'caldav.discoverServer') return { server_url: server }
            return { accounts: [] }
          },
        },
      },
    }
  })

  afterEach(() => {
    cleanup()
    accounts$.set([])
    ui$.selectedAccount.set('')
    ui$.system.set(null)
    if (previousGo) Object.defineProperty(window, 'go', previousGo)
    else delete (window as any).go
  })

  async function find() {
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Find calendars for a mail account' })))
  }

  it('looks up only the selected mailbox and prefills the server and username', async () => {
    render(<CalendarAccountsPanel />)
    expect(screen.queryByRole('option', { name: 'feed@example.test' })).toBeNull()
    expect(calls.some(({ method }) => method === 'caldav.discoverServer')).toBe(false)
    await find()
    expect(calls.filter(({ method }) => method === 'caldav.discoverServer')).toEqual([
      { method: 'caldav.discoverServer', payload: { email: mailbox.email } },
    ])
    expect((screen.getByLabelText('Server') as HTMLInputElement).value).toBe(discoveredServer)
    expect((screen.getByLabelText('Username') as HTMLInputElement).value).toBe(mailbox.email)
    expect((screen.getByLabelText('Password') as HTMLInputElement).value).toBe('')
  })

  it('falls back to manual setup while keeping the mailbox address filled in', async () => {
    server = null
    render(<CalendarAccountsPanel />)
    await find()
    expect(screen.getByRole('status').textContent).toContain('No calendar server was found')
    expect((screen.getByLabelText('Server') as HTMLInputElement).value).toBe('')
    expect((screen.getByLabelText('Username') as HTMLInputElement).value).toBe(mailbox.email)
  })

  it('starts Google calendar consent with the selected mailbox address and no CalDAV lookup', async () => {
    render(<CalendarAccountsPanel />)
    fireEvent.change(screen.getByRole('combobox', { name: 'Mail accounts' }), { target: { value: google.id } })
    await act(async () => fireEvent.click(screen.getByRole('button', { name: /^Connect$/ })))
    expect(calls.filter(({ method }) => method === 'caldav.connectGoogle')).toEqual([
      { method: 'caldav.connectGoogle', payload: { account_id: undefined, login_hint: google.email } },
    ])
    expect(calls.some(({ method }) => method === 'caldav.discoverServer')).toBe(false)
  })

  it('uses Google consent for a Workspace mailbox configured with an app password', async () => {
    accounts$.set([{ ...mailbox, imap_host: 'imap.gmail.com' }])
    render(<CalendarAccountsPanel />)
    await act(async () => fireEvent.click(screen.getByRole('button', { name: /^Connect$/ })))
    expect(calls.filter(({ method }) => method === 'caldav.connectGoogle')).toEqual([
      { method: 'caldav.connectGoogle', payload: { account_id: undefined, login_hint: mailbox.email } },
    ])
    expect(calls.some(({ method }) => method === 'caldav.discoverServer')).toBe(false)
  })
})
