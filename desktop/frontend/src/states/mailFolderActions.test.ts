import { beforeEach, describe, expect, it } from 'bun:test'
import type { Folder, Message } from '../types'
import { kanban$ } from './kanban'
import { mail$ } from './mail'
import { deletableFolder, deleteFolder, ensureAccountFolders } from './mailFolders'
import { settings$ } from './settings'
import { settleConfirm, ui$ } from './ui'

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

describe('deletableFolder', () => {
  const folder = (id: string, overrides: Partial<Folder> = {}): Folder => ({
    id,
    account_id: 'acc',
    name: id,
    role: 'folder',
    unread: 0,
    ...overrides,
  })

  it('accepts an ordinary leaf folder', () => {
    const folders = [folder('inbox', { role: 'inbox' }), folder('Work'), folder('Receipts')]

    expect(deletableFolder(folder('Receipts'), folders)).toEqual({ name: 'Receipts', nested: 0 })
  })

  it('accepts a parent and counts the subfolders that go with it', () => {
    const folders = [
      folder('inbox', { role: 'inbox' }),
      folder('Work', { delimiter: '/' }),
      folder('Work/Reports', { delimiter: '/' }),
      folder('Work/Reports/2026', { delimiter: '/' }),
      // Shares a name prefix but is not nested under Work.
      folder('Workshop', { delimiter: '/' }),
    ]

    expect(deletableFolder(folder('Work', { delimiter: '/' }), folders)).toEqual({ name: 'Work', nested: 2 })
    expect(deletableFolder(folder('Work/Reports', { delimiter: '/' }), folders)).toEqual({
      name: 'Work/Reports',
      nested: 1,
    })
  })

  it('refuses special-use folders, their parents and the unified view', () => {
    const folders = [
      folder('inbox', { role: 'inbox' }),
      folder('Mail', { delimiter: '/' }),
      folder('Mail/Archive', { delimiter: '/', role: 'archive' }),
    ]

    expect(deletableFolder(folder('inbox', { role: 'inbox' }), folders)).toBeNull()
    // Deleting Mail would take the archive with it.
    expect(deletableFolder(folder('Mail', { delimiter: '/' }), folders)).toBeNull()
    expect(deletableFolder(folder('Mail', { account_id: 'unified' }), folders)).toBeNull()
    expect(deletableFolder(undefined, folders)).toBeNull()
  })
})

describe('deleteFolder', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.folderDelete': {
        ok: true,
        deleted: 2,
        folders: [{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 }],
      },
    }
    mail$.threads.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Work', account_id: 'acc', name: 'Work', role: 'folder', unread: 0 },
    ])
    mail$.foldersByAccount.set({ acc: mail$.folders.get() })
    kanban$.threads.set({})
    settings$.kanbanBoards.set([
      {
        id: 'b1',
        name: 'Board',
        columns: [
          { accountId: 'acc', folderId: 'inbox' },
          { accountId: 'acc', folderId: 'Work' },
        ],
      },
    ])
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('Work')
    ui$.selectedThread.set('')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command] ?? {}
          },
        },
      },
    }
  })

  it('deletes on confirm, then drops the column and leaves the folder', async () => {
    const pending = deleteFolder('acc', 'Work', 'Work')
    settleConfirm(true)

    expect(await pending).toBe(true)
    expect(calls.filter((call) => call.command === 'mail.folderDelete')).toEqual([
      { command: 'mail.folderDelete', payload: { account_id: 'acc', folder_id: 'Work' } },
    ])
    expect(mail$.foldersByAccount.get().acc.map((item) => item.id)).toEqual(['inbox'])
    expect(settings$.kanbanBoards.get()[0].columns).toEqual([{ accountId: 'acc', folderId: 'inbox' }])
    expect(ui$.selectedFolder.get()).toBe('inbox')
    expect(ui$.toast.get()).toBe('Work deleted')
  })

  it('clears the columns and the open view of every folder the core removed', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'b1',
        name: 'Board',
        columns: [
          { accountId: 'acc', folderId: 'inbox' },
          { accountId: 'acc', folderId: 'Work' },
          { accountId: 'acc', folderId: 'Work/Reports' },
        ],
      },
    ])
    ui$.selectedFolder.set('Work/Reports')
    responses['mail.folderDelete'] = {
      ok: true,
      deleted: 3,
      removed: ['Work/Reports', 'Work'],
      folders: [{ id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 }],
    }

    const pending = deleteFolder('acc', 'Work', 'Work', 1)
    settleConfirm(true)

    expect(await pending).toBe(true)
    expect(settings$.kanbanBoards.get()[0].columns).toEqual([{ accountId: 'acc', folderId: 'inbox' }])
    // The mailbox was sitting in the subfolder, not in the folder deleted.
    expect(ui$.selectedFolder.get()).toBe('inbox')
  })

  it('reconciles folders removed before a later server DELETE fails', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'b1',
        name: 'Board',
        columns: [
          { accountId: 'acc', folderId: 'Work' },
          { accountId: 'acc', folderId: 'Work/Reports' },
        ],
      },
    ])
    ui$.selectedFolder.set('Work/Reports')
    responses['mail.folderDelete'] = {
      ok: false,
      deleted: 1,
      removed: ['Work/Reports'],
      warning: 'One subfolder was deleted before the server rejected Work',
      folders: [{ id: 'Work', account_id: 'acc', name: 'Work', role: 'folder', unread: 0 }],
    }

    const pending = deleteFolder('acc', 'Work', 'Work', 1)
    settleConfirm(true)

    expect(await pending).toBe(false)
    expect(settings$.kanbanBoards.get()[0].columns).toEqual([{ accountId: 'acc', folderId: 'Work' }])
    expect(ui$.selectedFolder.get()).toBe('inbox')
    expect(ui$.toast.get()).toBe('One subfolder was deleted before the server rejected Work')
  })

  it('clears a selected thread that exists only in the deleted Kanban column', async () => {
    const selected = thread({
      id: 'acc#Work#t.work',
      thread_id: 'acc#Work#t.work',
      account_id: 'acc',
      folder_id: 'Work',
    })
    kanban$.threads['acc\nWork'].set([selected])
    ui$.selectedThread.set(selected.thread_id)

    const pending = deleteFolder('acc', 'Work', 'Work')
    settleConfirm(true)

    expect(await pending).toBe(true)
    expect(ui$.selectedThread.get()).toBe('')
    expect(kanban$.threads['acc\nWork'].get()).toBeUndefined()
  })

  it('does nothing when the confirm is declined', async () => {
    const pending = deleteFolder('acc', 'Work', 'Work')
    settleConfirm(false)

    expect(await pending).toBe(false)
    expect(calls.filter((call) => call.command === 'mail.folderDelete')).toHaveLength(0)
    expect(settings$.kanbanBoards.get()[0].columns).toHaveLength(2)
  })

  it('keeps the folder and reports the error when the backend refuses', async () => {
    ;(window as any).go.main.App.Invoke = async () => {
      throw new Error('Work is a special folder and cannot be deleted')
    }

    const pending = deleteFolder('acc', 'Work', 'Work')
    settleConfirm(true)

    expect(await pending).toBe(false)
    expect(settings$.kanbanBoards.get()[0].columns).toHaveLength(2)
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Work is a special folder and cannot be deleted')
  })
})

describe('ensureAccountFolders', () => {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    calls.length = 0
    mail$.foldersByAccount.set({
      acc: [
        { id: 'INBOX', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Work', account_id: 'acc', name: 'Work', role: 'folder', unread: 0 },
      ],
    })
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return { folders: [] }
          },
        },
      },
    }
  })

  it('serves the cache without touching the server by default', async () => {
    const folders = await ensureAccountFolders('acc')

    expect(folders.map((folder) => folder.id)).toEqual(['INBOX', 'Work'])
    expect(calls).toHaveLength(0)
  })

  // A populated cache is not a current one: folders created in webmail only land
  // through a server-side LIST, which is what refresh:true kicks off.
  it('kicks a server-side list on forceRefresh while still serving the cache', async () => {
    const folders = await ensureAccountFolders('acc', { forceRefresh: true })

    expect(folders.map((folder) => folder.id)).toEqual(['INBOX', 'Work'])
    expect(calls).toEqual([{ command: 'mail.folderList', payload: { account_id: 'acc', refresh: true } }])
  })
})
