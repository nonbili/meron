import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { waitFor } from '@testing-library/react'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { kanban$, markColumnAllRead } from './kanban'
import { mail$ } from './mail'
import {
  markAccountInboxRead,
  markAllRead,
  markMessageReadState,
  markThreadUnread,
  markUnifiedInboxRead,
} from './mailFlags'
import { refreshAccountFoldersCache } from './mailFolders'
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

  it('restores unread rows and reconciles the badge when the backend rejects the action', async () => {
    const inbox = mail$.foldersByAccount['acc-1'][0].get()
    const write = Promise.withResolvers<never>()
    ;(window as any).go.main.App.Invoke = async (command: string) => {
      if (command === 'mail.folderList') return { folders: [{ ...inbox, unread: 53 }] }
      return write.promise
    }

    const pending = markAccountInboxRead('acc-1')
    await refreshAccountFoldersCache('acc-1', false)
    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(0)
    write.reject(new Error('offline'))
    await pending

    expect(mail$.threads[0].unread.get()).toBe(true)
    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(53)
    expect(mail$.folders[0].unread.get()).toBe(60)
    expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(true)
    expect(kanban$.unreadCounts['acc-1\nINBOX'].get()).toBe(53)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(60)
  })

  it('keeps a newer view and a successful sibling write when one account fails', async () => {
    const failingWrite = Promise.withResolvers<never>()
    const healthyWrite = Promise.withResolvers<{ folder_unreads: Record<string, Record<string, number>> }>()
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'mail.markAllRead') {
        return payload.account_id === 'acc-1' ? failingWrite.promise : healthyWrite.promise
      }
      return {
        folders: [
          {
            id: 'INBOX',
            account_id: payload.account_id,
            name: 'Inbox',
            role: 'inbox',
            unread: payload.account_id === 'acc-1' ? 50 : 0,
          },
        ],
      }
    }

    const unified = markUnifiedInboxRead()
    const individual = markAccountInboxRead('acc-2')
    expect(mail$.threads.get().map((thread) => thread.unread)).toEqual([false, true, false])
    expect(mail$.messages.get().map((row) => row.unread)).toEqual([false, true, false])
    expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(false)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(0)
    healthyWrite.resolve({ folder_unreads: { 'acc-2': { INBOX: 0 } } })
    await individual
    expect(mail$.threads[2].unread.get()).toBe(false)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(0)

    const archive = message({ id: 'archive', account_id: 'acc-2', folder_id: 'Archive' })
    const archiveFolders = [{ id: 'Archive', account_id: 'acc-2', name: 'Archive', role: '', unread: 1 }]
    ui$.selectedAccount.set('acc-2')
    mail$.threads.set([archive])
    mail$.messages.set([archive])
    mail$.folders.set(archiveFolders)
    failingWrite.reject(new Error('offline'))
    await unified

    expect(
      calls.filter(({ command }) => command === 'mail.markAllRead').map(({ payload }) => payload.account_id),
    ).toEqual(['acc-1', 'acc-2'])
    expect(mail$.threads.get()).toEqual([archive])
    expect(mail$.messages.get()).toEqual([archive])
    expect(mail$.folders.get()).toEqual(archiveFolders)
    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(50)
    expect(mail$.foldersByAccount['acc-2'][0].unread.get()).toBe(0)
    expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(true)
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(50)
  })

  it('keeps the confirmed count when an older folder refresh returns late', async () => {
    const olderRefresh = Promise.withResolvers<unknown>()
    let refreshes = 0
    const inbox = mail$.foldersByAccount['acc-1'][0].get()
    ;(window as any).go.main.App.Invoke = async (command: string) => {
      if (command === 'mail.folderList') {
        if (++refreshes === 1) return olderRefresh.promise
        return { folders: [{ ...inbox, unread: 0 }] }
      }
      return { folder_unreads: { 'acc-1': { INBOX: 0 } } }
    }

    const pendingRefresh = refreshAccountFoldersCache('acc-1', false)
    await markAccountInboxRead('acc-1')
    olderRefresh.resolve({ folders: [inbox] })
    await pendingRefresh

    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(0)
    expect(mail$.folders[0].unread.get()).toBe(7)
  })

  it.each(['thread', 'message'])(
    'preserves a later unread action on a %s and sends it after the Inbox write',
    async (target) => {
      const inbox = mail$.foldersByAccount['acc-1'][0].get()
      const inboxWrite = Promise.withResolvers<unknown>()
      const unreadWrite = Promise.withResolvers<unknown>()
      const counts = (unread: number) => ({ folder_unreads: { 'acc-1': { INBOX: unread } } })
      let unread = 0
      ;(window as any).go.main.App.Invoke = async (command: string, payload: { account_id: string }) => {
        if (command === 'mail.markAllRead') return inboxWrite.promise
        if (command === 'mail.markRead') {
          calls.push({ command, payload })
          return unreadWrite.promise
        }
        const folders = payload.account_id === 'acc-1' ? [{ ...inbox, unread }] : mail$.foldersByAccount['acc-2'].get()
        return { folders }
      }

      const inboxPending = markAccountInboxRead('acc-1')
      const unreadPending =
        target === 'thread'
          ? markThreadUnread(message().thread_id)
          : markMessageReadState(mail$.messages[0].get(), false)
      expect(calls).toHaveLength(0)

      inboxWrite.resolve(counts(0))
      await inboxPending
      expect(mail$.threads[0].unread.get()).toBe(true)
      expect(mail$.messages[0].unread.get()).toBe(true)
      expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(true)
      expect(calls).toHaveLength(1)

      unread = 1
      unreadWrite.resolve(counts(1))
      await unreadPending
      await waitFor(() => {
        expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(8)
        expect(mail$.folders[0].unread.get()).toBe(8)
      })
    },
  )

  it('keeps Inbox rollback when a queued message action also fails', async () => {
    const unread = message()
    const read = message({ id: 'read-message', thread_id: 'read-thread', unread: false })
    const inbox = mail$.foldersByAccount['acc-1'][0].get()
    mail$.threads.set([unread, read])
    mail$.messages.set([unread, read])
    kanban$.threads['unified\ninbox'].set([unread, read])
    const inboxWrite = Promise.withResolvers<never>()
    ;(window as any).go.main.App.Invoke = async (command: string, payload: { account_id: string }) => {
      if (command === 'mail.markAllRead') return inboxWrite.promise
      if (command === 'mail.markRead') throw new Error('flag write failed')
      return { folders: payload.account_id === 'acc-1' ? [inbox] : mail$.foldersByAccount['acc-2'].get() }
    }

    const inboxPending = markAccountInboxRead('acc-1')
    const flagPending = markMessageReadState(read, false)
    inboxWrite.reject(new Error('Inbox write failed'))
    await expect(flagPending).rejects.toThrow('flag write failed')
    await inboxPending

    expect(mail$.threads.get().map((row) => row.unread)).toEqual([true, false])
    expect(mail$.messages.get().map((row) => row.unread)).toEqual([true, false])
    expect(kanban$.threads['unified\ninbox'].get().map((row) => row.unread)).toEqual([true, false])
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(57)
  })

  it('runs a second Inbox write after an intervening unread action', async () => {
    const inbox = mail$.foldersByAccount['acc-1'][0].get()
    const firstWrite = Promise.withResolvers<unknown>()
    const unreadWrite = Promise.withResolvers<unknown>()
    const replies = [firstWrite.promise, Promise.resolve({ folder_unreads: { 'acc-1': { INBOX: 0 } } })]
    let bulkWrites = 0
    ;(window as any).go.main.App.Invoke = async (command: string, payload: { account_id: string }) => {
      if (command === 'mail.markAllRead') return replies[bulkWrites++]
      if (command === 'mail.markRead') return unreadWrite.promise
      const unread = bulkWrites === 1 ? 1 : 0
      return {
        folders: payload.account_id === 'acc-1' ? [{ ...inbox, unread }] : mail$.foldersByAccount['acc-2'].get(),
      }
    }

    const first = markAccountInboxRead('acc-1')
    const flag = markThreadUnread(message().thread_id)
    firstWrite.resolve({ folder_unreads: { 'acc-1': { INBOX: 0 } } })
    await first

    const second = markAccountInboxRead('acc-1')
    expect(bulkWrites).toBe(1)
    unreadWrite.resolve({ folder_unreads: { 'acc-1': { INBOX: 1 } } })
    await flag
    await second

    expect(bulkWrites).toBe(2)
    expect(mail$.threads[0].unread.get()).toBe(false)
    expect(mail$.foldersByAccount['acc-1'][0].unread.get()).toBe(0)
  })

  it.each(['thread list', 'column'])('orders a %s mark-all-read after a queued unread action', async (surface) => {
    ui$.selectedAccount.set('acc-1')
    ui$.selectedFolder.set('INBOX')
    const inbox = mail$.foldersByAccount['acc-1'][0].get()
    const firstWrite = Promise.withResolvers<unknown>()
    const actions: string[] = []
    let unread = 0
    ;(window as any).go.main.App.Invoke = async (command: string) => {
      if (command === 'mail.markAllRead') {
        actions.push(command)
        if (actions.length === 1) return firstWrite.promise
        unread = 0
      }
      if (command === 'mail.markRead') {
        actions.push(command)
        unread = 1
      }
      if (command === 'mail.folderList') return { folders: [{ ...inbox, unread }] }
      return { folder_unreads: { 'acc-1': { INBOX: unread } } }
    }

    const first = markAccountInboxRead('acc-1')
    const flag = markThreadUnread(message().thread_id)
    const second =
      surface === 'thread list' ? markAllRead() : markColumnAllRead({ accountId: 'unified', folderId: 'inbox' })
    expect(actions).toEqual(['mail.markAllRead'])
    firstWrite.resolve({ folder_unreads: { 'acc-1': { INBOX: 0 } } })
    await Promise.all([first, flag, second])

    expect(actions).toEqual(['mail.markAllRead', 'mail.markRead', 'mail.markAllRead'])
    if (surface === 'thread list') expect(mail$.threads[0].unread.get()).toBe(false)
    else expect(kanban$.threads['unified\ninbox'][0].unread.get()).toBe(false)
  })

  it('refreshes the Unified Inbox when a queued list action finishes after navigation', async () => {
    ui$.selectedAccount.set('acc-1')
    ui$.selectedFolder.set('INBOX')
    const inbox = mail$.foldersByAccount['acc-1'][0].get()
    const firstWrite = Promise.withResolvers<unknown>()
    let bulkWrites = 0
    ;(window as any).go.main.App.Invoke = async (
      command: string,
      payload: { account_id: string; refresh?: boolean },
    ) => {
      if (command === 'mail.markAllRead' && ++bulkWrites === 1) return firstWrite.promise
      if (command === 'mail.folderList') return { folders: [{ ...inbox, unread: 0 }] }
      if (command === 'mail.threadList') {
        expect(payload).toMatchObject({ account_id: 'unified', refresh: false })
        return { threads: [message({ unread: false })] }
      }
      return { folder_unreads: { 'acc-1': { INBOX: 0 } } }
    }

    const first = markAccountInboxRead('acc-1')
    const queued = markAllRead()
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('inbox')
    mail$.threads.set([message()])
    firstWrite.resolve({ folder_unreads: { 'acc-1': { INBOX: 0 } } })
    await Promise.all([first, queued])

    expect(bulkWrites).toBe(2)
    expect(mail$.threads[0].unread.get()).toBe(false)
  })

  it('keeps a queued unified list action ordered for accounts that were idle', async () => {
    ui$.selectedFolder.set('inbox')
    const row = { ...mail$.threads[2].get(), unread: false }
    mail$.threads[2].set(row)
    const firstWrite = Promise.withResolvers<unknown>()
    const actions: string[] = []
    ;(window as any).go.main.App.Invoke = async (
      command: string,
      payload: { account_id: string; thread_id: string },
    ) => {
      if (command === 'mail.markAllRead') {
        actions.push(payload.account_id)
        if (payload.account_id === 'acc-1') return firstWrite.promise
      }
      if (command === 'mail.markRead') actions.push(payload.thread_id)
      if (command === 'mail.folderList') return { folders: mail$.foldersByAccount[payload.account_id].get() }
      return {}
    }

    const first = markAccountInboxRead('acc-1')
    const unified = markAllRead()
    const flag = markThreadUnread(row.thread_id)
    expect(actions).toEqual(['acc-1'])
    firstWrite.resolve({})
    await Promise.all([first, unified, flag])

    expect(actions).toEqual(['acc-1', 'unified', row.thread_id])
    expect(mail$.threads[2].unread.get()).toBe(true)
  })

  it('orders a later message action after a queued mark-all in another folder', async () => {
    ui$.selectedAccount.set('acc-1')
    ui$.selectedFolder.set('Archive')
    const row = mail$.threads[1].get()
    mail$.threads.set([row])
    mail$.messages.set([row])
    const firstWrite = Promise.withResolvers<unknown>()
    const actions: string[] = []
    ;(window as any).go.main.App.Invoke = async (
      command: string,
      payload: { account_id: string; folder_id: string; folder: string },
    ) => {
      if (command === 'mail.markAllRead') {
        actions.push(payload.folder_id)
        if (payload.folder_id === 'INBOX') return firstWrite.promise
      }
      if (command === 'mail.markRead') actions.push(payload.folder)
      if (command === 'mail.folderList') return { folders: mail$.foldersByAccount[payload.account_id].get() }
      return {}
    }

    const first = markAccountInboxRead('acc-1')
    const bulk = markAllRead()
    const flag = markMessageReadState(row, false)
    expect(actions).toEqual(['INBOX'])
    firstWrite.resolve({})
    await Promise.all([first, bulk, flag])

    expect(actions).toEqual(['INBOX', 'Archive', 'Archive'])
    expect(mail$.messages[0].unread.get()).toBe(true)
  })
})
