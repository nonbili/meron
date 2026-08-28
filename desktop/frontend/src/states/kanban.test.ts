import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import {
  kanban$,
  focusKanbanThreadFolder,
  markColumnAllRead,
  openCorrespondentMail,
  removeKanbanColumnsForFolder,
  switchKanbanColumnFolder,
} from './kanban'
import { mail$ } from './mail'
import { settings$ } from './settings'
import { ui$ } from './ui'
import { thread$ } from './thread'

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

describe('openCorrespondentMail', () => {
  it('opens mail search for the address in the conversation account and folder', () => {
    kanban$.activeBoardId.set('')
    ui$.selectedAccount.set('other')
    ui$.selectedFolder.set('Archive')
    ui$.filterMode.set('unread')
    ui$.query.set('old query')
    ui$.selectedThread.set('old-thread')
    ui$.mobilePane.set('conversation')
    thread$.mediaOpen.set(true)

    openCorrespondentMail('acc1', 'INBOX', '  sender@example.com  ')

    expect(ui$.selectedAccount.get()).toBe('acc1')
    expect(ui$.selectedFolder.get()).toBe('INBOX')
    expect(ui$.filterMode.get()).toBe('all')
    expect(ui$.query.get()).toBe('sender@example.com')
    expect(ui$.selectedThread.get()).toBe('')
    expect(ui$.mobilePane.get()).toBe('threads')
    expect(thread$.mediaOpen.get()).toBe(false)
  })

  it('uses the card folder instead of resuming a same-account mail folder', () => {
    const prefsWrites: Array<{ key: string; value: string }> = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: { key: string; value: string }) => {
            if (command === 'app.prefsSet') prefsWrites.push(payload)
            return { ok: true }
          },
        },
      },
    }
    ui$.selectedAccount.set('acc1')
    ui$.selectedFolder.set('Archive')
    kanban$.activeBoardId.set('board')
    focusKanbanThreadFolder('INBOX')
    prefsWrites.length = 0

    openCorrespondentMail('acc1', 'INBOX', 'sender@example.com')

    expect(kanban$.activeBoardId.get()).toBe('')
    expect(ui$.selectedFolder.get()).toBe('INBOX')
    expect(prefsWrites.filter((write) => write.key === 'session_folder')).toEqual([
      { key: 'session_folder', value: 'INBOX' },
    ])
  })

  it('ignores an empty address', () => {
    ui$.query.set('keep me')

    openCorrespondentMail('acc1', 'INBOX', '   ')

    expect(ui$.query.get()).toBe('keep me')
  })
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
      },
    ])
    kanban$.threads.set({})
    kanban$.unreadCounts.set({})
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
    kanban$.unreadCounts['unified\ninbox'].set(2)

    await markColumnAllRead({ accountId: 'unified', folderId: 'inbox' })

    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'unified', folder_id: 'inbox' },
    ])
    expect(calls.filter((call) => call.command === 'mail.folderList').map((call) => call.payload)).toEqual([
      { account_id: 'acc1', refresh: false },
      { account_id: 'acc2', refresh: false },
    ])
    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(0)
    expect(mail$.foldersByAccount.acc2.get()?.[0]?.unread).toBe(0)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(0)
  })

  it('sends a unified non-inbox role through the role-resolving backend path', async () => {
    kanban$.threads['unified\nsent'].set([message({ folder_id: '[Gmail]/Sent Mail' })])

    await markColumnAllRead({ accountId: 'unified', folderId: 'sent' })

    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'unified', folder_id: 'sent' },
    ])
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

describe('switchKanbanColumnFolder', () => {
  beforeEach(() => {
    settings$.kanbanBoards.set([
      {
        id: 'b1',
        name: 'Board',
        columns: [
          { accountId: 'acc1', folderId: 'INBOX' },
          { accountId: 'acc1', folderId: 'Archive' },
        ],
      },
    ])
    settings$.kanbanMinimizedColumns.set({})
    kanban$.threads.set({})
    kanban$.cursors.set({})
    kanban$.accountCursors.set({})
    kanban$.filters.set({})
    kanban$.paneColumnKey.set('')
    kanban$.searchScope.set('all')
  })

  it('repoints the column in place and carries its state to the new key', () => {
    kanban$.threads['acc1\nINBOX'].set([message()])
    kanban$.cursors['acc1\nINBOX'].set('cursor')
    kanban$.filters['acc1\nINBOX'].set('unread')
    settings$.kanbanMinimizedColumns['b1\nacc1\nINBOX'].set(true)
    kanban$.paneColumnKey.set('b1\nacc1\nINBOX')
    kanban$.searchScope.set('acc1\nINBOX')

    const switched = switchKanbanColumnFolder('b1', { accountId: 'acc1', folderId: 'INBOX' }, 'Sent')

    expect(switched).toBe(true)
    expect(settings$.kanbanBoards.get()[0].columns).toEqual([
      { accountId: 'acc1', folderId: 'Sent' },
      { accountId: 'acc1', folderId: 'Archive' },
    ])
    expect(kanban$.filters['acc1\nSent'].get()).toBe('unread')
    expect(settings$.kanbanMinimizedColumns['b1\nacc1\nSent'].get()).toBe(true)
    expect(settings$.kanbanMinimizedColumns['b1\nacc1\nINBOX'].get()).toBeUndefined()
    expect(kanban$.paneColumnKey.get()).toBe('b1\nacc1\nSent')
    expect(kanban$.searchScope.get()).toBe('acc1\nSent')
    expect(kanban$.threads['acc1\nINBOX'].get()).toBeUndefined()
    expect(kanban$.cursors['acc1\nINBOX'].get()).toBeUndefined()
  })

  it('refuses a folder that already has its own column', () => {
    const switched = switchKanbanColumnFolder('b1', { accountId: 'acc1', folderId: 'INBOX' }, 'Archive')

    expect(switched).toBe(false)
    expect(settings$.kanbanBoards.get()[0].columns).toEqual([
      { accountId: 'acc1', folderId: 'INBOX' },
      { accountId: 'acc1', folderId: 'Archive' },
    ])
  })

  it('keeps the old folder cache when another board still shows it', () => {
    settings$.kanbanBoards.set([
      { id: 'b1', name: 'Board', columns: [{ accountId: 'acc1', folderId: 'INBOX' }] },
      { id: 'b2', name: 'Other', columns: [{ accountId: 'acc1', folderId: 'INBOX' }] },
    ])
    kanban$.threads['acc1\nINBOX'].set([message()])

    expect(switchKanbanColumnFolder('b1', { accountId: 'acc1', folderId: 'INBOX' }, 'Sent')).toBe(true)
    expect(kanban$.threads['acc1\nINBOX'].get()).toHaveLength(1)
  })
})

describe('removeKanbanColumnsForFolder', () => {
  it("drops the folder's columns on every board and forgets its cache", () => {
    settings$.kanbanBoards.set([
      {
        id: 'b1',
        name: 'Board',
        columns: [
          { accountId: 'acc1', folderId: 'INBOX' },
          { accountId: 'acc1', folderId: 'Work' },
        ],
      },
      { id: 'b2', name: 'Other', columns: [{ accountId: 'acc1', folderId: 'Work' }] },
    ])
    kanban$.threads.set({})
    kanban$.threads['acc1\nWork'].set([message({ folder_id: 'Work' })])

    removeKanbanColumnsForFolder('acc1', 'Work')

    expect(settings$.kanbanBoards.get()[0].columns).toEqual([{ accountId: 'acc1', folderId: 'INBOX' }])
    expect(settings$.kanbanBoards.get()[1].columns).toEqual([])
    expect(kanban$.threads['acc1\nWork'].get()).toBeUndefined()
  })

  it('leaves a same-named folder of another account alone', () => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [{ accountId: 'acc2', folderId: 'Work' }] }])

    removeKanbanColumnsForFolder('acc1', 'Work')

    expect(settings$.kanbanBoards.get()[0].columns).toEqual([{ accountId: 'acc2', folderId: 'Work' }])
  })
})
