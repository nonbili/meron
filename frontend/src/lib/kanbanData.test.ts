import { beforeEach, describe, expect, it } from 'bun:test'
import { accounts$ } from '../states/accounts'
import { kanban$, setGlobalKanbanFilter } from '../states/kanban'
import { mail$ } from '../states/mail'
import type { Account, Folder } from '../types'
import {
  accountLabel,
  activeKanbanColumnFilter,
  columnEmptyText,
  columnSearchActive,
  folderLabel,
  folderMatches,
  isUnifiedStarredColumn,
  isRSSAccount,
  kanbanColumnUnreadCount,
  loadKanbanColumn,
  loadMoreKanbanColumn,
  mergeLabelFolders,
  searchScopeColumn,
  searchTargets,
} from './kanbanData'

const account = (id: string): Account => ({
  id,
  email: `${id}@example.com`,
  display_name: id,
  provider: 'imap',
  auth_type: 'password',
  imap_host: '',
  imap_port: 993,
  smtp_host: '',
  smtp_port: 465,
  tls: true,
})

beforeEach(() => {
  accounts$.set([])
  kanban$.threads.set({})
  kanban$.unreadCounts.set({})
  kanban$.loading.set({})
  kanban$.cursors.set({})
  kanban$.accountCursors.set({})
  kanban$.loadingMore.set({})
  kanban$.filters.set({})
  kanban$.globalFilter.set('all')
  mail$.readThreads.set({})
  ;(window as any).go = undefined
})

const message = (overrides: Partial<import('../types').Message>): import('../types').Message => ({
  id: 'm',
  account_id: 'acc1',
  folder_id: 'INBOX',
  thread_id: 't',
  from_name: '',
  from_addr: '',
  to: '',
  subject: '',
  preview: '',
  body: '',
  date: 0,
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

describe('searchScopeColumn', () => {
  it("returns null for empty and 'all' scopes", () => {
    expect(searchScopeColumn('')).toBeNull()
    expect(searchScopeColumn('all')).toBeNull()
  })

  it('splits an account/folder scope on newline', () => {
    expect(searchScopeColumn('acc1\nINBOX')).toEqual({ accountId: 'acc1', folderId: 'INBOX' })
  })

  it('returns null for a malformed scope', () => {
    expect(searchScopeColumn('acc1')).toBeNull()
  })
})

describe('searchTargets', () => {
  const columns = [
    { accountId: 'acc1', folderId: 'INBOX' },
    { accountId: 'acc2', folderId: 'INBOX' },
  ]

  it("returns all columns for scope 'all'", () => {
    expect(searchTargets(columns, 'all')).toEqual(columns)
  })

  it('returns only the scoped column', () => {
    expect(searchTargets(columns, 'acc2\nINBOX')).toEqual([columns[1]])
  })
})

describe('columnEmptyText', () => {
  it('describes empty columns per filter mode', () => {
    expect(columnEmptyText('all', false, false)).toBe('No threads')
    expect(columnEmptyText('unread', false, false)).toBe('No unread threads')
    expect(columnEmptyText('starred', false, false)).toBe('No starred threads')
  })

  it('uses feed wording for RSS columns', () => {
    expect(columnEmptyText('all', false, false, true)).toBe('No feeds')
    expect(columnEmptyText('unread', false, false, true)).toBe('No unread feeds')
    expect(columnEmptyText('starred', false, false, true)).toBe('No starred feeds')
  })

  it('distinguishes no matches from matches hidden by a filter', () => {
    expect(columnEmptyText('all', true, false)).toBe('No matches')
    expect(columnEmptyText('unread', true, false)).toBe('No unread matches')
    expect(columnEmptyText('unread', true, true)).toBe('Matches hidden by Unread filter')
    expect(columnEmptyText('starred', true, true)).toBe('Matches hidden by Starred filter')
  })
})

describe('columnSearchActive', () => {
  it('requires a non-blank query and a matching scope', () => {
    expect(columnSearchActive('k1', '', 'all')).toBe(false)
    expect(columnSearchActive('k1', '   ', 'all')).toBe(false)
    expect(columnSearchActive('k1', 'q', 'all')).toBe(true)
    expect(columnSearchActive('k1', 'q', 'k1')).toBe(true)
    expect(columnSearchActive('k1', 'q', 'k2')).toBe(false)
  })
})

describe('setGlobalKanbanFilter', () => {
  it('clears per-column overrides so the new global filter applies everywhere', () => {
    const column = { accountId: 'acc1', folderId: 'INBOX' }
    kanban$.filters['acc1\nINBOX'].set('all')
    expect(activeKanbanColumnFilter(column)).toBe('all')

    setGlobalKanbanFilter('unread')

    expect(kanban$.filters.get()).toEqual({})
    expect(activeKanbanColumnFilter(column)).toBe('unread')
  })
})

describe('kanban column loading filters', () => {
  it('sends the active global filter when loading a single-account column', async () => {
    const calls: { command: string; payload: unknown }[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return { threads: [], next_cursor: 'uid:10', folder_unread: 3 }
          },
        },
      },
    }
    kanban$.globalFilter.set('unread')

    await loadKanbanColumn({ accountId: 'acc1', folderId: 'INBOX' }, true)

    const threadListCalls = calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)
    expect(threadListCalls).toHaveLength(1)
    expect(threadListCalls[0]).toMatchObject({
      account_id: 'acc1',
      folder_id: 'INBOX',
      filter: 'unread',
      refresh: true,
      before_cursor: undefined,
    })
    expect(activeKanbanColumnFilter({ accountId: 'acc1', folderId: 'INBOX' })).toBe('unread')
    expect(kanban$.unreadCounts['acc1\nINBOX'].get()).toBe(3)
  })

  it('keeps using the active filter when loading more of a column', async () => {
    const calls: { command: string; payload: unknown }[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return { threads: [], next_cursor: '', folder_unread: command === 'mail.threadList' ? 2 : 0 }
          },
        },
      },
    }
    kanban$.globalFilter.set('unread')
    kanban$.cursors['acc1\nINBOX'].set('uid:10')
    kanban$.unreadCounts['acc1\nINBOX'].set(5)

    await loadMoreKanbanColumn({ accountId: 'acc1', folderId: 'INBOX' })

    const threadListCalls = calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)
    expect(threadListCalls).toHaveLength(1)
    expect(threadListCalls[0]).toMatchObject({
      account_id: 'acc1',
      folder_id: 'INBOX',
      filter: 'unread',
      refresh: false,
      before_cursor: 'uid:10',
    })
    expect(kanban$.unreadCounts['acc1\nINBOX'].get()).toBe(5)
  })

  it('sends the active filter for each account in a unified column', async () => {
    const calls: { command: string; payload: unknown }[] = []
    accounts$.set([account('acc1'), { ...account('acc2'), included_in_unified: false }, account('acc3')])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return { threads: [], next_cursor: '', folder_unread: 2 }
          },
        },
      },
    }
    kanban$.globalFilter.set('unread')

    await loadKanbanColumn({ accountId: 'unified', folderId: 'inbox' }, true)

    const threadListCalls = calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)
    expect(threadListCalls).toHaveLength(2)
    expect(threadListCalls).toEqual([
      expect.objectContaining({ account_id: 'acc1', folder_id: 'inbox', filter: 'unread' }),
      expect.objectContaining({ account_id: 'acc3', folder_id: 'inbox', filter: 'unread' }),
    ])
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(4)
  })

  it('keeps a just-read thread in an unread column when the reload drops it', async () => {
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async () => ({ threads: [message({ thread_id: 't2', date: 200, unread: true })], next_cursor: '' }),
        },
      },
    }
    kanban$.globalFilter.set('unread')
    // t1 was showing and is the thread we just read; the unread refetch omits it.
    kanban$.threads['acc1\nINBOX'].set([message({ thread_id: 't1', date: 100, unread: false })])
    mail$.readThreads.set({ t1: true })

    await loadKanbanColumn({ accountId: 'acc1', folderId: 'INBOX' }, false)

    expect(kanban$.threads['acc1\nINBOX'].get().map((thread) => thread.thread_id)).toEqual(['t2', 't1'])
  })

  it('does not resurrect read threads when the column filter is All', async () => {
    ;(window as any).go = {
      main: { App: { Invoke: async () => ({ threads: [], next_cursor: '' }) } },
    }
    kanban$.globalFilter.set('all')
    kanban$.threads['acc1\nINBOX'].set([message({ thread_id: 't1', date: 100, unread: false })])
    mail$.readThreads.set({ t1: true })

    await loadKanbanColumn({ accountId: 'acc1', folderId: 'INBOX' }, false)

    expect(kanban$.threads['acc1\nINBOX'].get()).toEqual([])
  })

  it('loads unified starred items through the starred-items bridge', async () => {
    const calls: { command: string; payload: unknown }[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return {
              items: [
                {
                  id: 'old',
                  account_id: 'acc1',
                  folder_id: 'inbox',
                  thread_id: 't1',
                  from_name: 'Alice',
                  from_addr: 'a@example.com',
                  to: '',
                  subject: 'Old',
                  preview: 'quarterly report',
                  body: '',
                  date: 1704067200, // 2024-01-01T00:00:00Z
                  unread: false,
                  starred: true,
                  has_attachments: false,
                },
                {
                  id: 'new',
                  account_id: 'acc2',
                  folder_id: 'inbox',
                  thread_id: 't2',
                  from_name: 'Bob',
                  from_addr: 'b@example.com',
                  to: '',
                  subject: 'New quarterly',
                  preview: '',
                  body: '',
                  date: 1704153600, // 2024-01-02T00:00:00Z
                  unread: false,
                  starred: true,
                  has_attachments: false,
                },
              ],
            }
          },
        },
      },
    }

    await loadKanbanColumn({ accountId: 'unified', folderId: 'starred' }, true, 'quarterly')

    expect(calls).toEqual([{ command: 'mail.starredItems', payload: {} }])
    expect(kanban$.threads['unified\nstarred'].get().map((item) => item.id)).toEqual(['new', 'old'])
    expect(kanban$.cursors['unified\nstarred'].get()).toBe('')
    expect(kanban$.accountCursors['unified\nstarred'].get()).toEqual({})
  })
})

describe('folderMatches', () => {
  it('matches the inbox role case-insensitively', () => {
    expect(folderMatches('inbox', 'INBOX')).toBe(true)
    expect(folderMatches('INBOX', 'inbox')).toBe(true)
  })

  it('matches other folders exactly', () => {
    expect(folderMatches('Archive', 'Archive')).toBe(true)
    expect(folderMatches('Archive', 'archive')).toBe(false)
  })

  it('is false when nothing synced', () => {
    expect(folderMatches('inbox', undefined)).toBe(false)
  })
})

describe('kanbanColumnUnreadCount', () => {
  it('uses the unread total returned with a mail column page', () => {
    expect(
      kanbanColumnUnreadCount({ accountId: 'acc1', folderId: 'inbox' }, 137, [
        message({ unread: true, unread_count: 1 }),
        message({ id: 'm2', thread_id: 't2', unread: true, unread_count: 1 }),
      ]),
    ).toBe(137)
  })

  it('trusts a zero total returned with the page', () => {
    expect(
      kanbanColumnUnreadCount({ accountId: 'acc1', folderId: 'inbox' }, 0, [
        message({ unread: true, unread_count: 1 }),
      ]),
    ).toBe(0)
  })

  it('falls back to loaded message totals before the first page returns', () => {
    expect(
      kanbanColumnUnreadCount({ accountId: 'acc1', folderId: 'inbox' }, undefined, [
        message({ unread: true, unread_count: 3 }),
      ]),
    ).toBe(3)
  })

  it('always derives unified starred from its loaded items', () => {
    expect(
      kanbanColumnUnreadCount({ accountId: 'unified', folderId: 'starred' }, 99, [
        message({ unread: true, unread_count: 3 }),
        message({ id: 'm2', thread_id: 't2', unread: false, unread_count: 0 }),
      ]),
    ).toBe(3)
  })
})

describe('isRSSAccount', () => {
  const accounts = [
    { id: 'acc1', provider: 'rss', auth_type: '' },
    { id: 'acc2', provider: 'gmail', auth_type: 'oauth' },
  ]

  it('detects rss provider and rss- prefix', () => {
    expect(isRSSAccount('acc1', accounts)).toBe(true)
    expect(isRSSAccount('rss-anything', [])).toBe(true)
  })

  it('is false for mail accounts', () => {
    expect(isRSSAccount('acc2', accounts)).toBe(false)
  })
})

describe('labels', () => {
  const accounts = [
    { id: 'acc1', email: 'a@x.com', display_name: 'Alice' },
    { id: 'acc2', email: 'b@x.com', display_name: '' },
  ]

  it('accountLabel prefers display name, then email, then id', () => {
    expect(accountLabel('unified', accounts)).toBe('Unified')
    expect(accountLabel('acc1', accounts)).toBe('Alice')
    expect(accountLabel('acc2', accounts)).toBe('b@x.com')
    expect(accountLabel('missing', accounts)).toBe('missing')
  })

  it('folderLabel names unified, inbox, and stored folders', () => {
    const folders = [{ account_id: 'acc1', id: 'f1', name: 'Receipts' } as Folder]
    const accs = [{ id: 'acc1', provider: 'gmail', auth_type: 'oauth' }] as Account[]
    expect(folderLabel({ accountId: 'unified', folderId: 'inbox' }, folders, accs)).toBe('Unified Inbox')
    expect(isUnifiedStarredColumn({ accountId: 'unified', folderId: 'starred' })).toBe(true)
    expect(folderLabel({ accountId: 'unified', folderId: 'starred' }, folders, accs)).toBe('Unified Starred')
    expect(folderLabel({ accountId: 'acc1', folderId: 'INBOX' }, folders, accs)).toBe('Inbox')
    expect(folderLabel({ accountId: 'acc1', folderId: 'f1' }, folders, accs)).toBe('Receipts')
    expect(folderLabel({ accountId: 'acc1', folderId: 'f2' }, folders, accs)).toBe('f2')
  })

  it('mergeLabelFolders flattens per-account folders after the base list', () => {
    const base = [{ account_id: 'acc1', id: 'f1', name: 'A' } as Folder]
    const byAccount = { acc2: [{ account_id: 'acc2', id: 'f2', name: 'B' } as Folder] }
    expect(mergeLabelFolders(base, byAccount).map((f) => f.id)).toEqual(['f1', 'f2'])
  })
})
