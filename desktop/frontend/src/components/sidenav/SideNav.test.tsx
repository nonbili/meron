import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { mail$ } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { SideNav } from './SideNav'

afterEach(cleanup)

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
