import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { ContactsView } from './ContactsView'
import { ui$ } from '../../states/ui'
import { contacts$, openContact } from '../../states/contacts'
import { AddressRow } from '../chat/AddressList'
import { mail$ } from '../../states/mail'
import type { Message } from '../../types'

afterEach(() => {
  cleanup()
  ui$.query.set('')
  ui$.selectedFolder.set('inbox')
  contacts$.selected.set(null)
  contacts$.pinned.set([])
  contacts$.sort.set('frequent')
  ui$.contactsOpen.set(false)
  ui$.calendarOpen.set(false)
  mail$.foldersByAccount.set({})
})

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 180))
  })
}

it('keeps directory rows mounted while loading more contacts', async () => {
  const contacts = Array.from({ length: 100 }, (_, i) => ({ name: `Person ${i}`, addr: `person${i}@example.com` }))
  let finish: ((value: any) => void) | undefined
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: any) => {
          if (method !== 'mail.suggestContacts') return {}
          if (params.limit === 100) return { contacts }
          return new Promise((resolve) => {
            finish = resolve
          })
        },
      },
    },
  }
  const view = render(<ContactsView />)
  await settle()
  const row = view.getByText('Person 0')
  fireEvent.click(view.getByRole('button', { name: 'Load more' }))
  await settle()
  expect(view.getByText('Person 0')).toBe(row)
  expect(finish).toBeDefined()
  await act(async () => finish?.({ contacts: [...contacts, { name: 'Next person', addr: 'next@example.com' }] }))
  expect(view.getByText('Next person')).toBeTruthy()
})

it('starts history over when returning to a previously paged contact', async () => {
  const calls: any[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: any) => {
          if (method !== 'mail.contactThreads') return { contacts: [], prefs: {} }
          calls.push(params)
          const subject = `${params.addr}-${params.before_cursor || 'first'}`
          return {
            threads: [
              {
                thread_id: subject,
                subject,
                from_addr: params.addr,
                folder_id: 'INBOX',
                account_id: 'acct',
                date: 1,
              } as Message,
            ],
            next_cursor: params.before_cursor ? '' : 'second',
          }
        },
      },
    },
  }
  openContact({ name: 'Alice', addr: 'alice@example.com' })
  const view = render(<ContactsView />)
  await settle()
  fireEvent.click(view.getByRole('button', { name: 'Load more' }))
  await settle()
  expect(view.getByTitle('alice@example.com-second')).toBeTruthy()
  act(() => openContact({ name: 'Bob', addr: 'bob@example.com' }))
  await settle()
  act(() => openContact({ name: 'Alice', addr: 'alice@example.com' }))
  await settle()
  expect(calls.at(-1)).toEqual({ addr: 'alice@example.com', before_cursor: '', limit: 50 })
  expect(view.getByTitle('alice@example.com-first')).toBeTruthy()
  expect(view.queryByTitle('alice@example.com-second')).toBeNull()
  expect(view.queryByTitle('bob@example.com-first')).toBeNull()
})

it('loads exact contact history without changing the mailbox navigation', async () => {
  const calls: Array<[string, any]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: any) => {
          calls.push([method, params])
          if (method === 'mail.suggestContacts') return { contacts: [{ name: 'Alice', addr: 'alice@example.com' }] }
          if (method === 'mail.contactThreads') return { threads: [], next_cursor: params.before_cursor ? '' : '50' }
          return {}
        },
      },
    },
  }
  ui$.query.set('subject:work')
  ui$.selectedFolder.set('Archive')
  const view = render(<ContactsView />)
  await settle()
  fireEvent.click(view.getByRole('button', { name: /Alice/ }))
  await settle()
  expect(calls.find(([method]) => method === 'mail.contactThreads')?.[1]).toEqual({
    addr: 'alice@example.com',
    before_cursor: '',
    limit: 50,
  })
  fireEvent.click(view.getByRole('button', { name: 'Load more' }))
  await settle()
  expect(calls.filter(([method]) => method === 'mail.contactThreads').at(-1)?.[1].before_cursor).toBe('50')
  expect(ui$.query.peek()).toBe('subject:work')
  expect(ui$.selectedFolder.peek()).toBe('Archive')
})

it('offers retry when the directory fails instead of claiming it is empty', async () => {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async () => {
          throw new Error('unavailable')
        },
      },
    },
  }
  const view = render(<ContactsView />)
  await settle()
  expect(view.getByRole('button', { name: 'Could not load. Try again' })).toBeTruthy()
  expect(view.queryByText('No contacts found in cached mail.')).toBeNull()
})

it('keeps pinned contacts available without cached mail and supports unpinning', async () => {
  contacts$.pinned.set([{ name: 'Alice', addr: 'alice@example.com' }])
  ;(window as any).go = { main: { App: { Invoke: async () => ({ contacts: [], threads: [] }) } } }
  const view = render(<ContactsView />)
  await settle()
  expect(view.getByText('Pinned')).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: /Alice/ }))
  await settle()
  fireEvent.click(view.getByRole('button', { name: 'Unpin contact' }))
  expect(contacts$.pinned.peek()).toEqual([])
  fireEvent.click(view.getByRole('button', { name: 'Back' }))
  await settle()
  expect(view.queryByText('Alice')).toBeNull()
})

it('opens a message address in Contacts without replacing copy-address behavior', () => {
  const view = render(<AddressRow label="From" rawList="Alice <alice@example.com>" />)
  expect(view.getByRole('button', { name: 'Copy name and email' })).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'Open contact' }))
  expect(ui$.contactsOpen.peek()).toBe(true)
  expect(contacts$.selected.peek()?.addr).toBe('alice@example.com')
})

it('can switch directly to another contact while the panel is open', async () => {
  const addresses: string[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: any) => {
          if (method === 'mail.contactThreads') addresses.push(params.addr)
          return { contacts: [], threads: [] }
        },
      },
    },
  }
  openContact({ name: 'Alice', addr: 'alice@example.com' })
  const view = render(<ContactsView />)
  await settle()
  act(() => openContact({ name: 'Bob', addr: 'bob@example.com' }))
  await settle()
  expect(addresses).toEqual(['alice@example.com', 'bob@example.com'])
  expect(view.getByRole('heading', { name: 'Bob' })).toBeTruthy()
})

it('asks the core for the chosen order and marks it in the sort menu', async () => {
  const sorts: string[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: any) => {
          if (method === 'mail.suggestContacts') sorts.push(params.sort)
          return { contacts: [], threads: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  await settle()
  fireEvent.click(view.getByRole('button', { name: 'Sort contacts: Most frequent' }))
  fireEvent.click(view.getByRole('button', { name: 'Name' }))
  await settle()
  expect(sorts).toEqual(['frequent', 'name'])
  expect(contacts$.sort.peek()).toBe('name')
  fireEvent.click(view.getByRole('button', { name: 'Sort contacts: Name' }))
  expect(view.getByRole('button', { name: 'Name' }).getAttribute('aria-pressed')).toBe('true')
})

it('closes the full calendar when reopening the active conversation from contact history', async () => {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string) => {
          if (method !== 'mail.contactThreads') return { contacts: [], prefs: {} }
          return {
            threads: [
              {
                thread_id: 't1',
                subject: 'Lunch',
                from_addr: 'alice@example.com',
                folder_id: 'INBOX',
                account_id: 'acct',
                date: 1,
              } as Message,
            ],
          }
        },
      },
    },
  }
  openContact({ name: 'Alice', addr: 'alice@example.com' })
  const view = render(<ContactsView />)
  await settle()
  fireEvent.click(view.getByTitle('Lunch'))
  // The same conversation again, its tab already active: no tab change
  // closes the calendar, so opening it must.
  ui$.calendarOpen.set(true)
  fireEvent.click(view.getByTitle('Lunch'))
  expect(ui$.calendarOpen.peek()).toBe(false)
})

it('names the folder of each conversation in contact history', async () => {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string) => {
          if (method !== 'mail.contactThreads') return { contacts: [], prefs: {} }
          return {
            threads: [
              {
                thread_id: 'a',
                subject: 'In inbox',
                from_addr: 'alice@example.com',
                folder_id: 'INBOX',
                account_id: 'acct',
                date: 2,
              },
              {
                thread_id: 'b',
                subject: 'Filed',
                from_addr: 'alice@example.com',
                folder_id: 'f1',
                account_id: 'acct',
                date: 1,
              },
            ] as Message[],
            next_cursor: '',
          }
        },
      },
    },
  }
  mail$.foldersByAccount.set({ acct: [{ id: 'f1', name: 'Projects/Now', delimiter: '/', account_id: 'acct' } as any] })
  openContact({ name: 'Alice', addr: 'alice@example.com' })
  const view = render(<ContactsView />)
  await settle()
  expect(view.getByTitle('Inbox')).toBeTruthy()
  expect(view.getByTitle('Now')).toBeTruthy()
})
