import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { kanban$ } from './kanban'
import { loadThread, mail$ } from './mail'
import { markAllRead, markMessagesRead } from './mailFlags'
import { ui$ } from './ui'

const thread = (overrides: Partial<Message> = {}): Message => ({
  id: 'acc:inbox:thread:1#101',
  account_id: 'acc',
  folder_id: 'inbox',
  thread_id: 'acc:inbox:thread:1',
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Subject',
  preview: '',
  body: '',
  date: Math.floor(Date.parse('2026-06-11T12:00:00Z') / 1000),
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

const nextTick = () => new Promise((resolve) => setTimeout(resolve, 0))

describe('markAllRead', () => {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    calls.length = 0
    accounts$.set([
      {
        id: 'acc',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: '',
        imap_port: 993,
        smtp_host: '',
        smtp_port: 465,
        tls: true,
      },
    ])
    mail$.threads.set([])
    mail$.messages.set([])
    mail$.folders.set([{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 12 }])
    mail$.foldersByAccount.set({})
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.folderList') {
              return { folders: [{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 }] }
            }
            return { ok: true }
          },
        },
      },
    }
  })

  it('marks the selected mail folder read even when no unread thread is loaded', async () => {
    await markAllRead()

    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'acc', folder_id: 'inbox' },
    ])
    expect(calls.filter((call) => call.command === 'mail.folderList').map((call) => call.payload)).toEqual([
      { account_id: 'acc', refresh: false },
    ])
  })
})

describe('thread read state', () => {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    calls.length = 0
    mail$.threads.set([thread({ unread: true, unread_count: 2 })])
    mail$.messages.set([])
    mail$.readThreads.set({})
    mail$.folders.set([{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 2 }])
    mail$.foldersByAccount.set({})
    kanban$.threads.set({})
    kanban$.unreadCounts.set({})
    mail$.threadLoading.set(false)
    mail$.messagesCursor.set('')
    ui$.selectedThread.set('acc:inbox:thread:1')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.threadRead') {
              return {
                messages: [
                  thread({ id: 'acc:inbox:thread:1#101', unread: true, unread_count: 2 }),
                  thread({ id: 'acc:inbox:thread:1#102', unread: true, unread_count: 2 }),
                ],
                next_cursor: '',
              }
            }
            if (command === 'mail.folderList') {
              return { folders: [{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 1 }] }
            }
            return { ok: true }
          },
        },
      },
    }
  })

  it('does not mark a full thread read just because it was opened', async () => {
    await loadThread('acc:inbox:thread:1')
    await nextTick()

    expect(calls.find((call) => call.command === 'mail.threadRead')?.payload).toEqual({
      thread_id: 'acc:inbox:thread:1',
      limit: 10,
    })
    expect(mail$.threads.get()[0].unread).toBe(true)
    expect(mail$.threads.get()[0].unread_count).toBe(2)
    expect(mail$.messages.get().map((message) => message.unread)).toEqual([true, true])
    expect(mail$.folders.get()[0].unread).toBe(2)
    expect(calls.some((call) => call.command === 'mail.markRead')).toBe(false)
  })

  it('clears a stale thread badge when all loaded messages are already read', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') {
        return {
          messages: [
            thread({ id: 'acc:inbox:thread:1#101', unread: false, unread_count: 2 }),
            thread({ id: 'acc:inbox:thread:1#102', unread: false, unread_count: 2 }),
          ],
          next_cursor: '',
        }
      }
      return { ok: true }
    }

    await loadThread('acc:inbox:thread:1')
    await nextTick()

    expect(mail$.threads.get()[0].unread).toBe(false)
    expect(mail$.threads.get()[0].unread_count).toBe(0)
    expect(mail$.folders.get()[0].unread).toBe(0)
    expect(calls.some((call) => call.command === 'mail.markRead')).toBe(false)
  })

  it('keeps the thread badge when unread messages may exist on an older page', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') {
        return {
          messages: [thread({ id: 'acc:inbox:thread:1#101', unread: false, unread_count: 2 })],
          next_cursor: 'date:1:1',
        }
      }
      return { ok: true }
    }

    await loadThread('acc:inbox:thread:1')
    await nextTick()

    expect(mail$.threads.get()[0].unread).toBe(true)
    expect(mail$.threads.get()[0].unread_count).toBe(2)
    expect(mail$.folders.get()[0].unread).toBe(2)
    expect(calls.some((call) => call.command === 'mail.markRead')).toBe(false)
  })

  it('marks only the visible message ids read', async () => {
    await loadThread('acc:inbox:thread:1')
    await markMessagesRead('acc:inbox:thread:1', ['acc:inbox:thread:1#101'])

    expect(mail$.threads.get()[0].unread).toBe(true)
    expect(mail$.threads.get()[0].unread_count).toBe(1)
    expect(mail$.messages.get().map((message) => message.unread)).toEqual([false, true])
    expect(mail$.folders.get()[0].unread).toBe(1)
    expect(calls.filter((call) => call.command === 'mail.markRead').map((call) => call.payload)).toEqual([
      { thread_id: 'acc:inbox:thread:1', folder: 'inbox', message_ids: ['acc:inbox:thread:1#101'] },
    ])
  })

  it('marks a cross-folder thread read per folder without touching the other mailbox', async () => {
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 2 },
      { id: 'sent', account_id: 'acc', name: 'Sent', role: 'sent', unread: 1 },
    ])
    mail$.threads.set([thread({ unread: true, unread_count: 2 })])
    mail$.messages.set([
      thread({ id: 'acc:inbox:thread:1#101', unread: true }),
      thread({ id: 'acc:inbox:thread:1#102', unread: true }),
      thread({ id: 'acc:sent:thread:1#7', folder_id: 'sent', unread: true }),
    ])

    await markMessagesRead('acc:inbox:thread:1', ['acc:inbox:thread:1#101', 'acc:sent:thread:1#7'])

    // Each folder gets its own call, since IMAP UIDs are mailbox-local.
    expect(calls.filter((call) => call.command === 'mail.markRead').map((call) => call.payload)).toEqual([
      { thread_id: 'acc:inbox:thread:1', folder: 'inbox', message_ids: ['acc:inbox:thread:1#101'] },
      { thread_id: 'acc:inbox:thread:1', folder: 'sent', message_ids: ['acc:sent:thread:1#7'] },
    ])
    // Only the Inbox message came off the Inbox card and badge; the Sent read
    // lands on the Sent badge.
    expect(mail$.threads.get()[0].unread_count).toBe(1)
    expect(mail$.threads.get()[0].unread).toBe(true)
    expect(mail$.folders.get()[0].unread).toBe(1)
    expect(mail$.folders.get()[1].unread).toBe(0)
  })

  it('updates kanban card and column unread state immediately', async () => {
    const columnKey = 'acc\u0000inbox'
    kanban$.threads.set({ [columnKey]: [thread({ unread: true, unread_count: 2 })] })
    kanban$.unreadCounts.set({ [columnKey]: 2 })

    await loadThread('acc:inbox:thread:1')
    await markMessagesRead('acc:inbox:thread:1', ['acc:inbox:thread:1#101'])

    expect(kanban$.threads.get()[columnKey][0].unread).toBe(true)
    expect(kanban$.threads.get()[columnKey][0].unread_count).toBe(1)
    expect(kanban$.unreadCounts.get()[columnKey]).toBe(1)

    await markMessagesRead('acc:inbox:thread:1', ['acc:inbox:thread:1#102'])

    expect(kanban$.threads.get()[columnKey][0].unread).toBe(false)
    expect(kanban$.threads.get()[columnKey][0].unread_count).toBe(0)
    expect(kanban$.unreadCounts.get()[columnKey]).toBe(0)
    expect(mail$.readThreads.get()['acc:inbox:thread:1']).toBe(true)
  })

  // A failed mutation must undo its own optimistic edit and nothing else: a folder
  // LIST for another account, or another column's page, can land while the call is
  // in flight, and restoring whole caches would silently throw those away.
  it('rolls back only the keys it touched when the backend rejects', async () => {
    const columnKey = 'acc inbox'
    const otherColumnKey = 'acc archive'
    const otherCard = thread({ id: 'acc:archive:thread:9#1', thread_id: 'acc:archive:thread:9' })
    kanban$.threads.set({ [columnKey]: [thread({ unread: true, unread_count: 2 })], [otherColumnKey]: [] })
    kanban$.unreadCounts.set({ [columnKey]: 2, [otherColumnKey]: 0 })
    mail$.foldersByAccount.set({
      acc: [{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 2 }],
      other: [{ id: 'inbox', account_id: 'other', name: 'Inbox', role: 'inbox', unread: 5 }],
    })

    await loadThread('acc:inbox:thread:1')
    const invoke = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command !== 'mail.markRead') return invoke(command, payload)
      // Writes that land while the mutation is in flight.
      mail$.foldersByAccount.other.set([{ id: 'inbox', account_id: 'other', name: 'Inbox', role: 'inbox', unread: 9 }])
      kanban$.threads[otherColumnKey].set([otherCard])
      kanban$.unreadCounts[otherColumnKey].set(4)
      throw new Error('offline')
    }

    await expect(markMessagesRead('acc:inbox:thread:1', ['acc:inbox:thread:1#101'])).rejects.toThrow('offline')

    // Untouched keys keep what landed mid-flight.
    expect(mail$.foldersByAccount.get().other[0].unread).toBe(9)
    expect(kanban$.threads.get()[otherColumnKey]).toEqual([otherCard])
    expect(kanban$.unreadCounts.get()[otherColumnKey]).toBe(4)
    // The column the mutation edited is back to its pre-call state.
    expect(kanban$.threads.get()[columnKey][0].unread_count).toBe(2)
    expect(kanban$.unreadCounts.get()[columnKey]).toBe(2)
    expect(mail$.messages.get().every((message) => message.unread)).toBe(true)
  })
})
