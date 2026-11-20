import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { ContactsView } from './ContactsView'
import { settleConfirm, ui$ } from '../../states/ui'
import { contactNotes$, contacts$, openContact } from '../../states/contacts'
import { AddressRow } from '../chat/AddressList'
import { mail$ } from '../../states/mail'
import { accounts$ } from '../../states/accounts'
import type { Account, Message, SystemCheck } from '../../types'

afterEach(() => {
  cleanup()
  ui$.query.set('')
  ui$.selectedFolder.set('inbox')
  contacts$.selected.set(null)
  contacts$.pinned.set([])
  contacts$.sort.set('frequent')
  ui$.contactsOpen.set(false)
  ui$.calendarOpen.set(false)
  ui$.system.set(null)
  mail$.foldersByAccount.set({})
  accounts$.set([])
  for (const key of Object.keys(contactNotes$.peek())) contactNotes$[key].delete()
})

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 180))
  })
}

it('reconnects Google Contacts using the connected account as the login hint', async () => {
  ui$.system.set({ gmail_oauth_configured: true } as SystemCheck)
  const calls: Array<[string, Record<string, unknown>]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts')
            return { accounts: [{ id: 'google-book', google: true, username: 'cora@example.test', books: [] }] }
          if (method === 'carddav.connectGoogle') return { account_id: 'google-book' }
          return { contacts: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(view.getByRole('button', { name: 'Address books' }))
  const reconnect = await view.findByRole('button', { name: 'Reconnect' })
  await act(async () => fireEvent.click(reconnect))
  expect(calls).toContainEqual([
    'carddav.connectGoogle',
    { account_id: 'google-book', login_hint: 'cora@example.test' },
  ])
})

it('connects an address book and shows the synced contact in the directory', async () => {
  const calls: Array<[string, Record<string, unknown>]> = []
  let synced = false
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts') return { accounts: [] }
          if (method === 'carddav.add') return { account_id: 'book-account' }
          if (method === 'carddav.sync') synced = true
          return { contacts: synced ? [{ name: 'Cora Fixture', addr: 'cora@example.test' }] : [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(view.getByRole('button', { name: 'Address books' }))
  fireEvent.click(view.getByRole('button', { name: 'Enter server' }))
  fireEvent.change(view.getByLabelText('CardDAV server URL'), { target: { value: 'https://dav.example.test' } })
  fireEvent.change(view.getByLabelText('Username'), { target: { value: 'cora@example.test' } })
  fireEvent.change(view.getByLabelText('Password'), { target: { value: 'fictional-app-password' } })
  await act(async () => fireEvent.submit(view.getByRole('button', { name: 'Connect' }).closest('form')!))
  expect(calls).toContainEqual(['carddav.sync', { account_id: 'book-account' }])
  fireEvent.click(view.getByRole('button', { name: 'Address books' }))
  await settle()
  expect(view.getByText('Cora Fixture')).toBeTruthy()
})

it("finds a mail account's address book and signs in with its saved password", async () => {
  accounts$.set([
    {
      id: 'mail-1',
      email: 'cora@example.test',
      display_name: 'Cora',
      provider: 'custom',
      auth_type: 'password',
      imap_host: 'mail.example.test',
      imap_port: 993,
      smtp_host: 'mail.example.test',
      smtp_port: 465,
      tls: true,
    } as Account,
  ])
  const calls: Array<[string, Record<string, unknown>]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts') return { accounts: [] }
          if (method === 'carddav.discoverServer') return { server_url: 'https://dav.example.test/' }
          if (method === 'carddav.add') return { account_id: 'book-account' }
          return { contacts: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(view.getByRole('button', { name: 'Address books' }))
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Find address book' })))
  expect(calls).toContainEqual(['carddav.discoverServer', { email: 'cora@example.test' }])
  expect((view.getByLabelText('CardDAV server URL') as HTMLInputElement).value).toBe('https://dav.example.test/')
  expect((view.getByLabelText('Username') as HTMLInputElement).value).toBe('cora@example.test')
  // A blank password here is the mail account's: the warning says so.
  const server = view.getByLabelText('CardDAV server URL')
  fireEvent.change(server, { target: { value: 'http://192.0.2.10/' } })
  expect(view.getByRole('note').textContent).toContain('mail account’s password')
  fireEvent.change(server, { target: { value: 'https://dav.example.test/' } })
  expect(view.queryByRole('note')).toBeNull()
  await act(async () => fireEvent.submit(view.getByRole('button', { name: 'Connect' }).closest('form')!))
  expect(calls).toContainEqual([
    'carddav.add',
    {
      server_url: 'https://dav.example.test/',
      username: 'cora@example.test',
      password: '',
      mail_account_id: 'mail-1',
    },
  ])
})

it('lets an abandoned Google sign-in be cancelled', async () => {
  ui$.system.set({ gmail_oauth_configured: true } as SystemCheck)
  accounts$.set([
    {
      id: 'gmail-1',
      email: 'cora@example.test',
      display_name: 'Cora',
      provider: 'gmail',
      auth_type: 'gmail_oauth',
      imap_host: 'imap.gmail.com',
      imap_port: 993,
      smtp_host: 'smtp.gmail.com',
      smtp_port: 465,
      tls: true,
    } as Account,
  ])
  const calls: Array<[string, Record<string, unknown>]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts') return { accounts: [] }
          // The browser tab was closed: sign-in never answers.
          if (method === 'carddav.connectGoogle') return new Promise(() => {})
          return { contacts: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(view.getByRole('button', { name: 'Address books' }))
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Find address book' })))
  expect(calls).toContainEqual(['carddav.connectGoogle', { account_id: undefined, login_hint: 'cora@example.test' }])
  expect(view.getByRole('status').textContent).toContain('Google')
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Cancel' })))
  expect(view.queryByRole('status')).toBeNull()
  expect((view.getByRole('button', { name: 'Find address book' }) as HTMLButtonElement).disabled).toBe(false)
  expect(view.queryByRole('button', { name: 'Google account' })).toBeNull()
})

it('cancels a connection to a server that does not answer', async () => {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string) => {
          if (method === 'carddav.accounts') return { accounts: [] }
          if (method === 'carddav.add') return new Promise(() => {})
          return { contacts: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(view.getByRole('button', { name: 'Address books' }))
  fireEvent.click(view.getByRole('button', { name: 'Enter server' }))
  expect(view.queryByRole('note')).toBeNull()
  fireEvent.change(view.getByLabelText('CardDAV server URL'), { target: { value: 'http://192.0.2.10' } })
  expect(view.getByRole('note').textContent).toContain('the password is sent as plain text')
  fireEvent.change(view.getByLabelText('Username'), { target: { value: 'cora@example.test' } })
  fireEvent.change(view.getByLabelText('Password'), { target: { value: 'fictional-app-password' } })
  await act(async () => fireEvent.submit(view.getByRole('button', { name: 'Connect' }).closest('form')!))
  expect(view.getByRole('button', { name: 'Connecting…' })).toBeTruthy()
  const cancel = view.getByRole('button', { name: 'Cancel' }) as HTMLButtonElement
  expect(cancel.disabled).toBe(false)
  await act(async () => fireEvent.click(cancel))
  // The form stays, as typed, to fix the address.
  const server = view.getByLabelText('CardDAV server URL') as HTMLInputElement
  expect(server.value).toBe('http://192.0.2.10')
  expect((view.getByRole('button', { name: 'Connect' }) as HTMLButtonElement).disabled).toBe(false)
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Cancel' })))
  expect(view.queryByLabelText('CardDAV server URL')).toBeNull()
})

const bookAccounts = {
  accounts: [
    {
      id: 'dav',
      username: 'cora@example.test',
      server: 'https://dav.example.test/',
      books: [
        { id: 'work', name: 'Work', enabled: true },
        { id: 'family', name: 'Family', enabled: true },
        { id: 'old', name: 'Old', enabled: false },
      ],
    },
  ],
}

it('creates a contact from the list in the chosen address book', async () => {
  const calls: Array<[string, Record<string, unknown>]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts') return bookAccounts
          if (method === 'carddav.cardsFor') return { cards: [] }
          if (method === 'carddav.createContact') return {}
          return { contacts: [], threads: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(await view.findByRole('button', { name: 'New contact' }))
  // Only enabled address books can take it.
  expect(view.getAllByRole('option').map((option) => option.textContent)).toEqual([
    'Work · cora@example.test',
    'Family · cora@example.test',
  ])
  fireEvent.change(view.getByRole('combobox'), { target: { value: 'family' } })
  fireEvent.change(view.getByLabelText('Name'), { target: { value: 'Ada Lane' } })
  fireEvent.change(view.getByLabelText('Email addresses'), { target: { value: ' ada@example.test ' } })
  fireEvent.click(view.getByRole('button', { name: 'Add email' }))
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Save' })))
  const created = calls.find(([method]) => method === 'carddav.createContact')![1]
  expect(created).toMatchObject({ book_id: 'family', name: 'Ada Lane', emails: ['ada@example.test'] })
  expect(typeof created.creation_id).toBe('string')
  // It opens on the new contact.
  expect(view.getByRole('heading', { name: 'Ada Lane' })).toBeTruthy()
})

it('edits and deletes a saved contact from its page', async () => {
  const card = {
    book_id: 'work',
    book_name: 'Work',
    id: 'cora.vcf',
    version: '"v1"',
    name: 'Cora Fixture',
    emails: ['cora@example.test'],
    account: 'cora@dav.example.test',
    google: false,
    note: 'Met at the fair',
  }
  let saved = true
  const calls: Array<[string, Record<string, unknown>]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts') return bookAccounts
          if (method === 'carddav.cardsFor') return { cards: saved ? [card] : [] }
          if (method === 'carddav.deleteContact') saved = false
          if (method === 'contacts.noteGet') return { prefs: {} }
          return { contacts: [], threads: [] }
        },
      },
    },
  }
  await act(async () => openContact({ name: 'Cora Fixture', addr: 'cora@example.test' }))
  const view = render(<ContactsView />)
  // The page says where the contact is saved.
  expect(await view.findByText('Work')).toBeTruthy()
  expect(view.getByText('cora@dav.example.test')).toBeTruthy()
  expect(view.getByText('Met at the fair')).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'Edit contact' }))
  expect(view.getByText('Saved in Work · cora@dav.example.test')).toBeTruthy()
  fireEvent.change(view.getByLabelText('Name'), { target: { value: 'Cora Lane' } })
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Save' })))
  expect(calls).toContainEqual([
    'carddav.updateContact',
    {
      book_id: 'work',
      contact_id: 'cora.vcf',
      version: '"v1"',
      name: 'Cora Lane',
      emails: ['cora@example.test'],
    },
  ])
  expect(view.getByRole('heading', { name: 'Cora Lane' })).toBeTruthy()

  fireEvent.click(await view.findByRole('button', { name: 'Delete contact' }))
  await waitFor(() => expect(ui$.confirm.peek()).not.toBeNull())
  expect(calls.some(([method]) => method === 'carddav.deleteContact')).toBe(false)
  await act(async () => settleConfirm(true))
  expect(calls).toContainEqual(['carddav.deleteContact', { book_id: 'work', contact_id: 'cora.vcf', version: '"v1"' }])
  // Still in mail, so the page stays and offers to save it again.
  expect(await view.findByText('Only in your mail')).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'Save to address book' }))
  expect((view.getByLabelText('Name') as HTMLInputElement).value).toBe('Cora Lane')
  expect((view.getByLabelText('Email addresses') as HTMLInputElement).value).toBe('cora@example.test')
})

function noteFixture(serverNote: string, failWrites = 0, deviceNote = 'Prefers calls', books = 1) {
  const card = {
    book_id: 'work',
    book_name: 'Work',
    id: 'cora.vcf',
    version: '"v1"',
    name: 'Cora Fixture',
    emails: ['cora@example.test'],
    account: 'cora@dav.example.test',
    google: false,
    note: serverNote,
  }
  const state = { deviceNote, failWrites }
  const calls: Array<[string, Record<string, unknown>]> = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.accounts') return bookAccounts
          if (method === 'carddav.cardsFor')
            return { cards: books > 1 ? [card, { ...card, book_id: 'family', book_name: 'Family' }] : [card] }
          if (method === 'carddav.updateContact') {
            if (state.failWrites-- > 0) throw new Error('This contact changed. Reopen it and try again.')
            card.note = params.note as string
            return {}
          }
          if (method === 'contacts.noteGet') return { prefs: { 'contact_note:cora@example.test': state.deviceNote } }
          if (method === 'contacts.noteSet') state.deviceNote = params.value as string
          return { contacts: [], threads: [] }
        },
      },
    },
  }
  return { calls, state }
}

const updates = (calls: Array<[string, Record<string, unknown>]>) =>
  calls.filter(([method]) => method === 'carddav.updateContact').map(([, params]) => params.note)

async function moveNote(view: ReturnType<typeof render>, book: string, confirm = true) {
  fireEvent.click(await view.findByRole('button', { name: `Move to ${book}` }))
  await waitFor(() => expect(ui$.confirm.peek()?.message).toContain('anyone with access'))
  await act(async () => settleConfirm(confirm))
}

it('labels where each note is kept and moves the device note only when asked', async () => {
  const { calls, state } = noteFixture('From the server')
  await act(async () => openContact({ name: 'Cora Fixture', addr: 'cora@example.test' }))
  const view = render(<ContactsView />)
  expect(await view.findByRole('heading', { name: 'Note only on this device' })).toBeTruthy()
  expect(view.getByRole('heading', { name: 'Work · cora@dav.example.test' })).toBeTruthy()
  expect(view.getByText('From the server')).toBeTruthy()
  await settle()
  // Nothing leaves the device by itself, nor when the move is declined.
  expect(updates(calls)).toEqual([])
  await moveNote(view, 'Work', false)
  expect(updates(calls)).toEqual([])
  await moveNote(view, 'Work')
  await waitFor(() => expect(state.deviceNote).toBe(''))
  expect(updates(calls)).toEqual(['From the server\n\nPrefers calls'])
  await waitFor(() => expect(view.queryByRole('heading', { name: 'Note only on this device' })).toBeNull())
})

it('fills an empty card note with the moved device note', async () => {
  const { calls, state } = noteFixture('')
  await act(async () => openContact({ name: 'Cora Fixture', addr: 'cora@example.test' }))
  const view = render(<ContactsView />)
  await moveNote(view, 'Work')
  await waitFor(() => expect(state.deviceNote).toBe(''))
  expect(updates(calls)).toEqual(['Prefers calls'])
})

it('offers each address book holding the contact as a destination', async () => {
  const { calls } = noteFixture('', 0, 'Prefers calls', 2)
  await act(async () => openContact({ name: 'Cora Fixture', addr: 'cora@example.test' }))
  const view = render(<ContactsView />)
  expect(await view.findByRole('button', { name: 'Move to Family' })).toBeTruthy()
  await moveNote(view, 'Family')
  expect(calls.find(([method]) => method === 'carddav.updateContact')?.[1]).toMatchObject({ book_id: 'family' })
})

it('keeps the device note when the server refuses the move', async () => {
  const { calls, state } = noteFixture('', 1)
  await act(async () => openContact({ name: 'Cora Fixture', addr: 'cora@example.test' }))
  const view = render(<ContactsView />)
  await moveNote(view, 'Work')
  await settle()
  expect(updates(calls)).toEqual(['Prefers calls'])
  expect(state.deviceNote).toBe('Prefers calls')
  expect(view.getByRole('heading', { name: 'Note only on this device' })).toBeTruthy()
})

it('a save that finishes after leaving the editor leaves the next editor open', async () => {
  let finish: (value: unknown) => void = () => {}
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string) => {
          if (method === 'carddav.accounts') return bookAccounts
          if (method === 'carddav.cardsFor') return { cards: [] }
          if (method === 'carddav.createContact') return new Promise((resolve) => (finish = resolve))
          return { contacts: [], threads: [] }
        },
      },
    },
  }
  const view = render(<ContactsView />)
  fireEvent.click(await view.findByRole('button', { name: 'New contact' }))
  fireEvent.change(view.getByLabelText('Name'), { target: { value: 'Ada Lane' } })
  fireEvent.change(view.getByLabelText('Email addresses'), { target: { value: 'ada@example.test' } })
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Save' })))
  fireEvent.click(view.getByRole('button', { name: 'Back' }))
  fireEvent.click(view.getByRole('button', { name: 'New contact' }))
  fireEvent.change(view.getByLabelText('Name'), { target: { value: 'Ben Hall' } })
  await act(async () => finish({}))
  expect((view.getByLabelText('Name') as HTMLInputElement).value).toBe('Ben Hall')
  expect(contacts$.selected.peek()).toBeNull()
})

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
