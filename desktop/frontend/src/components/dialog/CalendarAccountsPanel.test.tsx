import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { caldav$, type CalendarAccount } from '../../states/caldav'
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
  let calendarAccounts: CalendarAccount[]
  let previousGo: PropertyDescriptor | undefined

  beforeEach(() => {
    accounts$.set([mailbox, google, { ...mailbox, id: 'rss', email: 'feed@example.test', auth_type: 'rss' }])
    caldav$.accounts.set([])
    ui$.selectedAccount.set(mailbox.id)
    ui$.system.set({ gmail_oauth_configured: true } as SystemCheck)
    calls = []
    calendarAccounts = []
    server = discoveredServer
    previousGo = Object.getOwnPropertyDescriptor(window, 'go')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (method: string, payload: unknown) => {
            calls.push({ method, payload })
            if (method === 'caldav.discoverServer') return { server_url: server }
            return { accounts: calendarAccounts }
          },
        },
      },
    }
  })

  afterEach(() => {
    cleanup()
    accounts$.set([])
    caldav$.accounts.set([])
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

  it('renames a writable CalDAV calendar through its visible Edit action', async () => {
    calendarAccounts = [
      {
        id: 'dav',
        server_url: discoveredServer,
        username: mailbox.email,
        google: false,
        display_name: 'Work account',
        last_sync_at: 0,
        last_error: '',
        sync_interval: 15,
        calendars: [
          { id: 'work', name: 'Work', color: '', read_only: false, enabled: true, events: true, tasks: false },
        ],
      },
    ]
    render(<CalendarAccountsPanel />)
    const account = await screen.findByRole('button', { name: 'Work account' })
    expect(account.getAttribute('aria-expanded')).toBe('false')
    expect(screen.queryByRole('button', { name: 'Edit calendar' })).toBeNull()
    fireEvent.click(account)
    expect(account.getAttribute('aria-expanded')).toBe('true')
    const edit = screen.getByRole('button', { name: 'Edit calendar' })
    expect(edit.textContent).toBe('Edit')
    fireEvent.click(edit)
    fireEvent.change(screen.getByRole('textbox', { name: 'Calendar name' }), { target: { value: 'Studio' } })
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Save' })))
    expect(calls).toContainEqual({ method: 'caldav.updateCalendar', payload: { calendar_id: 'work', name: 'Studio' } })
    expect(edit.getAttribute('aria-expanded')).toBe('false')
    fireEvent.click(account)
    expect(account.getAttribute('aria-expanded')).toBe('false')
    expect(screen.queryByRole('button', { name: 'Edit calendar' })).toBeNull()
  })
})
