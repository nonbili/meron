import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { kanban$, markColumnAllRead } from './kanban'
import { mail$ } from './mail'

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'acc1:INBOX:t1#1',
  account_id: 'acc1',
  folder_id: 'INBOX',
  thread_id: 'acc1#INBOX#t1',
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

describe('markColumnAllRead', () => {
  const calls: { command: string; payload: any }[] = []

  beforeEach(() => {
    calls.length = 0
    accounts$.set([
      {
        id: 'acc1',
        email: 'one@example.com',
        display_name: 'One',
        provider: 'custom',
        auth_type: 'password',
        imap_host: '',
        imap_port: 993,
        smtp_host: '',
        smtp_port: 465,
        tls: true,
        signature: '',
      },
      {
        id: 'acc2',
        email: 'two@example.com',
        display_name: 'Two',
        provider: 'custom',
        auth_type: 'password',
        imap_host: '',
        imap_port: 993,
        smtp_host: '',
        smtp_port: 465,
        tls: true,
        signature: '',
      },
    ])
    kanban$.threads.set({})
    mail$.foldersByAccount.set({})
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
            return { ok: true }
          },
        },
      },
    }
  })

  it('refreshes affected account folder caches after marking a kanban column read', async () => {
    kanban$.threads['unified\ninbox'].set([
      message({ account_id: 'acc1', thread_id: 'acc1#INBOX#t1' }),
      message({ id: 'acc2:INBOX:t2#1', account_id: 'acc2', thread_id: 'acc2#INBOX#t2' }),
    ])

    await markColumnAllRead({ accountId: 'unified', folderId: 'inbox' })

    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'acc1', folder_id: 'inbox' },
      { account_id: 'acc2', folder_id: 'inbox' },
    ])
    expect(calls.filter((call) => call.command === 'mail.folderList').map((call) => call.payload)).toEqual([
      { account_id: 'acc1', refresh: false },
      { account_id: 'acc2', refresh: false },
    ])
    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(0)
    expect(mail$.foldersByAccount.acc2.get()?.[0]?.unread).toBe(0)
  })

  it('marks a mail column read even when no unread thread is loaded', async () => {
    kanban$.threads['acc1\ninbox'].set([])

    await markColumnAllRead({ accountId: 'acc1', folderId: 'inbox' })

    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'acc1', folder_id: 'inbox' },
    ])
    expect(calls.filter((call) => call.command === 'mail.folderList').map((call) => call.payload)).toEqual([
      { account_id: 'acc1', refresh: false },
    ])
  })
})
