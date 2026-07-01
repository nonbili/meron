import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { kanban$ } from './kanban'
import {
  archiveThread,
  bulkArchiveSelected,
  bulkDeleteSelected,
  bulkMarkSelectedUnread,
  copyThreadToFolder,
  deleteThread,
  discardSavedDraftCopy,
  mail$,
  markAllRead,
  moveThreadToFolder,
} from './mail'
import { runToastUndo, settleConfirm, toggleBulkSelection, ui$, type BulkSelectionItem } from './ui'

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

const bulkItem = (message: Message, overrides: Partial<BulkSelectionItem> = {}): BulkSelectionItem => ({
  key: `test:${message.id}`,
  groupKey: 'test:column',
  threadId: message.thread_id,
  accountId: message.account_id,
  folderId: message.folder_id,
  surface: 'thread-list',
  kind: 'mail',
  unread: message.unread,
  starred: message.starred,
  draft: false,
  trash: false,
  ...overrides,
})

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
        signature: '',
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

describe('deleteThread', () => {
  const calls: { command: string; payload: unknown }[] = []
  // What the mocked backend returns per command; tests override per scenario.
  let responses: Record<string, unknown> = {}

  beforeEach(() => {
    calls.length = 0
    responses = { 'mail.delete': { ok: true, deleted: 0 } }
    mail$.threads.set([thread()])
    mail$.messages.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
    ])
    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
      ],
    })
    kanban$.threads.set({})
    ui$.selectedThread.set('acc:inbox:thread:1')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
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

  it('rolls back and shows an error when delete affects no messages', async () => {
    await deleteThread('acc:inbox:thread:1')

    expect(calls.filter((call) => call.command === 'mail.delete')).toHaveLength(1)
    expect(calls.some((call) => call.command === 'mail.threadList')).toBe(false)
    expect(mail$.threads.get()).toHaveLength(1)
    expect(ui$.selectedThread.get()).toBe('acc:inbox:thread:1')
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Delete failed: no matching messages found')
  })

  it('shows an error when the backend reports success but the thread survives the refresh', async () => {
    responses['mail.delete'] = { ok: true, deleted: 2 }
    responses['mail.threadList'] = { threads: [thread()] }

    await deleteThread('acc:inbox:thread:1')

    const threadListCall = calls.find((call) => call.command === 'mail.threadList')
    expect(threadListCall?.payload).toMatchObject({ refresh: true })
    expect(mail$.threads.get()).toHaveLength(1)
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Delete failed: thread is still in this folder')
  })

  it('shows the success toast when the thread is gone after the refresh', async () => {
    responses['mail.delete'] = { ok: true, deleted: 2 }
    responses['mail.threadList'] = { threads: [] }

    await deleteThread('acc:inbox:thread:1')

    const threadListCall = calls.find((call) => call.command === 'mail.threadList')
    expect(threadListCall?.payload).toMatchObject({ refresh: true })
    expect(mail$.threads.get()).toHaveLength(0)
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Thread moved to Trash')
  })

  it('advances the selection to the next thread after deleting the selected one', async () => {
    const second = thread({
      id: 'acc:inbox:thread:2#201',
      thread_id: 'acc:inbox:thread:2',
      date: Math.floor(Date.parse('2026-06-10T12:00:00Z') / 1000),
    })
    mail$.threads.set([thread(), second])
    responses['mail.delete'] = { ok: true, deleted: 2 }
    responses['mail.threadList'] = { threads: [second] }

    await deleteThread('acc:inbox:thread:1')

    expect(mail$.threads.get()).toHaveLength(1)
    expect(ui$.selectedThread.get()).toBe('acc:inbox:thread:2')
  })

  it('advances the selection to the column neighbour when deleting in kanban view', async () => {
    const second = thread({
      id: 'acc:inbox:thread:2#201',
      thread_id: 'acc:inbox:thread:2',
      date: Math.floor(Date.parse('2026-06-10T12:00:00Z') / 1000),
    })
    kanban$.activeBoardId.set('board')
    kanban$.threads.set({ 'acc:inbox': [thread(), second] })
    kanban$.paneThreadId.set('acc:inbox:thread:1')
    responses['mail.delete'] = { ok: true, deleted: 2 }
    responses['mail.threadList'] = { threads: [second] }

    await deleteThread('acc:inbox:thread:1')

    expect(ui$.selectedThread.get()).toBe('acc:inbox:thread:2')
    expect(kanban$.paneThreadId.get()).toBe('acc:inbox:thread:2')
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
  })

  it('permanently deletes threads already in Trash', async () => {
    const trashThread = thread({
      id: 'acc:Trash:thread:1#101',
      folder_id: 'Trash',
      thread_id: 'acc:Trash:thread:1',
    })
    mail$.threads.set([trashThread])
    ui$.selectedThread.set('acc:Trash:thread:1')
    ui$.selectedFolder.set('Trash')
    responses['mail.delete'] = { ok: true, deleted: 1, permanent: true }
    responses['mail.threadList'] = { threads: [] }

    const pending = deleteThread('acc:Trash:thread:1')
    settleConfirm(true)

    await pending

    expect(calls.find((call) => call.command === 'mail.delete')?.payload).toMatchObject({
      thread_id: 'acc:Trash:thread:1',
      folder: 'Trash',
    })
    expect(mail$.threads.get()).toHaveLength(0)
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Thread deleted')
  })

  it('discards drafts permanently with draft wording (engine expunges in place)', async () => {
    const draftThread = thread({
      id: 'acc:Drafts:thread:1#101',
      folder_id: 'Drafts',
      thread_id: 'acc:Drafts:thread:1',
    })
    mail$.threads.set([draftThread])
    ui$.selectedThread.set('acc:Drafts:thread:1')
    ui$.selectedFolder.set('Drafts')
    responses['mail.delete'] = { ok: true, deleted: 1, permanent: true }
    responses['mail.threadList'] = { threads: [] }

    const pending = deleteThread('acc:Drafts:thread:1')
    settleConfirm(true)

    await pending

    expect(calls.find((call) => call.command === 'mail.delete')?.payload).toMatchObject({
      thread_id: 'acc:Drafts:thread:1',
      folder: 'Drafts',
    })
    expect(mail$.threads.get()).toHaveLength(0)
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Draft discarded')
  })

  it('sends the displayed Trash folder even when the thread id still encodes another folder', async () => {
    const movedThread = thread({
      folder_id: 'Trash',
    })
    mail$.threads.set([movedThread])
    ui$.selectedFolder.set('Trash')
    responses['mail.delete'] = { ok: true, deleted: 1, permanent: true }
    responses['mail.threadList'] = { threads: [] }

    const pending = deleteThread('acc:inbox:thread:1')
    settleConfirm(true)

    await pending

    expect(calls.find((call) => call.command === 'mail.delete')?.payload).toMatchObject({
      thread_id: 'acc:inbox:thread:1',
      folder: 'Trash',
    })
    expect(ui$.toast.get()).toBe('Thread deleted')
  })

  it('silently discards the saved draft copy after sending', async () => {
    const draftThread = thread({
      id: 'acc:Drafts:thread:1#101',
      folder_id: 'Drafts',
      thread_id: 'acc:Drafts:thread:1',
    })
    const draftMessage = thread({
      id: 'acc:Drafts:thread:1#101',
      folder_id: 'Drafts',
      thread_id: 'acc:Drafts:thread:1',
    })
    mail$.threads.set([draftThread])
    mail$.messages.set([draftMessage])
    ui$.selectedThread.set('acc:Drafts:thread:1')
    ui$.selectedFolder.set('Drafts')
    responses['mail.delete'] = { ok: true, deleted: 1, permanent: true }
    responses['mail.threadList'] = { threads: [] }

    await discardSavedDraftCopy({
      threadId: 'acc:Drafts:thread:1',
      messageId: 'acc:Drafts:thread:1#101',
      folderId: 'Drafts',
      accountId: 'acc',
    })

    expect(calls.find((call) => call.command === 'mail.delete')?.payload).toMatchObject({
      thread_id: 'acc:Drafts:thread:1',
      message_ids: ['acc:Drafts:thread:1#101'],
      folder: 'Drafts',
    })
    expect(mail$.threads.get()).toHaveLength(0)
    expect(mail$.messages.get()).toHaveLength(0)
    expect(ui$.toast.get()).toBe('')
  })

  it('discards an autosaved compose draft by stable draft message id', async () => {
    responses['mail.discardDraft'] = { ok: true, deleted: 0, permanent: true }
    responses['mail.threadList'] = { threads: [] }

    await discardSavedDraftCopy({
      threadId: '',
      messageId: '',
      folderId: '',
      accountId: 'acc',
      draftMessageId: 'draft-id@example.com',
    })

    expect(calls.find((call) => call.command === 'mail.discardDraft')?.payload).toMatchObject({
      account_id: 'acc',
      draft_id: 'draft-id@example.com',
    })
    expect(calls.some((call) => call.command === 'mail.delete')).toBe(false)
    expect(ui$.toast.get()).toBe('')
  })
})

describe('moveThreadToFolder undo', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.move': [
        { ok: true, moved: 1 },
        { ok: true, moved: 1 },
      ],
      'mail.threadList': [{ threads: [] }, { threads: [thread({ thread_id: 'acc#inbox#t.MQ' })] }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('offers undo for a successful folder move', async () => {
    await moveThreadToFolder('acc#inbox#t.MQ', 'Work')

    expect(calls.find((call) => call.command === 'mail.move')?.payload).toMatchObject({
      thread_id: 'acc#inbox#t.MQ',
      target_folder_id: 'Work',
    })
    expect(ui$.toast.get()).toBe('Thread moved')
    expect(ui$.toastUndo.peek()).toBeTruthy()

    runToastUndo()
    await nextTick()

    expect(calls.filter((call) => call.command === 'mail.move')[1]?.payload).toMatchObject({
      thread_id: 'acc#Work#t.MQ',
      target_folder_id: 'inbox',
    })
    expect(ui$.toast.get()).toBe('Thread moved')
    expect(ui$.toastUndo.peek()).toBeNull()
  })
})

describe('copyThreadToFolder', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.copy': [{ ok: true, copied: 1 }],
      'mail.threadList': [{ threads: [thread({ thread_id: 'acc#inbox#t.MQ' })] }],
      'mail.folderList': [{ folders: [] }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('copies a thread to the requested account and folder', async () => {
    await copyThreadToFolder('acc#inbox#t.MQ', 'acc2', 'Archive')

    expect(calls.find((call) => call.command === 'mail.copy')?.payload).toMatchObject({
      thread_id: 'acc#inbox#t.MQ',
      target_account_id: 'acc2',
      target_folder_id: 'Archive',
    })
    expect(calls.some((call) => call.command === 'mail.delete')).toBe(false)
    expect(ui$.toast.get()).toBe('Thread copied')
  })

  it('reports an error when copy affects no messages', async () => {
    responses['mail.copy'] = [{ ok: true, copied: 0 }]

    await copyThreadToFolder('acc#inbox#t.MQ', 'acc2', 'Archive')

    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Copy failed: no matching messages found')
  })
})

describe('deleteThread trash undo', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.delete': [{ ok: true, deleted: 1, trash: 'Trash', thread_id: 'acc#Trash#t.MQ' }],
      'mail.move': [{ ok: true, moved: 1 }],
      'mail.threadList': [{ threads: [] }, { threads: [thread({ thread_id: 'acc#inbox#t.MQ' })] }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
    ])
    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
      ],
    })
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('offers undo for moving a thread to Trash', async () => {
    await deleteThread('acc#inbox#t.MQ')

    expect(ui$.toast.get()).toBe('Thread moved to Trash')
    expect(ui$.toastUndo.peek()).toBeTruthy()

    runToastUndo()
    await nextTick()

    expect(calls.find((call) => call.command === 'mail.move')?.payload).toMatchObject({
      thread_id: 'acc#Trash#t.MQ',
      target_folder_id: 'inbox',
    })
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['acc#inbox#t.MQ'])
    expect(ui$.toast.get()).toBe('Thread moved')
    expect(ui$.toastUndo.peek()).toBeNull()
  })

  it('does not offer undo for permanent delete', async () => {
    responses['mail.delete'] = [{ ok: true, deleted: 1, permanent: true }]
    mail$.threads.set([
      thread({
        id: 'acc#Trash#t.MQ',
        folder_id: 'Trash',
        thread_id: 'acc#Trash#t.MQ',
      }),
    ])
    ui$.selectedThread.set('acc#Trash#t.MQ')
    ui$.selectedFolder.set('Trash')
    responses['mail.threadList'] = [{ threads: [] }]

    const pending = deleteThread('acc#Trash#t.MQ')
    settleConfirm(true)

    await pending

    expect(ui$.toast.get()).toBe('Thread deleted')
    expect(ui$.toastUndo.peek()).toBeNull()
  })
})

describe('archiveThread undo', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.archive': [{ ok: true, moved: 1, folder: 'Archive', thread_id: 'acc#Archive#t.MQ' }],
      'mail.threadList': [{ threads: [] }, { threads: [thread({ thread_id: 'acc#inbox#t.MQ' })] }],
      'mail.move': [{ ok: true, moved: 1 }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Archive', account_id: 'acc', name: 'Archive', role: 'archive', unread: 0 },
    ])
    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Archive', account_id: 'acc', name: 'Archive', role: 'archive', unread: 0 },
      ],
    })
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('moves the archived copy back to the original folder', async () => {
    await archiveThread('acc#inbox#t.MQ')

    expect(mail$.threads.get()).toHaveLength(0)
    expect(ui$.toast.get()).toBe('Thread archived')
    expect(ui$.toastUndo.peek()).toBeTruthy()

    runToastUndo()
    await nextTick()

    expect(calls.find((call) => call.command === 'mail.move')?.payload).toMatchObject({
      thread_id: 'acc#Archive#t.MQ',
      target_folder_id: 'inbox',
    })
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['acc#inbox#t.MQ'])
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Thread moved')
  })

  it('rolls back and reports an error when archive moves no messages', async () => {
    responses['mail.archive'] = [{ ok: true, moved: 0, folder: 'Archive', thread_id: 'acc#Archive#t.MQ' }]
    responses['mail.threadList'] = []

    await archiveThread('acc#inbox#t.MQ')

    expect(calls.some((call) => call.command === 'mail.threadList')).toBe(false)
    expect(mail$.threads.get()).toHaveLength(1)
    expect(ui$.selectedThread.get()).toBe('acc#inbox#t.MQ')
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Archive failed: no matching messages found')
  })
})

describe('bulk thread actions', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.markRead': [{ ok: true }],
      'mail.archive': [{ ok: true, moved: 1 }],
      'mail.delete': [{ ok: true, deleted: 1 }],
      'mail.threadList': [{ threads: [] }],
      'mail.folderList': [{ folders: [] }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
    ])
    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
      ],
    })
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.bulkSelection.set({})
    ui$.bulkAnchorKey.set('')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('dedupes bulk unread by thread id', async () => {
    const first = thread({ id: 'm1', thread_id: 'acc#inbox#t.MQ', unread: false })
    const second = thread({ id: 'm2', thread_id: 'acc#inbox#t.MQ', unread: false })
    mail$.threads.set([first, second])

    await bulkMarkSelectedUnread([bulkItem(first, { messageId: 'm1' }), bulkItem(second, { messageId: 'm2' })])

    expect(calls.filter((call) => call.command === 'mail.markRead').map((call) => call.payload)).toEqual([
      { thread_id: 'acc#inbox#t.MQ', seen: false },
    ])
    expect(ui$.toast.get()).toBe('Marked unread')
  })

  it('rolls back all local removals when a bulk archive fails', async () => {
    const second = thread({ id: 'm2', thread_id: 'acc#inbox#t.NQ' })
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' }), second])
    responses['mail.archive'] = [
      { ok: true, moved: 1 },
      { ok: true, moved: 0 },
    ]
    responses['mail.threadList'] = []

    await bulkArchiveSelected(mail$.threads.get().map((item) => bulkItem(item)))

    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['acc#inbox#t.MQ', 'acc#inbox#t.NQ'])
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Archive failed: no matching messages found')
  })

  it('uses one confirmation for permanent bulk delete', async () => {
    const trash = thread({ id: 'trash', folder_id: 'Trash', thread_id: 'acc#Trash#t.MQ' })
    mail$.threads.set([trash])
    ui$.selectedFolder.set('Trash')
    responses['mail.delete'] = [{ ok: true, deleted: 1, permanent: true }]
    responses['mail.threadList'] = [{ threads: [] }]

    const pending = bulkDeleteSelected([bulkItem(trash, { trash: true })])
    settleConfirm(true)
    await pending

    expect(calls.filter((call) => call.command === 'mail.delete')).toHaveLength(1)
    expect(calls.find((call) => call.command === 'mail.delete')?.payload).toMatchObject({
      thread_id: 'acc#Trash#t.MQ',
      folder: 'Trash',
    })
    expect(ui$.toast.get()).toBe('Thread deleted')
  })

  it('replaces bulk selection when selecting a different group', () => {
    const first = thread({ id: 'm1', thread_id: 'acc#inbox#t.MQ' })
    const second = thread({ id: 'm2', thread_id: 'acc#work#t.NQ', folder_id: 'Work' })

    toggleBulkSelection(bulkItem(first, { groupKey: 'kanban:inbox' }))
    toggleBulkSelection(bulkItem(second, { groupKey: 'kanban:work' }))

    expect(Object.keys(ui$.bulkSelection.get())).toEqual(['test:m2'])
    expect(ui$.bulkSelection['test:m2'].get()?.groupKey).toBe('kanban:work')
  })
})
