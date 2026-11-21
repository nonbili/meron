import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { caldav$, setCalendarTasksEnabled, type CalendarAccount } from '../../states/caldav'
import { settleConfirm, ui$ } from '../../states/ui'
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
  display_name: 'Sam Rivera',
  avatar_url: '/media/sam.png',
  provider: 'gmail',
  auth_type: 'gmail_oauth',
}
const discoveredServer = 'https://dav.example.test/calendars/'
const googleCalendars: CalendarAccount = {
  id: 'google-calendars',
  server_url: 'https://dav.example.test/google/',
  username: google.email,
  google: true,
  // Unnamed: the core calls it by its mailbox's name and picture.
  display_name: '',
  name: google.display_name,
  avatar_url: google.avatar_url,
  last_sync_at: 0,
  last_error: '',
  sync_interval: 15,
  calendars: [
    { id: 'personal', name: 'Personal', color: '', read_only: false, enabled: true, events: true, tasks: false },
    { id: 'studio', name: 'Studio', color: '', read_only: false, enabled: true, events: true, tasks: false },
  ],
}

describe('finding calendars for existing mail accounts', () => {
  let calls: { method: string; payload: unknown }[]
  let server: string | null
  let calendarAccounts: CalendarAccount[]
  let connectedAccount: CalendarAccount | undefined
  let refreshedAccounts: CalendarAccount[] | undefined
  let previousGo: PropertyDescriptor | undefined

  beforeEach(() => {
    accounts$.set([mailbox, google, { ...mailbox, id: 'rss', email: 'feed@example.test', auth_type: 'rss' }])
    caldav$.accounts.set([])
    ui$.selectedAccount.set(mailbox.id)
    ui$.system.set({ gmail_oauth_configured: true } as SystemCheck)
    calls = []
    calendarAccounts = []
    connectedAccount = undefined
    refreshedAccounts = undefined
    server = discoveredServer
    previousGo = Object.getOwnPropertyDescriptor(window, 'go')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (method: string, payload: unknown) => {
            calls.push({ method, payload })
            if (method === 'googleTasks.accounts') return { accounts: [] }
            if (method === 'caldav.discoverServer') return { server_url: server }
            if (method === 'caldav.sync' && refreshedAccounts) calendarAccounts = refreshedAccounts
            if (method === 'caldav.connectGoogle' && connectedAccount) {
              calendarAccounts = [...calendarAccounts, connectedAccount]
              return { account_id: connectedAccount.id, accounts: calendarAccounts }
            }
            if (method === 'caldav.setCalendarEnabled') {
              const { calendar_id, enabled } = payload as { calendar_id: string; enabled: boolean }
              calendarAccounts = calendarAccounts.map((account) => ({
                ...account,
                calendars: account.calendars.map((calendar) =>
                  calendar.id === calendar_id ? { ...calendar, enabled } : calendar,
                ),
              }))
            }
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

  it('connects Google Tasks independently with the supplied account', async () => {
    const invoke = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (method: string, payload: unknown) => {
      if (method === 'googleTasks.connect') {
        calls.push({ method, payload })
        return { accounts: [{ email: google.email, last_error: '', last_sync_at: 0 }] }
      }
      return invoke(method, payload)
    }
    render(<CalendarAccountsPanel />)
    const section = within(screen.getByRole('heading', { name: 'Google Tasks' }).closest('section')!)
    fireEvent.change(section.getByRole('textbox', { name: 'Email Address' }), { target: { value: google.email } })
    await act(async () =>
      fireEvent.submit(section.getByRole('button', { name: 'Sign in with Google' }).closest('form')!),
    )
    expect(calls.filter(({ method }) => method === 'googleTasks.connect')).toEqual([
      { method: 'googleTasks.connect', payload: { login_hint: google.email } },
    ])
    expect(section.getByText(google.email)).toBeTruthy()
    expect(calls.some(({ method }) => method === 'caldav.connectGoogle')).toBe(false)
  })

  it('asks before discarding unsent CalDAV tasks when switching off a list', async () => {
    const invoke = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (method: string, payload: { discard_unsent?: boolean }) => {
      if (method === 'caldav.setTasksEnabled') {
        calls.push({ method, payload })
        return { accounts: [], unsent: payload.discard_unsent ? 0 : 2 }
      }
      return invoke(method, payload)
    }
    const disabling = setCalendarTasksEnabled('task-list', false)
    await waitFor(() => expect(ui$.confirm.peek()?.message).toContain('2 changes'))
    settleConfirm(true)
    await disabling
    expect(calls.filter(({ method }) => method === 'caldav.setTasksEnabled').map(({ payload }) => payload)).toEqual([
      { calendar_id: 'task-list', enabled: false },
      { calendar_id: 'task-list', enabled: false, discard_unsent: true },
    ])
  })

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

  it('cancels a connection to a server that does not answer', async () => {
    const invoke = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (method: string, payload: unknown) =>
      method === 'caldav.add' ? new Promise(() => {}) : invoke(method, payload)
    render(<CalendarAccountsPanel />)
    await find()
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'fictional-app-password' } })
    await act(async () => fireEvent.submit(screen.getByLabelText('Server').closest('form')!))
    expect(screen.getByRole('button', { name: 'Connecting…' })).toBeTruthy()
    const cancel = screen.getByRole('button', { name: 'Cancel' }) as HTMLButtonElement
    expect(cancel.disabled).toBe(false)
    fireEvent.click(cancel)
    // The form stays, as typed, to fix the address.
    expect((screen.getByLabelText('Server') as HTMLInputElement).value).toBe(discoveredServer)
    expect((screen.getByLabelText('Password') as HTMLInputElement).value).toBe('fictional-app-password')
    expect((screen.getByRole('button', { name: 'Connect' }) as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByLabelText('Server')).toBeNull()
  })

  it('falls back to manual setup while keeping the mailbox address filled in', async () => {
    server = null
    render(<CalendarAccountsPanel />)
    await find()
    expect(screen.getByRole('status').textContent).toContain('No calendar server was found')
    expect((screen.getByLabelText('Server') as HTMLInputElement).value).toBe('')
    expect((screen.getByLabelText('Username') as HTMLInputElement).value).toBe(mailbox.email)
  })

  it('uses the mailbox picker to authorize the right Google account when more than one is available', async () => {
    const otherGoogle = { ...google, id: 'other-google', email: 'jordan@example.test' }
    accounts$.set([mailbox, google, otherGoogle])
    ui$.selectedAccount.set(google.id)
    render(<CalendarAccountsPanel />)
    const picker = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Mail accounts' })
    expect(picker.value).toBe(google.id)
    expect(screen.queryByRole('button', { name: 'Connect Google Calendar' })).toBeNull()
    fireEvent.change(picker, { target: { value: otherGoogle.id } })
    expect(calls.some(({ method }) => method === 'caldav.connectGoogle')).toBe(false)
    await find()
    expect(calls.filter(({ method }) => method === 'caldav.connectGoogle')).toEqual([
      { method: 'caldav.connectGoogle', payload: { account_id: undefined, login_hint: otherGoogle.email } },
    ])
    expect(calls.some(({ method }) => method === 'caldav.discoverServer')).toBe(false)
  })

  it('uses Google consent for a Workspace mailbox configured with an app password', async () => {
    accounts$.set([{ ...mailbox, imap_host: 'imap.gmail.com' }])
    render(<CalendarAccountsPanel />)
    await find()
    expect(calls.filter(({ method }) => method === 'caldav.connectGoogle')).toEqual([
      { method: 'caldav.connectGoogle', payload: { account_id: undefined, login_hint: mailbox.email } },
    ])
    expect(calls.some(({ method }) => method === 'caldav.discoverServer')).toBe(false)
  })

  it('lets an abandoned Google sign-in be cancelled', async () => {
    let finish: (value: unknown) => void = () => {}
    ;(window as any).go.main.App.Invoke = async (method: string, payload: unknown) => {
      calls.push({ method, payload })
      if (method === 'caldav.connectGoogle') return new Promise((resolve) => (finish = resolve))
      return { accounts: calendarAccounts }
    }
    ui$.selectedAccount.set(google.id)
    render(<CalendarAccountsPanel />)
    await find()
    expect(screen.getByRole('status').textContent).toContain('Google')
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('status')).toBeNull()
    expect((screen.getByRole('button', { name: 'Add account' }) as HTMLButtonElement).disabled).toBe(false)
    // A sign-in finished after cancelling opens no picker.
    await act(async () => finish({ account_id: googleCalendars.id, accounts: [googleCalendars] }))
    expect(screen.queryByRole('dialog', { name: 'Choose calendars to sync' })).toBeNull()
  })

  it('opens a picker after authorization and saves each calendar choice', async () => {
    connectedAccount = googleCalendars
    ui$.selectedAccount.set(google.id)
    render(<CalendarAccountsPanel />)
    await find()

    const picker = within(screen.getByRole('dialog', { name: 'Choose calendars to sync' }))
    expect(picker.getByText(google.email)).toBeTruthy()
    expect(picker.getByText(google.display_name)).toBeTruthy()
    expect(picker.getByRole('img', { hidden: true }).getAttribute('src')).toBe(google.avatar_url!)
    const personal = picker.getByRole<HTMLInputElement>('checkbox', { name: 'Personal' })
    const studio = picker.getByRole<HTMLInputElement>('checkbox', { name: 'Studio' })
    expect(personal.checked).toBe(true)
    expect(studio.checked).toBe(true)
    await act(async () => fireEvent.click(studio))
    expect(calls).toContainEqual({
      method: 'caldav.setCalendarEnabled',
      payload: { calendar_id: 'studio', enabled: false },
    })
    expect(studio.checked).toBe(false)
    expect(personal.checked).toBe(true)
    fireEvent.click(picker.getByRole('button', { name: 'Done' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('opens the selected connected mailbox’s picker without repeating Google authorization', async () => {
    calendarAccounts = [
      { ...googleCalendars, id: 'other', username: 'jordan@example.test', calendars: [] },
      googleCalendars,
    ]
    ui$.selectedAccount.set(google.id)
    render(<CalendarAccountsPanel />)
    const choose = await screen.findByRole('button', { name: 'Choose calendars to sync' })
    fireEvent.click(choose)
    const picker = within(screen.getByRole('dialog', { name: 'Choose calendars to sync' }))
    expect(picker.getByText(google.email)).toBeTruthy()
    expect(picker.getAllByRole('checkbox')).toHaveLength(2)
    expect(calls.some(({ method }) => method === 'caldav.connectGoogle')).toBe(false)
  })

  it('shows each account by the name and avatar the core resolves', async () => {
    calendarAccounts = [
      googleCalendars,
      { ...googleCalendars, id: 'custom-name', display_name: 'Shared schedules', name: 'Shared schedules' },
    ]
    render(<CalendarAccountsPanel />)
    const card = await screen.findByRole('button', { name: google.display_name })
    expect(within(card).getByRole('img', { hidden: true }).getAttribute('src')).toBe('/media/sam.png')
    fireEvent.click(card)
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: 'Name' }).value).toBe(google.display_name)
    expect(screen.getByRole('button', { name: 'Shared schedules' })).toBeTruthy()
  })

  it('explains why discovery is limited to the primary calendar and refreshes the picker after sync', async () => {
    const warning = 'Enable the Google Calendar API to discover secondary calendars.'
    calendarAccounts = [
      { ...googleCalendars, calendars: googleCalendars.calendars.slice(0, 1), discovery_warning: warning },
    ]
    refreshedAccounts = [googleCalendars]
    ui$.selectedAccount.set(google.id)
    render(<CalendarAccountsPanel />)
    fireEvent.click(await screen.findByRole('button', { name: 'Choose calendars to sync' }))
    const picker = within(screen.getByRole('dialog', { name: 'Choose calendars to sync' }))
    expect(picker.getByRole('status').textContent).toBe(warning)
    expect(picker.getAllByRole('checkbox')).toHaveLength(1)
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Sync now' })))
    expect(calls).toContainEqual({ method: 'caldav.sync', payload: { account_id: googleCalendars.id } })
    expect(picker.queryByRole('status')).toBeNull()
    expect(picker.getAllByRole('checkbox')).toHaveLength(2)
  })

  it('shows the full sync error in the expanded account details', async () => {
    const error = 'calendar Holidays: reading the calendar change list: different collection path'
    calendarAccounts = [{ ...googleCalendars, last_error: error }]
    render(<CalendarAccountsPanel />)
    const card = await screen.findByRole('button', { name: google.display_name })
    expect(card.getAttribute('aria-expanded')).toBe('true')
    const detailsError = screen.getAllByText(`Sync failed: ${error}`).find((node) => node.tagName === 'P')
    expect(detailsError).toBeDefined()
    expect(detailsError?.closest('[hidden]')).toBeNull()
  })

  it('renames a writable CalDAV calendar through its visible Edit action', async () => {
    calendarAccounts = [
      {
        id: 'dav',
        server_url: discoveredServer,
        username: mailbox.email,
        google: false,
        display_name: 'Work account',
        name: 'Work account',
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
    fireEvent.click(edit)
    fireEvent.change(screen.getByRole('textbox', { name: 'Calendar name' }), { target: { value: 'Studio' } })
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Save' })))
    expect(calls).toContainEqual({ method: 'caldav.updateCalendar', payload: { calendar_id: 'work', name: 'Studio' } })
    expect(edit.getAttribute('aria-expanded')).toBe('false')
    fireEvent.click(account)
    expect(account.getAttribute('aria-expanded')).toBe('false')
    expect(screen.queryByRole('button', { name: 'Edit calendar' })).toBeNull()
  })

  it("changes a Google calendar's colour from its dot, with no edit button", async () => {
    calendarAccounts = [googleCalendars]
    render(<CalendarAccountsPanel />)
    fireEvent.click(await screen.findByRole('button', { name: google.display_name }))
    expect(screen.queryByRole('button', { name: 'Edit calendar' })).toBeNull()
    const [dot] = screen.getAllByRole('button', { name: 'Colour' })
    fireEvent.click(dot)
    expect(dot.getAttribute('aria-expanded')).toBe('true')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(dot.getAttribute('aria-expanded')).toBe('false')
    fireEvent.click(dot)
    fireEvent.pointerDown(document.body)
    expect(dot.getAttribute('aria-expanded')).toBe('false')
    fireEvent.click(dot)
    await act(async () => fireEvent.click(screen.getByRole('radio', { name: 'Blue' })))
    expect(calls).toContainEqual({
      method: 'caldav.updateCalendar',
      payload: { calendar_id: 'personal', color: '#3b82f6' },
    })
    expect(dot.getAttribute('aria-expanded')).toBe('false')
  })
})
