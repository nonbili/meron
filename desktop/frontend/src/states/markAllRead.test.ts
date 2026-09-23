import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { mail$ } from './mail'
import { markAllRead } from './mailFlags'
import { ui$ } from './ui'

const account = (id: string) => ({
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

describe('markAllRead', () => {
  let previousGo: unknown
  let release = () => {}
  let calls: Array<{ command: string; payload: any }> = []
  let markAllReadResult: unknown = {}
  // The unread counts the server's folder list answers with after the write.
  let serverUnread: Record<string, number> = {}

  beforeEach(() => {
    previousGo = (window as any).go
    accounts$.set([account('acc-1'), account('acc-2')] as any)
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('inbox')
    mail$.folders.set([{ id: 'inbox', account_id: 'unified', name: 'Inbox', role: 'inbox', unread: 57 }])
    mail$.foldersByAccount.set({
      'acc-1': [{ id: 'INBOX', account_id: 'acc-1', name: 'Inbox', role: 'inbox', unread: 50 }],
      'acc-2': [{ id: 'INBOX', account_id: 'acc-2', name: 'Inbox', role: 'inbox', unread: 7 }],
    })
    mail$.threads.set([
      message(),
      message({ id: 'acc-2:INBOX:t2#1', account_id: 'acc-2', thread_id: 'acc-2#INBOX#t2' }),
    ])
    mail$.messages.set(mail$.threads.get())
    calls = []
    markAllReadResult = {}
    serverUnread = {}
    const gate = new Promise<void>((resolve) => (release = resolve))
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            calls.push({ command, payload })
            if (command === 'mail.folderList') {
              return {
                folders: [
                  {
                    id: 'INBOX',
                    account_id: payload.account_id,
                    name: 'Inbox',
                    role: 'inbox',
                    unread: serverUnread[payload.account_id] ?? 0,
                  },
                ],
              }
            }
            if (command === 'mail.markAllRead') {
              await gate
              return markAllReadResult
            }
            return {}
          },
        },
      },
    }
  })

  afterEach(() => {
    release()
    if (previousGo === undefined) delete (window as any).go
    else (window as any).go = previousGo
  })

  it('clears the side navigation folder badges before the backend answers', async () => {
    const pending = markAllRead()

    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(0)
    expect(mail$.foldersByAccount['acc-2'][0].unread.get()).toBe(0)
    expect(mail$.folders[0].unread.get()).toBe(0)
    release()
    await pending
  })

  it('puts rows and badges back when a unified write fails for an account', async () => {
    markAllReadResult = { ok: false, failures: [{ account_id: 'acc-2', message: 'Offline' }] }
    serverUnread = { 'acc-2': 7 }
    release()
    await markAllRead()

    expect(mail$.foldersByAccount['acc-2'][0].unread.get()).toBe(7)
    expect(mail$.threads.get().every((thread) => thread.unread)).toBe(true)
  })

  it('leaves a view and conversation opened while the write was pending alone on failure', async () => {
    markAllReadResult = { ok: false, failures: [{ account_id: 'acc-2', message: 'Offline' }] }
    serverUnread = { 'acc-2': 7 }
    const pending = markAllRead()

    // The user moves to acc-1's Archive and opens a conversation there.
    const other = message({
      id: 'acc-1:Archive:t9#1',
      folder_id: 'Archive',
      thread_id: 'acc-1#Archive#t9',
      unread: false,
      subject: 'Other',
    })
    const acc1Folders = [
      { id: 'INBOX', account_id: 'acc-1', name: 'Inbox', role: 'inbox', unread: 50 },
      { id: 'Archive', account_id: 'acc-1', name: 'Archive', role: 'archive', unread: 0 },
    ]
    ui$.selectedAccount.set('acc-1')
    ui$.selectedFolder.set('Archive')
    mail$.folders.set(acc1Folders)
    mail$.threads.set([other])
    mail$.messages.set([other])
    release()
    await pending

    expect(mail$.threads.get()).toEqual([other])
    expect(mail$.messages.get()).toEqual([other])
    // The unified write's folder refresh must not replace acc-1's folder list
    // with the unified one it started from.
    expect(mail$.folders.get().map((folder) => folder.account_id)).toEqual(['acc-1', 'acc-1'])
    expect(mail$.folders.get().some((folder) => folder.id === 'Archive')).toBe(true)
    expect(mail$.foldersByAccount['acc-2'][0].unread.get()).toBe(7)
    expect(calls.some((call) => call.command === 'mail.threadList')).toBe(false)
  })

  it('marks unified Starred item by item rather than as a folder', async () => {
    ui$.selectedFolder.set('starred')
    release()
    await markAllRead()

    expect(calls.filter((call) => call.command === 'mail.markAllRead')).toEqual([])
    expect(
      calls
        .filter((call) => call.command === 'mail.markRead')
        .map((call) => call.payload.thread_id)
        .sort(),
    ).toEqual(['acc-1#INBOX#thread-1', 'acc-2#INBOX#t2'])
  })
})
