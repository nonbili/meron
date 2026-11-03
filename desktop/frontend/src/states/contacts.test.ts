import { afterEach, expect, it } from 'bun:test'
import { contacts$, openContact, restoreContacts, toggleContactPin, toggleContactsPanel } from './contacts'
import { ui$ } from './ui'
import { openThreadTab, openThreadTabById, reopenComposeTab } from './compose'
import { compose$ } from './composeState'
import type { ComposeDraft, Message, MessageTab } from '../types'

afterEach(() => {
  contacts$.selected.set(null)
  contacts$.pinned.set([])
  ui$.contactsOpen.set(false)
  ui$.tasksPanelOpen.set(false)
  ui$.calendarPanelOpen.set(false)
})

it('keeps narrow Contacts open for background changes but closes for explicit thread navigation', async () => {
  const original = globalThis.matchMedia
  globalThis.matchMedia = (() => ({ matches: true })) as unknown as typeof matchMedia
  const thread = ui$.selectedThread.peek()
  const tabs = compose$.tabs.peek()
  const active = compose$.activeTab.peek()
  try {
    openContact({ name: 'Alice', addr: 'alice@example.com' })
    ui$.selectedThread.set('background-thread')
    compose$.activeTab.set('')
    expect(ui$.contactsOpen.peek()).toBe(true)
    openThreadTab({ thread_id: 'explicit-thread', account_id: 'acct', folder_id: 'INBOX' } as Message)
    expect(ui$.contactsOpen.peek()).toBe(false)
    openContact({ name: 'Alice', addr: 'alice@example.com' })
    await openThreadTabById('explicit-thread')
    expect(ui$.contactsOpen.peek()).toBe(false)
    openContact({ name: 'Alice', addr: 'alice@example.com' })
    reopenComposeTab({ id: 'saved-draft', kind: 'compose' } as MessageTab, { draftMessageId: 'draft' } as ComposeDraft)
    expect(ui$.contactsOpen.peek()).toBe(false)
  } finally {
    globalThis.matchMedia = original
    compose$.tabs.set(tabs)
    compose$.activeTab.set(active)
    ui$.selectedThread.set(thread)
  }
})

it('pins by normalized address and restores contacts without cached mail', () => {
  restoreContacts({
    pinned_contacts: [
      { name: 'Alice', addr: ' Alice@Example.com ' },
      { addr: 'alice@example.com', name: 'Alice' },
      null,
      { addr: '' },
    ],
  })
  expect(contacts$.pinned.peek()).toEqual([{ name: 'Alice', addr: 'alice@example.com' }])
  toggleContactPin({ name: 'Other spelling', addr: 'ALICE@example.com' })
  expect(contacts$.pinned.peek()).toEqual([])
  toggleContactPin({ name: 'Bob', addr: ' Bob@example.com ' })
  expect(contacts$.pinned.peek()).toEqual([{ name: 'Bob', addr: 'bob@example.com' }])
})

it('opens a contact without changing the mailbox and shares the panel slot', () => {
  const account = ui$.selectedAccount.peek()
  const folder = ui$.selectedFolder.peek()
  ui$.tasksPanelOpen.set(true)
  openContact({ name: 'Alice', addr: 'Alice@Example.com' })
  expect(ui$.tasksPanelOpen.peek()).toBe(false)
  expect(ui$.contactsOpen.peek()).toBe(true)
  expect(contacts$.selected.peek()?.addr).toBe('alice@example.com')
  expect(ui$.selectedAccount.peek()).toBe(account)
  expect(ui$.selectedFolder.peek()).toBe(folder)
  ui$.calendarPanelOpen.set(true)
  expect(ui$.contactsOpen.peek()).toBe(false)
  openContact()
  expect(ui$.calendarPanelOpen.peek()).toBe(false)
  ui$.tasksPanelOpen.set(true)
  expect(ui$.contactsOpen.peek()).toBe(false)
  toggleContactsPanel()
  expect(ui$.contactsOpen.peek()).toBe(true)
  toggleContactsPanel()
  expect(ui$.contactsOpen.peek()).toBe(false)
})
