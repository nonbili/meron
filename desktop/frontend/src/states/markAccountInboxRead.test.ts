import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { kanban$ } from './kanban'
import { mail$, markAccountInboxRead } from './mail'
import { ui$ } from './ui'

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'acc-1:INBOX:thread-1#1',
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: 'acc-1#INBOX#thread-1',
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Subject',
  preview: '',
  body: '',
  date: 1,
  unread: true,
  starred: false,
  has_attachments: false,
  ...overrides,
})

describe('markAccountInboxRead', () => {
  const calls: { command: string; payload: any }[] = []
  let previousGo: unknown

  beforeEach(() => {
    previousGo = (window as any).go
    calls.length = 0
    accounts$.set([
      {
        id: 'acc-1',
        email: 'one@example.com',
        display_name: 'One',
        provider: 'custom',
        auth_type: 'password',
        imap_host: '',
        imap_port: 993,
        smtp_host: '',
        smtp_port: 465,
        tls: true,
      },
      {
        id: 'acc-2',
        email: 'two@example.com',
        display_name: 'Two',
        provider: 'custom',
        auth_type: 'password',
        imap_host: '',
        imap_port: 993,
        smtp_host: '',
        smtp_port: 465,
        tls: true,
      },
    ])
    ui$.selectedAccount.set('unified')
    mail$.folders.set([{ id: 'inbox', account_id: 'unified', name: 'Inbox', role: 'inbox', unread: 57 }])
    mail$.foldersByAccount.set({
      'acc-1': [{ id: 'INBOX', account_id: 'acc-1', name: 'Inbox', role: 'inbox', unread: 50 }],
      'acc-2': [{ id: 'INBOX', account_id: 'acc-2', name: 'Inbox', role: 'inbox', unread: 7 }],
    })
    mail$.threads.set([
      message(),
      message({ id: 'acc-1:Archive:thread-2#1', folder_id: 'Archive', thread_id: 'acc-1#Archive#thread-2' }),
      message({ id: 'acc-2:INBOX:thread-3#1', account_id: 'acc-2', thread_id: 'acc-2#INBOX#thread-3' }),
    ])
    mail$.messages.set(mail$.threads.get())
    kanban$.threads.set({
      'acc-1\nINBOX': [],
      'unified\ninbox': [message()],
    })
    kanban$.unreadCounts.set({
      'acc-1\nINBOX': 50,
      'unified\ninbox': 57,
    })
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            calls.push({ command, payload })
            if (command === 'mail.folderList') {
              return {
                folders: [{ id: 'INBOX', account_id: payload.account_id, name: 'Inbox', role: 'inbox', unread: 0 }],
              }
            }
            return { folder_unreads: { 'acc-1': { INBOX: 0 } } }
          },
        },
      },
    }
  })

  // The bridge stub is global: leaving one behind lets a later test file's
  // state writes reach this file's fake backend.
  afterEach(() => {
    if (previousGo === undefined) delete (window as any).go
    else (window as any).go = previousGo
  })

  it('marks only the chosen account Inbox read', async () => {
    await markAccountInboxRead('acc-1')

    expect(calls[0]).toEqual({
      command: 'mail.markAllRead',
      payload: { account_id: 'acc-1', folder_id: 'INBOX' },
    })
    expect(mail$.threads.get().map((thread) => thread.unread)).toEqual([false, true, true])
    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(0)
    expect(mail$.foldersByAccount['acc-2'][0].unread.get()).toBe(7)
    expect(mail$.folders[0].unread.get()).toBe(7)
    expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(false)
    expect(kanban$.unreadCounts['acc-1\nINBOX'].get()).toBe(0)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(7)
  })

  it('rolls optimistic state back when the backend rejects the action', async () => {
    ;(window as any).go.main.App.Invoke = async () => {
      throw new Error('offline')
    }

    await markAccountInboxRead('acc-1')

    expect(mail$.threads[0].unread.get()).toBe(true)
    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(50)
    expect(mail$.folders[0].unread.get()).toBe(57)
    expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(true)
    expect(kanban$.unreadCounts['acc-1\nINBOX'].get()).toBe(50)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(57)
  })
})
