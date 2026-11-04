import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { compose$ } from './composeState'
import type { MessageTab } from '../types'
import {
  kanban$,
  closeCurrentConversation,
  kanbanPaneThreadId,
  kanbanColumnKey,
  kanbanBoardColumnKey,
  selectAdjacentKanbanThread,
  focusKanbanThreadFolder,
  markColumnAllRead,
  markBoardAllRead,
  openCorrespondentMail,
  removeKanbanBoard,
  removeKanbanColumnsForFolder,
  reorderKanbanColumn,
  selectKanbanBoard,
  setKanbanBoardUnreadBadge,
  setKanbanBoardWallpaper,
  switchKanbanColumnFolder,
} from './kanban'
import { mail$ } from './mail'
import {
  applyMutationFolderUnreads,
  holdFolderUnread,
  refreshAccountFoldersCache,
  updateCachedFolderUnread,
} from './mailFolders'
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

  it('holds synthesized lowercase inbox counts against uppercase folder refreshes', async () => {
    updateCachedFolderUnread('acc1', 'inbox', 6)
    const settle = holdFolderUnread('acc1', 'inbox')
    updateCachedFolderUnread('acc1', 'inbox', 0, 'local')
    ;(window as any).go.main.App.Invoke = async () => ({
      folders: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 6 }],
    })
    await refreshAccountFoldersCache('acc1')
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(0)
    settle(0, true)
  })

  it('publishes a failed column rollback while a sibling still holds the folder', () => {
    updateCachedFolderUnread('acc1', 'inbox', 0)
    const failed = holdFolderUnread('acc1', 'inbox')
    const sibling = holdFolderUnread('acc1', 'INBOX')
    failed(6, false)
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(6)
    sibling(6, false)
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(6)
  })

  it('preserves confirmed counts when a later sibling cannot refresh after failure', () => {
    updateCachedFolderUnread('acc1', 'inbox', 0)
    const succeeded = holdFolderUnread('acc1', 'inbox')
    const failed = holdFolderUnread('acc1', 'INBOX')
    succeeded(2, true)
    failed(6, false)
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(2)
  })

  it('lets a post-write refresh supersede a mutation observed before that refresh started', async () => {
    updateCachedFolderUnread('acc1', 'inbox', 6)
    let release = () => {}
    const gate = new Promise<void>((resolve) => (release = resolve))
    ;(window as any).go.main.App.Invoke = async (command: string) => {
      if (command === 'mail.markAllRead') await gate
      if (command === 'mail.folderList') {
        return { folders: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 0 }] }
      }
      return { ok: true }
    }
    const pending = markColumnAllRead({ accountId: 'acc1', folderId: 'inbox' })
    applyMutationFolderUnreads({ folder_unreads: { acc1: { INBOX: 5 } } })
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(5)
    release()
    await pending
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(0)
  })

  it('keeps a concurrent mutation result when a column settles with an older refresh', async () => {
    updateCachedFolderUnread('acc1', 'inbox', 6)
    let release = () => {}
    let started = () => {}
    const gate = new Promise<void>((resolve) => (release = resolve))
    const refreshing = new Promise<void>((resolve) => (started = resolve))
    ;(window as any).go.main.App.Invoke = async (command: string) => {
      if (command === 'mail.folderList') {
        started()
        await gate
        return { folders: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 3 }] }
      }
      return { ok: true }
    }
    const pending = markColumnAllRead({ accountId: 'acc1', folderId: 'inbox' })
    await refreshing
    applyMutationFolderUnreads({ folder_unreads: { acc1: { INBOX: 1 } } })
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(1)
    release()
    await pending
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(1)
  })

  it('keeps navigation counts cleared when stale folder and thread refreshes race a write', async () => {
    const folder = { id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 6 }
    mail$.foldersByAccount.set({ acc1: [folder] })
    settings$.kanbanBoards.set([{ id: 'board', name: 'Board', columns: [{ accountId: 'acc1', folderId: 'INBOX' }] }])
    let releaseWrite = () => {}
    let releaseStale = () => {}
    const writeGate = new Promise<void>((resolve) => (releaseWrite = resolve))
    const staleGate = new Promise<void>((resolve) => (releaseStale = resolve))
    let written = false
    let delayRefresh = false
    ;(window as any).go.main.App.Invoke = async (command: string) => {
      if (command === 'mail.markAllRead') {
        await writeGate
        written = true
      }
      if (command === 'mail.folderList') {
        const unread = written ? 0 : 6
        if (delayRefresh) await staleGate
        return { folders: [{ ...folder, unread }] }
      }
      return { ok: true }
    }

    const pending = markBoardAllRead('board')
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(0)
    await refreshAccountFoldersCache('acc1')
    updateCachedFolderUnread('acc1', 'inbox', 6)
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(0)

    delayRefresh = true
    const stale = refreshAccountFoldersCache('acc1')
    delayRefresh = false
    releaseWrite()
    await pending
    releaseStale()
    await stale
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(0)

    // Once the operation is over, fresh incoming mail can raise the badge.
    updateCachedFolderUnread('acc1', 'inbox', 1)
    expect(mail$.foldersByAccount.acc1.get()[0].unread).toBe(1)
  })

  it('marks all board columns including unloaded and minimized folders, deduplicating unified overlap', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'board',
        name: 'Board',
        columns: [
          { accountId: 'acc1', folderId: 'INBOX' },
          { accountId: 'unified', folderId: 'inbox' },
          { accountId: 'acc2', folderId: 'Archive' },
          { accountId: 'acc2', folderId: 'Archive' },
        ],
      },
      { id: 'other', name: 'Other', columns: [{ accountId: 'acc1', folderId: 'Trash' }] },
    ])
    settings$.kanbanMinimizedColumns.set({ 'board\nacc2\nArchive': true })
    await markBoardAllRead('board')
    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'unified', folder_id: 'inbox' },
      { account_id: 'acc2', folder_id: 'Archive' },
    ])
  })

  it('marks only the matches of a saved search, never widening an account folder write to unified', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'board',
        name: 'Board',
        columns: [
          { accountId: 'acc1', folderId: 'INBOX' },
          { accountId: 'unified', folderId: 'inbox', query: 'invoice' },
        ],
      },
    ])
    kanban$.threads['unified\ninbox\ninvoice'].set([message({ thread_id: 'match' })])
    await markBoardAllRead('board')
    expect(calls.filter((call) => call.command === 'mail.markAllRead').map((call) => call.payload)).toEqual([
      { account_id: 'acc1', folder_id: 'INBOX' },
    ])
    expect(calls.filter((call) => call.command === 'mail.markRead').map((call) => call.payload)).toEqual([
      { thread_id: 'match' },
    ])
  })

  it('clears every column before the backend answers', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'board',
        name: 'Board',
        columns: [
          { accountId: 'acc1', folderId: 'INBOX' },
          { accountId: 'acc2', folderId: 'Archive' },
        ],
      },
    ])
    kanban$.threads['acc1\nINBOX'].set([message()])
    kanban$.unreadCounts['acc1\nINBOX'].set(3)
    kanban$.threads['acc2\nArchive'].set([message({ account_id: 'acc2', folder_id: 'Archive' })])
    let release = () => {}
    const gate = new Promise<void>((resolve) => (release = resolve))
    const original = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      if (command === 'mail.markAllRead') await gate
      return original(command, payload)
    }

    const pending = markBoardAllRead('board')

    expect(kanban$.threads['acc1\nINBOX'].get()[0].unread).toBe(false)
    expect(kanban$.unreadCounts['acc1\nINBOX'].get()).toBe(0)
    expect(kanban$.threads['acc2\nArchive'].get()[0].unread).toBe(false)
    // A column reload racing the write brings back pre-write rows and badge.
    kanban$.threads['acc1\nINBOX'].set([message()])
    kanban$.unreadCounts['acc1\nINBOX'].set(3)
    kanban$.threads['acc2\nArchive'].set([message({ account_id: 'acc2', folder_id: 'Archive' })])
    release()
    await pending
    expect(kanban$.unreadCounts['acc1\nINBOX'].get()).toBe(0)
    expect(kanban$.threads['acc1\nINBOX'].get()[0].unread).toBe(false)
    expect(kanban$.threads['acc2\nArchive'].get()[0].unread).toBe(false)
  })

  it('keeps failed columns unread while completing other columns and reporting a partial failure', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'board',
        name: 'Board',
        columns: [
          { accountId: 'unified', folderId: 'inbox' },
          { accountId: 'acc2', folderId: 'Archive' },
        ],
      },
    ])
    kanban$.threads['unified\ninbox'].set([message()])
    kanban$.unreadCounts['unified\ninbox'].set(4)
    kanban$.threads['acc2\nArchive'].set([message({ account_id: 'acc2', folder_id: 'Archive' })])
    const original = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      if (command === 'mail.markAllRead' && payload.account_id === 'unified') {
        return { ok: false, failures: [{ account_id: 'acc1', message: 'Offline' }] }
      }
      return original(command, payload)
    }
    await markBoardAllRead('board')
    expect(kanban$.threads['unified\ninbox'].get()[0].unread).toBe(true)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(4)
    expect(kanban$.threads['acc2\nArchive'].get()[0].unread).toBe(false)
    expect(ui$.toastTone.get()).toBe('error')
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

  it('clears the side navigation folder badges before the backend answers', async () => {
    mail$.foldersByAccount.set({
      acc1: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 5 }],
      acc2: [{ id: 'INBOX', account_id: 'acc2', name: 'Inbox', role: 'inbox', unread: 2 }],
    })
    kanban$.threads['unified\ninbox'].set([message()])
    kanban$.unreadCounts['unified\ninbox'].set(7)
    let release = () => {}
    const gate = new Promise<void>((resolve) => (release = resolve))
    const original = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      if (command === 'mail.markAllRead') await gate
      return original(command, payload)
    }

    const pending = markColumnAllRead({ accountId: 'unified', folderId: 'inbox' })

    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(0)
    expect(mail$.foldersByAccount.acc2.get()?.[0]?.unread).toBe(0)
    release()
    await pending
  })

  it('puts the side navigation folder badges back when the write fails', async () => {
    mail$.foldersByAccount.set({
      acc1: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 5 }],
    })
    kanban$.threads['acc1\nINBOX'].set([message()])
    kanban$.unreadCounts['acc1\nINBOX'].set(5)
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'mail.markAllRead') return { ok: false, failures: [{ message: 'Offline' }] }
      // The failed write left the folder unread on the server too.
      return { folders: [{ id: 'INBOX', account_id: payload.account_id, name: 'Inbox', role: 'inbox', unread: 5 }] }
    }

    await markColumnAllRead({ accountId: 'acc1', folderId: 'INBOX' })

    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(5)
    expect(kanban$.unreadCounts['acc1\nINBOX'].get()).toBe(5)
  })

  // A board-wide mark shares one write between overlapping columns, so a single
  // failure rolls back both — and the second column would otherwise have
  // snapshotted the zero the first one had already written.
  it('rolls overlapping board columns back to the count the folder had before the board cleared', async () => {
    settings$.kanbanBoards.set([
      {
        id: 'board',
        name: 'Board',
        columns: [
          { accountId: 'unified', folderId: 'inbox' },
          { accountId: 'acc1', folderId: 'INBOX' },
        ],
      },
    ])
    mail$.foldersByAccount.set({
      acc1: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 5 }],
    })
    kanban$.threads['unified\ninbox'].set([message()])
    kanban$.threads['acc1\nINBOX'].set([message()])
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'mail.markAllRead') return { ok: false, failures: [{ message: 'Offline' }] }
      return { folders: [{ id: 'INBOX', account_id: payload.account_id, name: 'Inbox', role: 'inbox', unread: 5 }] }
    }

    await markBoardAllRead('board')

    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(5)
  })

  // The refresh that follows the writes answers with the server's own counts, so
  // a folder a sibling write did mark read keeps that count instead of being
  // rolled back with the column that failed.
  it('keeps the refreshed count for a folder the failed column did not leave unread', async () => {
    mail$.foldersByAccount.set({
      acc1: [{ id: 'INBOX', account_id: 'acc1', name: 'Inbox', role: 'inbox', unread: 5 }],
      acc2: [{ id: 'INBOX', account_id: 'acc2', name: 'Inbox', role: 'inbox', unread: 3 }],
    })
    kanban$.threads['unified\ninbox'].set([message()])
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'mail.markAllRead') return { ok: false, failures: [{ account_id: 'acc1', message: 'Offline' }] }
      const unread = payload.account_id === 'acc1' ? 5 : 0
      return { folders: [{ id: 'INBOX', account_id: payload.account_id, name: 'Inbox', role: 'inbox', unread }] }
    }

    await markColumnAllRead({ accountId: 'unified', folderId: 'inbox' })

    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(5)
    expect(mail$.foldersByAccount.acc2.get()?.[0]?.unread).toBe(0)
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

describe('reorderKanbanColumn', () => {
  const inbox = { accountId: 'acc1', folderId: 'INBOX' }
  const archive = { accountId: 'acc1', folderId: 'Archive' }
  const sent = { accountId: 'acc1', folderId: 'Sent' }
  const order = () => settings$.kanbanBoards.get()[0].columns.map((column) => column.folderId)

  beforeEach(() => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [inbox, archive, sent] }])
  })

  it('moves a column into the slot of the one it was dropped on, in either direction', () => {
    reorderKanbanColumn('b1', inbox, sent)
    expect(order()).toEqual(['Archive', 'Sent', 'INBOX'])
    reorderKanbanColumn('b1', inbox, archive)
    expect(order()).toEqual(['INBOX', 'Archive', 'Sent'])
  })

  it('leaves the board alone for a column it does not have', () => {
    reorderKanbanColumn('b1', inbox, { accountId: 'acc2', folderId: 'INBOX' })
    expect(order()).toEqual(['INBOX', 'Archive', 'Sent'])
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

describe('removeKanbanBoard', () => {
  beforeEach(() => {
    settings$.kanbanBoards.set([
      { id: 'b1', name: 'Board', columns: [] },
      { id: 'b2', name: 'Other', columns: [] },
    ])
  })

  it('falls back to the neighbouring board when the open one is deleted', () => {
    kanban$.activeBoardId.set('b2')

    removeKanbanBoard('b2')

    expect(settings$.kanbanBoards.get().map((board) => board.id)).toEqual(['b1'])
    expect(kanban$.activeBoardId.get()).toBe('b1')
  })

  it('deletes the last board and leaves the board view', () => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [] }])
    kanban$.activeBoardId.set('b1')

    removeKanbanBoard('b1')

    expect(settings$.kanbanBoards.get()).toEqual([])
    expect(kanban$.activeBoardId.get()).toBe('')
  })
})

describe('closeCurrentConversation', () => {
  const tab = (threadId: string): MessageTab => ({
    id: `thread-${threadId}`,
    kind: 'thread',
    messageId: '',
    threadId,
    subject: threadId,
    from: 'sender@example.com',
    body: '',
    viewMode: 'plain',
  })

  beforeEach(() => {
    kanban$.activeBoardId.set('b1')
    kanban$.paneThreadId.set('t-card')
    kanban$.paneColumnKey.set('b1:INBOX')
    ui$.selectedThread.set('t-card')
    compose$.tabs.set([])
    compose$.activeTab.set('')
  })

  afterEach(() => {
    kanban$.activeBoardId.set('')
  })

  it('closes the pane when the card conversation is all it holds', () => {
    closeCurrentConversation()

    expect(kanban$.paneThreadId.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('')
    expect(ui$.mobilePane.get()).toBe('threads')
  })

  it('keeps the pane on a still-open tab instead of hiding it', () => {
    compose$.tabs.set([tab('t-tab')])

    closeCurrentConversation()

    expect(kanban$.paneThreadId.get()).toBe('')
    expect(compose$.activeTab.get()).toBe('thread-t-tab')
    expect(ui$.selectedThread.get()).toBe('t-tab')
    expect(compose$.conversationThread.get()).toBe('')
  })

  it('leaves an active tab alone and only closes the card conversation', () => {
    compose$.tabs.set([tab('t-tab')])
    compose$.activeTab.set('thread-t-tab')
    ui$.selectedThread.set('t-tab')

    closeCurrentConversation()

    expect(kanban$.paneThreadId.get()).toBe('')
    expect(compose$.activeTab.get()).toBe('thread-t-tab')
    expect(ui$.selectedThread.get()).toBe('t-tab')
  })
})

describe('setKanbanBoardWallpaper', () => {
  it('clears the background when Plain is picked', () => {
    settings$.kanbanBoards.set([
      { id: 'b1', name: 'Board', columns: [], wallpaper: { kind: 'preset', presetId: 'dots' } },
    ])

    setKanbanBoardWallpaper('b1', { kind: 'preset', presetId: 'plain' })

    expect(settings$.kanbanBoards.get()[0]).not.toHaveProperty('wallpaper')
  })
})

describe('setKanbanBoardUnreadBadge', () => {
  it('turns the badge on for one board and clears the field when turned off', () => {
    settings$.kanbanBoards.set([
      { id: 'b1', name: 'Board', columns: [] },
      { id: 'b2', name: 'Other', columns: [] },
    ])

    setKanbanBoardUnreadBadge('b1', true)
    expect(settings$.kanbanBoards.get()[0].showUnreadBadge).toBe(true)
    expect(settings$.kanbanBoards.get()[1]).not.toHaveProperty('showUnreadBadge')

    setKanbanBoardUnreadBadge('b1', false)
    expect(settings$.kanbanBoards.get()[0]).not.toHaveProperty('showUnreadBadge')
  })
})

describe('selectKanbanBoard', () => {
  const tab = (threadId: string): MessageTab => ({
    id: `thread-${threadId}`,
    kind: 'thread',
    messageId: '',
    threadId,
    subject: threadId,
    from: 'sender@example.com',
    body: '',
    viewMode: 'plain',
  })

  beforeEach(() => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [{ accountId: 'acc1', folderId: 'INBOX' }] }])
    kanban$.activeBoardId.set('')
    compose$.tabs.set([])
    compose$.activeTab.set('')
  })

  afterEach(() => {
    kanban$.activeBoardId.set('')
    compose$.tabs.set([])
    compose$.activeTab.set('')
  })

  it('clears the mail view selection when no tab is on screen', () => {
    ui$.selectedThread.set('t-mail')

    selectKanbanBoard('b1')

    expect(kanban$.activeBoardId.get()).toBe('b1')
    expect(ui$.selectedThread.get()).toBe('')
  })

  it('keeps the active thread tab rendering when switching from the mail view', () => {
    compose$.tabs.set([tab('t-tab')])
    compose$.activeTab.set('thread-t-tab')
    ui$.selectedThread.set('t-tab')

    selectKanbanBoard('b1')

    expect(kanban$.activeBoardId.get()).toBe('b1')
    expect(ui$.selectedThread.get()).toBe('t-tab')
  })
})

describe('kanbanPaneThreadId', () => {
  const tab = (id: string, kind: MessageTab['kind'], threadId: string): MessageTab => ({
    id,
    kind,
    messageId: '',
    threadId,
    subject: threadId,
    from: 'sender@example.com',
    body: '',
    viewMode: 'plain',
  })

  beforeEach(() => {
    kanban$.paneThreadId.set('t-card')
    compose$.tabs.set([
      tab('thread-t-tab', 'thread', 't-tab'),
      tab('item-1', 'reader', 't-feed'),
      tab('compose-1', 'compose', ''),
    ])
    compose$.activeTab.set('')
  })

  afterEach(() => {
    kanban$.paneThreadId.set('')
    compose$.tabs.set([])
    compose$.activeTab.set('')
  })

  it('is the card conversation while no tab is active', () => {
    expect(kanbanPaneThreadId()).toBe('t-card')
  })

  it('is the active thread or reader tab rather than the card behind it', () => {
    compose$.activeTab.set('thread-t-tab')
    expect(kanbanPaneThreadId()).toBe('t-tab')
    compose$.activeTab.set('item-1')
    expect(kanbanPaneThreadId()).toBe('t-feed')
  })

  it('finds a tab opened while the pane was closed', () => {
    kanban$.paneThreadId.set('')
    compose$.activeTab.set('thread-t-tab')
    expect(kanbanPaneThreadId()).toBe('t-tab')
  })

  it('falls back to the card conversation behind a compose tab', () => {
    compose$.activeTab.set('compose-1')
    expect(kanbanPaneThreadId()).toBe('t-card')
  })
})

describe('kanban$.keyboardCard', () => {
  afterEach(() => {
    kanban$.keyboardCard.set(null)
    kanban$.paneThreadId.set('')
    kanban$.activeBoardId.set('')
  })

  it('forgets the card Esc left the keyboard on once a conversation opens', () => {
    kanban$.keyboardCard.set({ threadId: 't1', columnKey: 'b1:acc1:INBOX' })
    kanban$.paneThreadId.set('t2')
    expect(kanban$.keyboardCard.get()).toBeNull()
  })

  it('forgets it when another board is shown', () => {
    kanban$.activeBoardId.set('b1')
    kanban$.keyboardCard.set({ threadId: 't1', columnKey: 'b1:acc1:INBOX' })
    kanban$.activeBoardId.set('b2')
    expect(kanban$.keyboardCard.get()).toBeNull()
  })
})

describe('selectAdjacentKanbanThread', () => {
  const inbox = { accountId: 'acc1', folderId: 'INBOX' }
  const starred = { accountId: 'unified', folderId: 'starred' }
  const card = (n: number) => message({ id: `m${n}`, thread_id: `t${n}`, subject: `S${n}`, unread: false })
  // Items of one feed share a thread_id.
  const item = (n: number) =>
    message({ id: `rss-1:item${n}`, account_id: 'rss-1', folder_id: 'feed', thread_id: 'feed-1', unread: false })
  const threadTab = (threadId: string): MessageTab => ({
    id: `thread-${threadId}`,
    kind: 'thread',
    messageId: '',
    threadId,
    subject: threadId,
    from: 'sender@example.com',
    body: '',
    viewMode: 'plain',
  })

  beforeEach(() => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [inbox, starred] }])
    kanban$.activeBoardId.set('b1')
    kanban$.threads.set({
      [kanbanColumnKey(inbox)]: [card(1), card(2), card(3)],
      [kanbanColumnKey(starred)]: [item(1), item(2), item(3)],
    })
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
    kanban$.keyboardCard.set(null)
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedThread.set('')
  })

  afterEach(() => {
    kanban$.keyboardCard.set(null)
    kanban$.activeBoardId.set('')
    kanban$.threads.set({})
    compose$.tabs.set([])
    compose$.activeTab.set('')
  })

  it('steps from the thread tab on screen, not the card conversation behind it', () => {
    kanban$.paneThreadId.set('t1')
    compose$.tabs.set([threadTab('t2')])
    compose$.activeTab.set('thread-t2')
    ui$.selectedThread.set('t2')

    selectAdjacentKanbanThread(1)

    expect(kanban$.paneThreadId.get()).toBe('t3')
    expect(compose$.activeTab.get()).toBe('')
  })

  it('steps from a tab opened over a closed pane', () => {
    compose$.tabs.set([threadTab('t2')])
    compose$.activeTab.set('thread-t2')
    ui$.selectedThread.set('t2')

    selectAdjacentKanbanThread(-1)

    expect(kanban$.paneThreadId.get()).toBe('t1')
  })

  it('moves the selection from the given card without opening it while no conversation is open', () => {
    selectAdjacentKanbanThread(1, { threadId: 't2', columnKey: kanbanBoardColumnKey('b1', inbox) })

    expect(kanban$.keyboardCard.get()).toEqual({ threadId: 't3', columnKey: kanbanBoardColumnKey('b1', inbox) })
    expect(kanban$.paneThreadId.get()).toBe('')
  })

  it('moves on from the selected card on the next step', () => {
    kanban$.keyboardCard.set({ threadId: 't1', columnKey: kanbanBoardColumnKey('b1', inbox) })

    selectAdjacentKanbanThread(1)
    selectAdjacentKanbanThread(1)

    expect(kanban$.keyboardCard.get()?.threadId).toBe('t3')
    expect(kanban$.paneThreadId.get()).toBe('')
  })

  it('selects the first card, without opening it, when nothing is selected or open', () => {
    selectAdjacentKanbanThread(1)

    expect(kanban$.keyboardCard.get()).toEqual({ threadId: 't1', columnKey: kanbanBoardColumnKey('b1', inbox) })
    expect(kanban$.paneThreadId.get()).toBe('')
  })

  it('skips a minimized column when moving the selection', () => {
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [starred, inbox] }])
    settings$.kanbanMinimizedColumns.set({ [kanbanBoardColumnKey('b1', starred)]: true })
    try {
      selectAdjacentKanbanThread(1)

      expect(kanban$.keyboardCard.get()).toEqual({ threadId: 't1', columnKey: kanbanBoardColumnKey('b1', inbox) })
    } finally {
      settings$.kanbanMinimizedColumns.set({})
    }
  })

  it('steps from the open conversation, not the given card', () => {
    kanban$.paneThreadId.set('t1')
    ui$.selectedThread.set('t1')

    selectAdjacentKanbanThread(1, { threadId: 't2', columnKey: kanbanBoardColumnKey('b1', inbox) })

    expect(kanban$.paneThreadId.get()).toBe('t2')
  })

  it('walks feed items by their own id and keeps them out of the card conversation', () => {
    kanban$.paneColumnKey.set(kanbanBoardColumnKey('b1', starred))
    compose$.tabs.set([{ ...threadTab('feed-1'), id: 'rss-1:item2', kind: 'reader', messageId: 'rss-1:item2' }])
    compose$.activeTab.set('rss-1:item2')

    selectAdjacentKanbanThread(1)

    expect(compose$.activeTab.get()).toBe('rss-1:item3')
    expect(kanban$.paneThreadId.get()).toBe('')
  })

  it('moves the selection through feed items by their own id while no conversation is open', () => {
    const columnKey = kanbanBoardColumnKey('b1', starred)
    kanban$.keyboardCard.set({ threadId: 'feed-1', columnKey, itemId: 'rss-1:item1' })

    selectAdjacentKanbanThread(1)
    expect(kanban$.keyboardCard.get()).toEqual({ threadId: 'feed-1', columnKey, itemId: 'rss-1:item2' })
    selectAdjacentKanbanThread(1, { threadId: 'feed-1', columnKey, itemId: 'rss-1:item2' })

    expect(kanban$.keyboardCard.get()?.itemId).toBe('rss-1:item3')
    expect(compose$.activeTab.get()).toBe('')
  })
})
