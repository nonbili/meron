import { afterEach, beforeEach, expect, it } from 'bun:test'
import { openThreadWindow } from './threadWindow'
import {
  restoreThreadWindowDrafts,
  openComposeTab,
  updateComposeDraft,
  openDraftConversationOrCompose,
  openDraftFromThreadWindow,
} from '../states/compose'
import { compose$, COMPOSE_TABS_KEY } from '../states/composeState'
import { accounts$ } from '../states/accounts'
import { mail$ } from '../states/mail'
import { ui$, dismissToast } from '../states/ui'
import { ensureThreadData } from '../states/threadData'
import { threadView, forgetThreadViews } from '../states/threadView'
import type { Account, Message } from '../types'

let calls: { command: string; payload: any }[]
let liveSessions: string[]
let focusedDraft: boolean
let threadMessages: Message[]
beforeEach(() => {
  calls = []
  liveSessions = []
  focusedDraft = false
  threadMessages = []
  localStorage.clear()
  compose$.tabs.set([])
  compose$.activeTab.set('')
  ui$.selectedThread.set('')
  compose$.composer.set('')
  compose$.quickReplyDraftId.set('')
  compose$.quickReplyDraftThreadId.set('')
  mail$.threads.set([])
  mail$.messages.set([])
  forgetThreadViews()
  accounts$.set([{ id: 'a', email: 'me@example.com', provider: 'custom', conversation_html: false } as Account])
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'threadWindow.sessions') return liveSessions
          if (command === 'threadWindow.focusDraft') return focusedDraft
          if (command === 'mail.threadRead') return { messages: threadMessages }
          return {}
        },
      },
    },
  }
})
afterEach(() => {
  dismissToast()
  localStorage.clear()
})
it('opens the active tab with its own scroll and expansion snapshot', async () => {
  const thread = { thread_id: 't', id: 'm', subject: 'Subject', account_id: 'a', folder_id: 'INBOX' } as Message
  mail$.threads.set([thread])
  ensureThreadData('t').messages.set([thread])
  compose$.tabs.set([{ id: 'thread-t', kind: 'thread', threadId: 't', subject: 'Subject' } as any])
  compose$.activeTab.set('thread-t')
  threadView('thread-t:t').scroll = { messageId: 'm', offset: 23, atBottom: false }
  threadView('thread-t:t').overrides = { m: true }
  await openThreadWindow('t')
  const call = calls.find((call) => call.command === 'threadWindow.open')!
  expect(call.payload.thread).toEqual(thread)
  expect(call.payload.view.scroll).toEqual({ messageId: 'm', offset: 23, atBottom: false })
  expect(call.payload.view.overrides).toEqual({ m: true })
  expect(ui$.selectedThread.peek()).toBe('')
})
it('recovers abandoned auxiliary drafts into main storage once and retires their buffers', async () => {
  const id = openComposeTab({ to: 'friend@example.com', subject: 'Recovered', text: 'Keep me' })!
  updateComposeDraft(id, { draftMessageId: 'recovery@example.com' })
  const buffer = localStorage.getItem(COMPOSE_TABS_KEY)!
  compose$.tabs.set([])
  localStorage.setItem('meron-thread-abandoned-tabs', buffer)
  localStorage.setItem('meron-thread-abandoned-view', '{}')
  await restoreThreadWindowDrafts()
  expect(compose$.tabs.peek()).toHaveLength(1)
  expect(compose$.tabs.peek()[0]?.compose?.text).toBe('Keep me')
  expect(localStorage.getItem('meron-thread-abandoned-tabs')).toBeNull()
  expect(localStorage.getItem('meron-thread-abandoned-view')).toBeNull()
  localStorage.setItem('meron-thread-abandoned-tabs', buffer)
  await restoreThreadWindowDrafts()
  expect(compose$.tabs.peek()).toHaveLength(1)
})

it('does not recover or retire the buffer of an open thread window on main reload', async () => {
  const id = openComposeTab({ to: 'friend@example.com', text: 'Still editing' })!
  updateComposeDraft(id, { draftMessageId: 'live@example.com' })
  const buffer = localStorage.getItem(COMPOSE_TABS_KEY)!
  compose$.tabs.set([])
  localStorage.setItem('meron-thread-live-tabs', buffer)
  localStorage.setItem('meron-thread-live-view', '{}')
  liveSessions = ['live']
  await restoreThreadWindowDrafts()
  expect(compose$.tabs.peek()).toHaveLength(0)
  expect(localStorage.getItem('meron-thread-live-tabs')).toBe(buffer)
  expect(localStorage.getItem('meron-thread-live-view')).toBe('{}')
  liveSessions = []
  await restoreThreadWindowDrafts()
  expect(compose$.tabs.peek()[0]?.compose?.text).toBe('Still editing')
})

it('focuses a window-owned reply draft before opening its conversation from Drafts', async () => {
  const original = {
    thread_id: 't',
    id: 'm',
    subject: 'Subject',
    account_id: 'a',
    folder_id: 'INBOX',
    date: 1,
  } as Message
  const draft = {
    ...original,
    id: 'd',
    folder_id: 'Drafts',
    date: 2,
    message_id: 'draft@example.com',
    references: '<original@example.com>',
  }
  threadMessages = [original, draft]
  focusedDraft = true
  await openDraftConversationOrCompose(draft)
  expect(
    calls.some((call) => call.command === 'threadWindow.focusDraft' && call.payload.draft_id === 'draft@example.com'),
  ).toBe(true)
  expect(ui$.selectedThread.peek()).toBe('')
  expect(compose$.tabs.peek()).toHaveLength(0)
})

it('focuses the main quick reply without duplicating its saved draft or losing unsaved text', async () => {
  const original = {
    thread_id: 't',
    id: 'm',
    subject: 'Subject',
    account_id: 'a',
    folder_id: 'INBOX',
    date: 1,
  } as Message
  const draft = { ...original, id: 'd', folder_id: 'Drafts', message_id: '<DRAFT@example.com>' }
  ui$.selectedThread.set('t')
  compose$.quickReplyDraftId.set('draft@example.com')
  compose$.quickReplyDraftThreadId.set('t')
  compose$.quickReplyDraftSaved.set(true)
  compose$.composer.set('The newest text has not been saved yet')
  compose$.conversationThread.set('other')
  ui$.calendarOpen.set(true)
  ui$.contactsOpen.set(true)
  await openDraftFromThreadWindow(draft)
  expect(compose$.tabs.peek()).toHaveLength(0)
  expect(ui$.selectedThread.peek()).toBe('t')
  expect(compose$.conversationThread.peek()).toBe('t')
  expect(compose$.composer.peek()).toBe('The newest text has not been saved yet')
  expect(compose$.quickReplyDraftId.peek()).toBe('draft@example.com')
  expect(ui$.calendarOpen.peek()).toBe(false)
  expect(ui$.contactsOpen.peek()).toBe(false)
  expect(calls.some((call) => call.command === 'mail.threadRead')).toBe(false)
})

it('removes orphaned view-only buffers while preserving those of live windows', async () => {
  liveSessions = ['live']
  localStorage.setItem('meron-thread-orphan-view', '{}')
  localStorage.setItem('meron-thread-live-view', '{}')
  await restoreThreadWindowDrafts()
  expect(localStorage.getItem('meron-thread-orphan-view')).toBeNull()
  expect(localStorage.getItem('meron-thread-live-view')).toBe('{}')
})

it('opens an unowned rich reply draft in its editor without disturbing another quick reply', async () => {
  const original = {
    thread_id: 't',
    id: 'm',
    subject: 'Subject',
    account_id: 'a',
    folder_id: 'INBOX',
    date: 1,
    from_addr: 'sender@example.com',
    to: 'me@example.com',
    body: 'Received message',
    preview: '',
    from_name: 'Sender',
    unread: false,
    starred: false,
    has_attachments: false,
  } as Message
  const draft = {
    ...original,
    id: 'd',
    folder_id: 'Drafts',
    date: 2,
    message_id: 'draft@example.com',
    from_addr: 'me@example.com',
    to: 'sender@example.com',
    bcc: 'hidden@example.com',
    body: 'Rich reply',
    body_html: '<p>Rich <strong>reply</strong></p>',
    references: '<original@example.com>',
  }
  threadMessages = [original, draft]
  ui$.selectedThread.set('other-thread')
  compose$.composer.set('New unsaved text in the other conversation')
  compose$.quickReplyDraftId.set('other-draft@example.com')
  compose$.quickReplyDraftThreadId.set('other-thread')
  await openDraftFromThreadWindow(draft)
  expect(compose$.tabs.peek()).toHaveLength(1)
  expect(compose$.tabs.peek()[0]?.compose?.html).toBe(draft.body_html)
  expect(compose$.tabs.peek()[0]?.compose?.draftMessageId).toBe('draft@example.com')
  expect(compose$.tabs.peek()[0]?.compose?.bcc).toBe('hidden@example.com')
  expect(ui$.selectedThread.peek()).toBe('other-thread')
  expect(compose$.composer.peek()).toBe('New unsaved text in the other conversation')
  expect(compose$.quickReplyDraftId.peek()).toBe('other-draft@example.com')
})
