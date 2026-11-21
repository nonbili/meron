import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import type { Account, Folder } from '../../types'
import { SideNav } from './SideNav'

afterEach(cleanup)

it('marks participating inboxes read from the Unified Inbox menu with unread badges hidden', async () => {
  const showUnifiedInbox = settings$.showUnifiedInboxInSideNav.peek()
  const showUnreadBadge = settings$.showUnreadAccountBadge.peek()
  const previousAccounts = accounts$.peek()
  const previousFolders = mail$.foldersByAccount.peek()
  const selectedAccount = ui$.selectedAccount.peek()
  const previousGo = (window as any).go
  const account = (id: string): Account => ({
    id,
    email: `${id}@example.com`,
    display_name: id,
    provider: 'custom',
    auth_type: 'password',
    imap_host: '',
    imap_port: 993,
    smtp_host: '',
    smtp_port: 465,
    tls: true,
  })
  const inbox = (id: string): Folder => ({ id: 'INBOX', account_id: id, name: 'Inbox', role: 'inbox', unread: 3 })
  settings$.showUnifiedInboxInSideNav.set(true)
  settings$.showUnreadAccountBadge.set(false)
  accounts$.set([account('personal'), account('studio'), { ...account('other'), included_in_unified: false }])
  mail$.foldersByAccount.set({ personal: [inbox('personal')], studio: [inbox('studio')], other: [inbox('other')] })
  ui$.selectedAccount.set('other')
  const writes: Array<{ account_id: string; folder_id: string }> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: { account_id: string; folder_id: string }) => {
          if (command === 'mail.markAllRead') writes.push(payload)
          if (command === 'mail.folderList') return { folders: [{ ...inbox(payload.account_id), unread: 0 }] }
          return { folder_unreads: { [payload.account_id]: { INBOX: 0 } } }
        },
      },
    },
  }

  try {
    const view = render(<SideNav />)
    const unifiedInbox = view.getByTitle(/unified inbox/i)
    fireEvent.contextMenu(unifiedInbox)
    fireEvent.click(view.getByRole('button', { name: 'Mark all as read' }))

    await waitFor(() => {
      expect(writes).toEqual([
        { account_id: 'personal', folder_id: 'INBOX' },
        { account_id: 'studio', folder_id: 'INBOX' },
      ])
      expect(mail$.foldersByAccount.studio[0].unread.peek()).toBe(0)
    })
    expect(mail$.foldersByAccount.other[0].unread.peek()).toBe(3)
    expect(ui$.selectedAccount.peek()).toBe('other')
    fireEvent.contextMenu(unifiedInbox)
    expect((view.getByRole('button', { name: 'Mark all as read' }) as HTMLButtonElement).disabled).toBe(true)
  } finally {
    cleanup()
    settings$.showUnifiedInboxInSideNav.set(showUnifiedInbox)
    settings$.showUnreadAccountBadge.set(showUnreadBadge)
    accounts$.set(previousAccounts)
    mail$.foldersByAccount.set(previousFolders)
    ui$.selectedAccount.set(selectedAccount)
    if (previousGo === undefined) delete (window as any).go
    else (window as any).go = previousGo
  }
})

it('opens calendar settings from the Calendar context menu without hiding the tile', () => {
  const calendarEnabled = settings$.calendarEnabled.peek()
  const showCalendar = settings$.showCalendarInSideNav.peek()
  settings$.calendarEnabled.set(true)
  settings$.showCalendarInSideNav.set(true)
  try {
    const view = render(<SideNav />)
    fireEvent.contextMenu(view.getByRole('button', { name: 'Calendar' }))
    fireEvent.click(view.getByRole('button', { name: 'Manage calendars' }))
    expect(ui$.settingsOpen.peek()).toBe(true)
    expect(ui$.accountSettingsId.peek()).toBe('calendars')
    expect(settings$.showCalendarInSideNav.peek()).toBe(true)
    expect(view.queryByRole('button', { name: 'Manage calendars' })).toBeNull()
  } finally {
    act(() => {
      settings$.calendarEnabled.set(calendarEnabled)
      settings$.showCalendarInSideNav.set(showCalendar)
      ui$.settingsOpen.set(false)
      ui$.accountSettingsId.set('')
    })
  }
})

// The rail's tiles and selection indicator are sized in rem, so they grow with
// the Text size setting (the root font size). A px rail stays put around them:
// at 120% the 2.75rem tiles leave less room per side than the indicator is wide,
// and the two touch. The rail has to scale with them.
it('sizes the rail in rem, like the tiles and indicator inside it', () => {
  const view = render(<SideNav />)
  const rail = view.container.querySelector('aside')
  expect(rail).not.toBeNull()
  // happy-dom has no layout, so check the class: a spacing-scale width (w-15)
  // or an arbitrary rem one both scale; a px width is what must not come back.
  const width = rail!.className.split(/\s+/).find((name) => name.startsWith('w-'))
  expect(width).toMatch(/^w-(\d+(\.\d+)?|\[[0-9.]+rem\])$/)
})

describe('board unread badge', () => {
  const columns = [
    { accountId: 'acc-1', folderId: 'INBOX' },
    { accountId: 'acc-1', folderId: 'Later' },
  ]

  beforeEach(() => {
    mail$.foldersByAccount.set({
      'acc-1': [
        { id: 'INBOX', account_id: 'acc-1', name: 'INBOX', role: 'inbox', unread: 3 },
        { id: 'Later', account_id: 'acc-1', name: 'Later', role: '', unread: 4 },
      ],
    })
  })

  afterEach(() => {
    settings$.kanbanBoards.set([])
    mail$.foldersByAccount.set({})
  })

  it("shows the unread count of the board's folder columns on its tile when switched on", () => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Work', columns, showUnreadBadge: true }])
    const view = render(<SideNav />)
    const tile = view.getByTitle('Work', { exact: false })
    expect(tile.textContent).toBe('7')
  })

  it('updates the count when the folder counts or the switch change', () => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Work', columns }])
    const view = render(<SideNav />)
    act(() => settings$.kanbanBoards.set([{ id: 'b1', name: 'Work', columns, showUnreadBadge: true }]))
    expect(view.getByTitle('Work', { exact: false }).textContent).toBe('7')
    act(() =>
      mail$.foldersByAccount['acc-1'].set([
        { id: 'INBOX', account_id: 'acc-1', name: 'INBOX', role: 'inbox', unread: 1 },
      ]),
    )
    expect(view.getByTitle('Work', { exact: false }).textContent).toBe('1')
  })

  it('shows no count on a board whose switch is off', () => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Work', columns }])
    const view = render(<SideNav />)
    expect(view.getByTitle('Work', { exact: false }).textContent).toBe('')
  })
})
