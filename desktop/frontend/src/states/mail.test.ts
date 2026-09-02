import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Folder, Message } from '../types'
import { accounts$ } from './accounts'
import { kanban$ } from './kanban'
import {
  archiveThread,
  deletableFolder,
  deleteFolder,
  bulkArchiveSelected,
  bulkDeleteSelected,
  bulkMarkSelectedUnread,
  copyThreadToFolder,
  deleteThread,
  discardSavedDraftCopy,
  ensureAccountFolders,
  loadMoreThreads,
  loadThread,
  loadThreads,
  mail$,
  markAllRead,
  markMessagesRead,
  mergeRefreshedThreadMessages,
  requestThreadReselect,
  threadListViewKey,
  moveThreadToFolder,
} from './mail'
import { settings$ } from './settings'
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

describe('thread message refresh reconciliation', () => {
  it('keeps an optimistic reply until the canonical Sent copy arrives', () => {
    const original = thread({ id: 'm1', thread_id: 't1', message_id: 'root@example.com', date: 100 })
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 200,
    })

    expect(mergeRefreshedThreadMessages([original, optimistic], [original], 't1')).toEqual([original, optimistic])

    const canonical = thread({
      id: 'sent:2',
      folder_id: 'Sent',
      thread_id: 't1',
      message_id: '<REPLY@example.com>',
      date: 200,
    })
    expect(mergeRefreshedThreadMessages([original, optimistic], [original, canonical], 't1')).toEqual([
      original,
      canonical,
    ])
  })

  it('drops an optimistic reply once a Sent copy arrives under a rewritten Message-ID', () => {
    // Proton Bridge stores the Sent copy under an id of its own, so the id we
    // generated never comes back and only the envelope identifies the copy.
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'Sender <sender@example.com>',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 1000,
    })
    const canonical = thread({
      id: 'sent:2',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'abc@protonmail.internalid',
      outgoing: true,
      date: 1004,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [canonical], 't1')).toEqual([canonical])
  })

  it('collapses two identical replies onto two server copies, not one', () => {
    const optimistic = (id: string, date: number) =>
      thread({
        id,
        thread_id: 't1',
        from_addr: 'me@example.com',
        to: 'sender@example.com',
        subject: 'Re: Subject',
        message_id: `${id}@example.com`,
        send_status: 'sent',
        date,
      })
    const canonical = (id: string, date: number) =>
      thread({
        id,
        folder_id: 'Sent',
        thread_id: 't1',
        from_addr: 'me@example.com',
        to: 'sender@example.com',
        subject: 'Re: Subject',
        message_id: `${id}@protonmail.internalid`,
        outgoing: true,
        date,
      })

    const first = optimistic('sent-1', 1000)
    const second = optimistic('sent-2', 1010)
    const serverCopy = canonical('sent:1', 1001)

    // One copy back so far: the second reply keeps its bubble.
    expect(mergeRefreshedThreadMessages([first, second], [serverCopy], 't1')).toEqual([serverCopy, second])
    // Both back: no bubbles left over.
    expect(mergeRefreshedThreadMessages([first, second], [serverCopy, canonical('sent:2', 1011)], 't1')).toHaveLength(2)
  })

  it('pairs a Sent copy with the reply it belongs to, not the first bubble', () => {
    // Two replies in one thread share sender, subject, recipients and a
    // timestamp minutes apart — the envelope alone cannot tell them apart. The
    // second one's copy comes back first.
    const reply = (id: string, body: string, date: number) =>
      thread({
        id,
        thread_id: 't1',
        from_addr: 'me@example.com',
        to: 'sender@example.com',
        subject: 'Re: Subject',
        message_id: `${id}@example.com`,
        body,
        send_status: 'sent',
        date,
      })
    const first = reply('sent-1', 'First reply', 1000)
    const second = reply('sent-2', 'Second reply', 1010)
    const secondCopy = thread({
      id: 'sent:b',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'b@protonmail.internalid',
      body: 'Second reply',
      outgoing: true,
      date: 1011,
    })

    // The first reply keeps its bubble; only the second one is resolved.
    expect(mergeRefreshedThreadMessages([first, second], [secondCopy], 't1').map((m) => m.id)).toEqual([
      'sent-1',
      'sent:b',
    ])
  })

  it('falls back to the closest send time when the server reflowed the body', () => {
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      body: 'A reply long enough to wrap',
      send_status: 'sent',
      date: 1000,
    })
    const canonical = thread({
      id: 'sent:1',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'abc@protonmail.internalid',
      // Same words, rewrapped by the submission server.
      body: 'A reply long enough\nto wrap',
      outgoing: true,
      date: 1003,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [canonical], 't1').map((m) => m.id)).toEqual(['sent:1'])
  })

  it('does not let a newly saved draft claim an optimistic reply', () => {
    // The autosaved draft of the *next* reply: outgoing, same envelope, minutes
    // apart — but a draft is not a sent copy.
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 1000,
    })
    const draft = thread({
      id: 'draft:1',
      folder_id: 'Drafts',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'next-draft@example.com',
      outgoing: true,
      date: 1005,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [draft], 't1').map((m) => m.id)).toEqual([
      'sent-local',
      'draft:1',
    ])
  })

  it('does not mistake the message being replied to for the Sent copy', () => {
    // A thread the user talks to themselves in: same sender, same recipients,
    // minutes apart. Only the reply is a local send.
    const original = thread({
      id: 'm1',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'me@example.com',
      subject: 'Re: Subject',
      message_id: 'root@example.com',
      outgoing: true,
      date: 1000,
    })
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'me@example.com',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 1060,
    })

    expect(mergeRefreshedThreadMessages([original, optimistic], [original], 't1')).toEqual([original, optimistic])
  })

  it('does not carry an optimistic reply into another thread', () => {
    const optimistic = thread({ id: 'sent-local', thread_id: 't1', send_status: 'sending' })
    const other = thread({ id: 'm2', thread_id: 't2' })

    expect(mergeRefreshedThreadMessages([optimistic], [other], 't2')).toEqual([other])
  })
})

describe('thread list paging', () => {
  beforeEach(() => {
    mail$.threads.set([thread({ subject: 'Old first page' })])
    mail$.threadsCursor.set('old-cursor')
    mail$.threadsLoadingMore.set(false)
    mail$.threadAccountCursors.set({})
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.query.set('old')
    ui$.filterMode.set('all')
  })

  afterEach(() => {
    ui$.query.set('')
  })

  it('drops a load-more response after a newer search replaces the list', async () => {
    let resolveOldPage!: (value: unknown) => void
    const oldPage = new Promise((resolve) => {
      resolveOldPage = resolve
    })
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { before_cursor?: string; query?: string }) => {
            if (payload.before_cursor === 'old-cursor') return oldPage
            if (payload.query === 'new') {
              return {
                threads: [thread({ thread_id: 'acc:inbox:thread:new', subject: 'New result' })],
                next_cursor: 'new-cursor',
              }
            }
            return {}
          },
        },
      },
    }

    const loadingMore = loadMoreThreads()
    ui$.query.set('new')
    await loadThreads()
    resolveOldPage({
      threads: [thread({ thread_id: 'acc:inbox:thread:old-page', subject: 'Stale result' })],
      next_cursor: 'stale-cursor',
    })
    await loadingMore

    expect(mail$.threads.get().map((item) => item.subject)).toEqual(['New result'])
    expect(mail$.threadsCursor.get()).toBe('new-cursor')
  })

  it('paints cached search results before replacing them with live results', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            if (payload.refresh) return livePage
            return {
              threads: [thread({ thread_id: 'acc:inbox:thread:cached', subject: 'Cached result' })],
              next_cursor: 'cached-cursor',
            }
          },
        },
      },
    }

    ui$.query.set('deploy')
    const loading = loadThreads()
    await nextTick()

    expect(refreshes).toEqual([false, true])
    expect(mail$.threads.get().map((item) => item.subject)).toEqual(['Cached result'])
    expect(mail$.threadsCursor.get()).toBe('cached-cursor')
    // Neither stage opens a result on its own — that would mark it read.
    expect(ui$.selectedThread.get()).toBe('')

    resolveLive({
      threads: [thread({ thread_id: 'acc:inbox:thread:live', subject: 'Live result' })],
      next_cursor: 'live-cursor',
    })
    await loading

    expect(mail$.threads.get().map((item) => item.subject)).toEqual(['Live result'])
    expect(mail$.threadsCursor.get()).toBe('live-cursor')
    expect(ui$.selectedThread.get()).toBe('')
  })
})

describe('thread selection on load', () => {
  beforeEach(() => {
    mail$.threads.set([])
    mail$.messages.set([])
    mail$.threadsCursor.set('')
    mail$.threadAccountCursors.set({})
    mail$.threadLoading.set(false)
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.query.set('')
    ui$.filterMode.set('all')
    kanban$.activeBoardId.set('')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async () => ({
            threads: [thread({ thread_id: 'acc:inbox:thread:top', unread: true })],
          }),
        },
      },
    }
  })

  afterEach(() => {
    kanban$.activeBoardId.set('')
  })

  it('leaves the conversation pane closed when a view is opened', async () => {
    await loadThreads()

    expect(mail$.threads.get()).toHaveLength(1)
    // Opening a thread marks its visible messages read, so switching to a view
    // must never do it on the user's behalf.
    expect(ui$.selectedThread.get()).toBe('')
  })

  it('closes a selection the new view does not contain instead of opening another thread', async () => {
    // The conversation is open and fully loaded, as it is after any ordinary
    // click — its messages must not keep it selected in a view it is absent from.
    ui$.selectedThread.set('acc:other:thread:9')
    mail$.messages.set([thread({ id: 'acc:other:thread:9#1', thread_id: 'acc:other:thread:9' })])

    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('')
  })

  it('keeps a selection whose conversation is still loading', async () => {
    ui$.selectedThread.set('acc:other:thread:9')
    mail$.threadLoading.set(true)

    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('acc:other:thread:9')
  })

  it('keeps the open thread when it is still in the loaded view', async () => {
    ui$.selectedThread.set('acc:inbox:thread:top')

    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('acc:inbox:thread:top')
  })

  it('does not replace the waiting mail list while a kanban card owns the folder selection', async () => {
    const sent = thread({ folder_id: 'sent', subject: 'Unified sent' })
    let calls = 0
    mail$.threads.set([sent])
    ui$.selectedFolder.set('[Gmail]/Sent Mail')
    kanban$.activeBoardId.set('board-1')
    ;(window as any).go.main.App.Invoke = async () => {
      calls += 1
      return { threads: [thread({ subject: 'Unified inbox' })] }
    }

    await loadThreads()

    expect(calls).toBe(0)
    expect(mail$.threads.get()).toEqual([sent])
  })

  it('drops a reselect request the skipped kanban load was meant to answer', async () => {
    kanban$.activeBoardId.set('board-1')
    // Deleting the last card of a column clears the pane and asks the load that
    // follows to pick the replacement — a load that never runs behind a board.
    requestThreadReselect()

    await loadThreads()

    kanban$.activeBoardId.set('')
    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('')
  })
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

  it('opens the replacement thread when the deleted one had no visible neighbour', async () => {
    const replacement = thread({
      id: 'acc:inbox:thread:2#201',
      thread_id: 'acc:inbox:thread:2',
      date: Math.floor(Date.parse('2026-06-10T12:00:00Z') / 1000),
    })
    responses['mail.delete'] = { ok: true, deleted: 2 }
    // The deleted thread was the only row we had, so the neighbour comes from
    // the refreshed list rather than from the local one.
    responses['mail.threadList'] = { threads: [replacement] }

    await deleteThread('acc:inbox:thread:1')

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

  it('clears the thread draft badge when the quick reply draft is discarded', async () => {
    const inboxThread = thread({
      has_draft: true,
      preview: 'test',
    })
    const inboxMessage = thread({
      id: 'acc:inbox:thread:1#101',
      folder_id: 'inbox',
      message_id: 'root@example.com',
    })
    const draftMessage = thread({
      id: 'acc:Drafts:thread:1#201',
      folder_id: 'Drafts',
      // IMAP commonly returns the header form while the identity allocated for
      // save/discard is bare. They identify the same draft.
      message_id: '<DRAFT-ID@example.com>',
      body: 'test',
    })
    mail$.threads.set([inboxThread])
    mail$.messages.set([inboxMessage, draftMessage])
    responses['mail.discardDraft'] = { ok: true, deleted: 1, permanent: true }
    responses['mail.threadList'] = { threads: [{ ...inboxThread, has_draft: true }] }

    await discardSavedDraftCopy({
      threadId: 'acc:inbox:thread:1',
      messageId: '',
      folderId: '',
      accountId: 'acc',
      draftMessageId: 'draft-id@example.com',
    })

    expect(mail$.messages.get()).toEqual([inboxMessage])
    expect(mail$.threads.get()[0]).toMatchObject({ has_draft: false, preview: 'test' })
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

    await bulkMarkSelectedUnread([bulkItem(first), bulkItem(second)])

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

describe('thread list view identity', () => {
  const currentKey = () =>
    threadListViewKey(ui$.selectedAccount.get(), ui$.selectedFolder.get(), ui$.query.get(), ui$.filterMode.get())

  beforeEach(() => {
    mail$.threads.set([])
    mail$.threadsLoadedKey.set('')
    mail$.threadsCursor.set('')
    mail$.threadAccountCursors.set({})
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.query.set('')
    ui$.filterMode.set('all')
    ;(window as any).go = {
      main: { App: { Invoke: async () => ({ threads: [], next_cursor: '' }) } },
    }
  })

  afterEach(() => {
    ui$.query.set('')
    ui$.filterMode.set('all')
    mail$.threadsLoadedKey.set('')
  })

  it('does not call a folder loaded until its load lands', async () => {
    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    await loadThreads()

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // The flash this guards against: the list repaints on the navigation itself,
  // a frame or more before the effect that calls loadThreads runs, so a flag
  // the loader sets would still be off for that paint.
  it('stops matching the moment the folder or filter changes, before any load runs', async () => {
    await loadThreads()
    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())

    ui$.selectedFolder.set('archive')
    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    await loadThreads()
    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())

    ui$.filterMode.set('unread')
    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())
  })

  // A sync event or feed edit refreshes the list on its own schedule. Landing in
  // the gap between a keystroke and the debounced search, it would otherwise
  // settle the pending query on cache-only rows.
  it('is not settled by a background refresh that lands mid-navigation', async () => {
    await loadThreads()

    ui$.selectedFolder.set('archive')
    await loadThreads(false)

    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    await loadThreads()

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // The overlap: a sync event fires while the view's own load is still out. If
  // the background call took the version, the foreground one would lose
  // `superseded` on arrival, and with no load left to settle the view the list
  // would spin for good.
  it('settles when a sync event fires while the view load is still in flight', async () => {
    let resolveForeground!: (value: unknown) => void
    const foregroundPage = new Promise((resolve) => {
      resolveForeground = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            if (payload.refresh) return foregroundPage
            return { threads: [], next_cursor: '' }
          },
        },
      },
    }

    const foreground = loadThreads()
    await nextTick()
    // The sync event, mid-flight.
    await loadThreads(false)

    expect(refreshes).toEqual([true])

    resolveForeground({ threads: [], next_cursor: '' })
    await foreground

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // Stepping aside is not the same as being answered: the load in flight read the
  // cache before the change the refresh is reacting to. A quick reply's post-send
  // draft discard used to run its refresh while the view's own load was still
  // out, and the rows that load then wrote still counted the discarded draft, so
  // the card showed one message too many until something else reloaded the list.
  it('runs a background refresh that stepped aside once the view load lands', async () => {
    let resolveForeground!: (value: unknown) => void
    const foregroundPage = new Promise((resolve) => {
      resolveForeground = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            if (payload.refresh) return foregroundPage
            return { threads: [], next_cursor: '' }
          },
        },
      },
    }

    const foreground = loadThreads()
    await nextTick()
    // The post-send discard's refresh, while the view load is still out.
    await loadThreads(false)
    expect(refreshes).toEqual([true])

    resolveForeground({ threads: [], next_cursor: '' })
    await foreground
    await nextTick()

    expect(refreshes).toEqual([true, false])
    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // Stepping aside for the load in flight is only right while that load is still
  // going to write. A search superseded by the next keystroke used to hold the
  // claim for good, so every later background refresh of that view was skipped —
  // including the one a post-send draft discard runs, leaving the card's Draft
  // badge and message count stuck at what they were before the send.
  it('refreshes in the background again after a superseded search load', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            return payload.refresh ? livePage : { threads: [], next_cursor: '' }
          },
        },
      },
    }

    ui$.query.set('0821')
    const stale = loadThreads()
    await nextTick()
    // The next keystroke, then a correction back to the same query: the load
    // above lands with nothing left to write.
    ui$.query.set('0821 ')
    resolveLive({ threads: [], next_cursor: '' })
    await stale
    ui$.query.set('0821')

    refreshes.length = 0
    await loadThreads(false)

    expect(refreshes).toEqual([false])
  })

  // Same claim, still out on the wire: a search another load has overtaken will
  // drop whatever it returns, so waiting for it — a slow one can take the whole
  // 15s timeout — buys nothing and costs every background refresh in between.
  it('refreshes in the background while a superseded search load is still out', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            return payload.refresh ? livePage : { threads: [], next_cursor: '' }
          },
        },
      },
    }

    ui$.query.set('0821')
    const stale = loadThreads()
    await nextTick()
    // The next keystroke paints its own cache hits, then the user deletes it
    // again — the search above is now writing for nobody, still on the wire.
    ui$.query.set('0821 ')
    await loadThreads(false, 'cache')
    ui$.query.set('0821')

    refreshes.length = 0
    await loadThreads(false)

    expect(refreshes).toEqual([false])

    resolveLive({ threads: [], next_cursor: '' })
    await stale
  })

  // A search paints local hits first, then the live IMAP results. An empty cache
  // stage must not claim the search is answered while the live half is still out.
  it('waits for the live stage of a search, not the cache stage', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) =>
            payload.refresh ? livePage : { threads: [], next_cursor: '' },
        },
      },
    }

    ui$.query.set('deploy')
    const loading = loadThreads()
    await nextTick()

    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    resolveLive({ threads: [], next_cursor: '' })
    await loading

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })
})
