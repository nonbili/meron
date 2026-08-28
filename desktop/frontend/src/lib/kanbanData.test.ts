import { beforeEach, describe, expect, it } from 'bun:test'
import { accounts$ } from '../states/accounts'
import { t } from './i18n'
import { kanban$, setGlobalKanbanFilter } from '../states/kanban'
import { mail$ } from '../states/mail'
import { settings$ } from '../states/settings'
import type { Account, Folder } from '../types'
import {
  KANBAN_DROP_REASONS,
  KANBAN_MOVE_MESSAGES,
  accountLabel,
  activeKanbanColumnFilter,
  columnDropTargetClass,
  columnEmptyText,
  columnSearchActive,
  folderLabel,
  folderMatches,
  isUnifiedStarredColumn,
  isRSSAccount,
  kanbanColumnMatchesMailEvent,
  kanbanColumnUnreadCount,
  loadKanbanColumn,
  loadMoreKanbanColumn,
  mergeLabelFolders,
  nextFoldersSnapshot,
  resolveKanbanMove,
  searchColumnLabel,
  searchScopeColumn,
  searchTargets,
  subscribeKanbanMailReloads,
  syncKanbanColumn,
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
  mail$.foldersByAccount.set({})
  ;(window as any).go = undefined
  ;(window as any).runtime = undefined
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
  it('keeps a manual column sync pending until its completion event', async () => {
    const calls: string[] = []
    const handlers = new Map<string, (detail: unknown) => void>()
    ;(window as any).runtime = {
      EventsOn: (name: string, handler: (detail: unknown) => void) => {
        handlers.set(name, handler)
        return () => handlers.delete(name)
      },
    }
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string) => {
            calls.push(command)
            return command === 'mail.threadList' ? { threads: [], next_cursor: '' } : { online: true, queued: true }
          },
        },
      },
    }

    const sync = syncKanbanColumn({ accountId: 'acc1', folderId: 'INBOX' })
    let finished = false
    void sync.then(() => {
      finished = true
    })
    await Promise.resolve()

    expect(finished).toBe(false)
    expect(calls).toEqual(['mail.sync'])

    handlers.get('mail.synced')?.({ account: 'acc1', folder: 'INBOX' })
    await sync

    expect(calls).toEqual(['mail.sync', 'mail.threadList'])
    expect(finished).toBe(true)
    expect(handlers.size).toBe(0)
  })

  it('waits for every account in a unified column before finishing sync', async () => {
    const handlers = new Map<string, (detail: unknown) => void>()
    ;(window as any).runtime = {
      EventsOn: (name: string, handler: (detail: unknown) => void) => {
        handlers.set(name, handler)
        return () => handlers.delete(name)
      },
    }
    accounts$.set([account('acc1'), account('acc2')])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string) =>
            command === 'mail.threadList'
              ? { threads: [], next_cursor: '', folder_unreads: {} }
              : { online: true, queued: true },
        },
      },
    }

    const sync = syncKanbanColumn({ accountId: 'unified', folderId: 'inbox' })
    let finished = false
    void sync.then(() => {
      finished = true
    })
    await Promise.resolve()

    handlers.get('mail.newMessages')?.({ account: 'acc1', folder: 'inbox' })
    await Promise.resolve()
    expect(finished).toBe(false)

    handlers.get('mail.synced')?.({ account: 'acc2', folder: 'INBOX' })
    await sync

    expect(finished).toBe(true)
    expect(handlers.size).toBe(0)
  })

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
    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(3)
  })

  it('waits for a persisted never-synced folder before treating it as empty', async () => {
    const handlers = new Map<string, (detail: unknown) => void>()
    let reads = 0
    accounts$.set([account('acc1')])
    ;(window as any).runtime = {
      EventsOn: (name: string, handler: (detail: unknown) => void) => {
        handlers.set(name, handler)
        return () => handlers.delete(name)
      },
    }
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async () => {
            reads += 1
            return reads === 1
              ? { threads: [], folder_synced: false }
              : { threads: [message({ thread_id: 'synced' })], folder_synced: true }
          },
        },
      },
    }

    const load = loadKanbanColumn({ accountId: 'acc1', folderId: 'Archive' }, true)
    for (let attempt = 0; attempt < 10 && !handlers.has('mail.synced'); attempt += 1) {
      await Promise.resolve()
    }

    expect(kanban$.loading['acc1\nArchive'].get()).toBe(true)
    expect(reads).toBe(1)
    expect(handlers.has('mail.synced')).toBe(true)

    handlers.get('mail.synced')?.({ account: 'acc1', folder: 'Archive' })
    await load

    expect(reads).toBe(2)
    expect(kanban$.threads['acc1\nArchive'].get()[0]?.thread_id).toBe('synced')
    expect(kanban$.loading['acc1\nArchive'].get()).toBe(false)
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

  it('sends one core-owned request for a unified column', async () => {
    const calls: { command: string; payload: unknown }[] = []
    accounts$.set([account('acc1'), { ...account('acc2'), included_in_unified: false }, account('acc3')])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return { threads: [], next_cursor: '', folder_unread: 4, folder_unreads: { acc1: 2, acc3: 2 } }
          },
        },
      },
    }
    kanban$.globalFilter.set('unread')

    await loadKanbanColumn({ accountId: 'unified', folderId: 'inbox' }, true)

    const threadListCalls = calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)
    expect(threadListCalls).toHaveLength(1)
    expect(threadListCalls[0]).toMatchObject({ account_id: 'unified', folder_id: 'inbox', filter: 'unread' })
    expect(kanban$.unreadCounts['unified\ninbox'].get()).toBe(4)
    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(2)
    expect(mail$.foldersByAccount.acc3.get()?.[0]?.unread).toBe(2)
  })

  it('asks for a unified column by role and keeps its unreads out of the inbox badge', async () => {
    const calls: { command: string; payload: unknown }[] = []
    accounts$.set([account('acc1')])
    mail$.foldersByAccount.acc1.set([{ account_id: 'acc1', id: 'INBOX', name: 'Inbox', role: 'inbox', unread: 7 }])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return { threads: [], next_cursor: '', folder_unreads: { acc1: 1 } }
          },
        },
      },
    }
    kanban$.globalFilter.set('all')

    await loadKanbanColumn({ accountId: 'unified', folderId: 'trash' }, true)

    const threadListCalls = calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)
    expect(threadListCalls).toHaveLength(1)
    // Both fields: folder_id keeps the request readable, folder_role is what the
    // core resolves per account.
    expect(threadListCalls[0]).toMatchObject({
      account_id: 'unified',
      folder_id: 'trash',
      folder_role: 'trash',
    })
    // Trash's unread count must not land on the side-nav's inbox badge.
    expect(mail$.foldersByAccount.acc1.get()?.[0]?.unread).toBe(7)
  })

  it('syncs each account own folder behind a unified non-inbox column', async () => {
    const calls: { command: string; payload: any }[] = []
    accounts$.set([account('acc1'), account('acc2')])
    // acc1 has a Sent folder; acc2's server does not, so it sits the sync out.
    mail$.foldersByAccount.acc1.set([
      { account_id: 'acc1', id: 'INBOX', name: 'Inbox', role: 'inbox', unread: 0 },
      { account_id: 'acc1', id: '[Gmail]/Sent Mail', name: 'Sent', role: 'sent', unread: 0 },
    ])
    mail$.foldersByAccount.acc2.set([{ account_id: 'acc2', id: 'INBOX', name: 'Inbox', role: 'inbox', unread: 0 }])
    const handlers = new Map<string, (detail: unknown) => void>()
    ;(window as any).runtime = {
      EventsOn: (name: string, handler: (detail: unknown) => void) => {
        handlers.set(name, handler)
        return () => handlers.delete(name)
      },
    }
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return command === 'mail.threadList' ? { threads: [], next_cursor: '' } : { online: true, queued: true }
          },
        },
      },
    }

    const sync = syncKanbanColumn({ accountId: 'unified', folderId: 'sent' })
    await Promise.resolve()
    handlers.get('mail.synced')?.({ account: 'acc1', folder: '[Gmail]/Sent Mail' })
    await sync

    const syncCalls = calls.filter((call) => call.command === 'mail.sync').map((call) => call.payload)
    expect(syncCalls).toEqual([{ account_id: 'acc1', folder: '[Gmail]/Sent Mail' }])
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

    expect(calls).toEqual([
      {
        command: 'mail.starredItems',
        payload: { query: 'quarterly', filter: 'all', limit: 50, before_cursor: undefined },
      },
    ])
    expect(kanban$.threads['unified\nstarred'].get().map((item) => item.id)).toEqual(['old', 'new'])
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
  const t = (key: string) =>
    ({
      'kanban.columns.unifiedInbox': 'Unified inbox',
      'kanban.columns.unifiedStarred': 'Unified starred',
      'kanban.columns.unifiedSent': 'Unified sent',
      'kanban.columns.unifiedDrafts': 'Unified drafts',
      'kanban.columns.unifiedArchive': 'Unified archive',
      'kanban.columns.unifiedJunk': 'Unified junk',
      'kanban.columns.unifiedTrash': 'Unified trash',
    })[key] ?? key

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
    const accs = [account('acc1')]
    expect(folderLabel({ accountId: 'unified', folderId: 'inbox' }, folders, accs, t)).toBe('Unified inbox')
    expect(isUnifiedStarredColumn({ accountId: 'unified', folderId: 'starred' })).toBe(true)
    expect(folderLabel({ accountId: 'unified', folderId: 'starred' }, folders, accs, t)).toBe('Unified starred')
    expect(folderLabel({ accountId: 'acc1', folderId: 'INBOX' }, folders, accs)).toBe('Inbox')
    expect(folderLabel({ accountId: 'acc1', folderId: 'f1' }, folders, accs)).toBe('Receipts')
    expect(folderLabel({ accountId: 'acc1', folderId: 'f2' }, folders, accs)).toBe('f2')
  })

  it('searchColumnLabel names a unified column by its role', () => {
    const accs = [account('acc1')]
    expect(searchColumnLabel({ accountId: 'unified', folderId: 'sent' }, [], accs, t)).toBe('Unified sent')
    expect(searchColumnLabel({ accountId: 'unified', folderId: 'starred' }, [], accs, t)).toBe('Unified starred')
  })

  it('matches unified columns to each account own folder in sync events', () => {
    const byAccount = {
      acc1: [{ account_id: 'acc1', id: '[Gmail]/Sent Mail', name: 'Sent', role: 'sent' } as Folder],
    }
    expect(
      kanbanColumnMatchesMailEvent({ accountId: 'unified', folderId: 'sent' }, 'acc1', '[Gmail]/Sent Mail', byAccount),
    ).toBe(true)
    expect(kanbanColumnMatchesMailEvent({ accountId: 'unified', folderId: 'sent' }, 'acc1', 'INBOX', byAccount)).toBe(
      false,
    )
    expect(
      kanbanColumnMatchesMailEvent({ accountId: 'unified', folderId: 'inbox' }, 'acc1', undefined, byAccount),
    ).toBe(true)
    expect(
      kanbanColumnMatchesMailEvent({ accountId: 'unified', folderId: 'starred' }, 'acc1', 'INBOX', byAccount),
    ).toBe(true)
  })

  it('mergeLabelFolders flattens per-account folders after the base list', () => {
    const base = [{ account_id: 'acc1', id: 'f1', name: 'A' } as Folder]
    const byAccount = { acc2: [{ account_id: 'acc2', id: 'f2', name: 'B' } as Folder] }
    expect(mergeLabelFolders(base, byAccount).map((f) => f.id)).toEqual(['f1', 'f2'])
  })
})

describe('nextFoldersSnapshot', () => {
  const folder = (id: string): Folder => ({ account_id: 'acc1', id, name: id }) as Folder

  it('keeps the previous object so memoized derivations stay stable', () => {
    const current = { acc1: [folder('f1')] }
    const first = nextFoldersSnapshot({}, current)
    expect(nextFoldersSnapshot(first, current)).toBe(first)
  })

  // The bug this exists for: Legend-State replaces one account's array in place on
  // the *same* record object, so a picker memoized on that object never saw a
  // folder created in the app until the next restart.
  it('mints a new object when an account list is replaced in place', () => {
    const live: Record<string, Folder[]> = { acc1: [folder('f1')] }
    const first = nextFoldersSnapshot({}, live)
    live.acc1 = [folder('f1'), folder('created')]
    const second = nextFoldersSnapshot(first, live)
    expect(second).not.toBe(first)
    expect(second.acc1.map((item) => item.id)).toEqual(['f1', 'created'])
    expect(first.acc1.map((item) => item.id)).toEqual(['f1'])
  })

  it('mints a new object when an account appears or disappears', () => {
    const live: Record<string, Folder[]> = { acc1: [folder('f1')] }
    const first = nextFoldersSnapshot({}, live)
    live.acc2 = [folder('f2')]
    const second = nextFoldersSnapshot(first, live)
    expect(second).not.toBe(first)
    delete live.acc1
    expect(nextFoldersSnapshot(second, live)).not.toBe(second)
  })
})

describe('subscribeKanbanMailReloads', () => {
  const setup = () => {
    const handlers = new Map<string, (detail: { account?: string; folder?: string }) => void>()
    const calls: { command: string; payload: any }[] = []
    settings$.kanbanBoards.set([
      { id: 'board-a', name: 'A', columns: [{ accountId: 'acc1', folderId: 'INBOX' }] },
      { id: 'board-b', name: 'B', columns: [{ accountId: 'acc2', folderId: 'Archive' }] },
    ])
    kanban$.activeBoardId.set('board-a')
    kanban$.searchQuery.set('')
    kanban$.searchScope.set('all')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            calls.push({ command, payload })
            return { threads: [], next_cursor: '', folder_unread: 0 }
          },
        },
      },
    }
    const unsubscribe = subscribeKanbanMailReloads((name, callback) => {
      handlers.set(name, callback)
      return () => handlers.delete(name)
    })
    const cleanup = () => {
      unsubscribe()
      kanban$.activeBoardId.set('')
    }
    return { calls, cleanup, handlers }
  }

  it('keeps a queued new-mail reload alive when the folder cache changes', async () => {
    const { calls, cleanup, handlers } = setup()
    try {
      handlers.get('mail.newMessages')?.({ account: 'acc1', folder: 'inbox' })
      mail$.foldersByAccount.acc1.set([{ account_id: 'acc1', id: 'INBOX', name: 'Inbox', role: 'inbox', unread: 1 }])
      await Bun.sleep(275)

      expect(calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)).toEqual([
        {
          account_id: 'acc1',
          folder_id: 'INBOX',
          query: '',
          filter: 'all',
          refresh: false,
          limit: 50,
        },
      ])
    } finally {
      cleanup()
    }
  })

  it("reloads the account's columns for a sent copy, which names no folder", async () => {
    const { calls, cleanup, handlers } = setup()
    try {
      handlers.get('mail.sentCopyCached')?.({ account: 'acc1' })
      await Bun.sleep(275)

      expect(calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)).toEqual([
        {
          account_id: 'acc1',
          folder_id: 'INBOX',
          query: '',
          filter: 'all',
          refresh: false,
          limit: 50,
        },
      ])
    } finally {
      cleanup()
    }
  })

  it('leaves other accounts alone when a sent copy is cached', async () => {
    const { calls, cleanup, handlers } = setup()
    try {
      handlers.get('mail.sentCopyCached')?.({ account: 'acc2' })
      await Bun.sleep(275)

      expect(calls.filter((call) => call.command === 'mail.threadList')).toEqual([])
    } finally {
      cleanup()
    }
  })

  it('resolves the active board and folder roles when each event arrives', async () => {
    const { calls, cleanup, handlers } = setup()
    try {
      kanban$.activeBoardId.set('board-b')
      settings$.kanbanBoards.set([
        { id: 'board-a', name: 'A', columns: [{ accountId: 'acc1', folderId: 'INBOX' }] },
        { id: 'board-b', name: 'B', columns: [{ accountId: 'unified', folderId: 'archive' }] },
      ])
      mail$.foldersByAccount.acc2.set([
        { account_id: 'acc2', id: 'All Mail', name: 'Archive', role: 'archive', unread: 0 },
      ])

      handlers.get('mail.synced')?.({ account: 'acc2', folder: 'All Mail' })
      await Bun.sleep(275)

      expect(calls.filter((call) => call.command === 'mail.threadList').map((call) => call.payload)).toEqual([
        {
          account_id: 'unified',
          folder_id: 'archive',
          folder_role: 'archive',
          query: '',
          filter: 'all',
          refresh: false,
          limit: 50,
        },
      ])
    } finally {
      cleanup()
    }
  })
})

describe('resolveKanbanMove', () => {
  const mailAccounts = [
    { id: 'acc1', provider: 'imap', auth_type: 'password' },
    { id: 'acc2', provider: 'imap', auth_type: 'password' },
    { id: 'feed1', provider: 'rss', auth_type: 'rss' },
    { id: 'feed2', provider: 'rss', auth_type: 'rss' },
  ]
  const unified = { accountId: 'unified', folderId: 'inbox' }

  it('takes the origin from the card when dragging out of a unified column', () => {
    const thread = message({ account_id: 'acc2', folder_id: 'INBOX' })
    expect(resolveKanbanMove(unified, { accountId: 'acc1', folderId: 'Archive' }, thread, mailAccounts)).toEqual({
      kind: 'move',
      origin: { accountId: 'acc2', folderId: 'INBOX' },
    })
  })

  it('uses the column itself as the origin for a single-account column', () => {
    const source = { accountId: 'acc1', folderId: 'INBOX' }
    expect(resolveKanbanMove(source, { accountId: 'acc1', folderId: 'Archive' }, message({}), mailAccounts)).toEqual({
      kind: 'move',
      origin: source,
    })
  })

  it('resolves the origin even when the card is missing from a single-account column', () => {
    const source = { accountId: 'acc1', folderId: 'INBOX' }
    expect(resolveKanbanMove(source, { accountId: 'acc2', folderId: 'INBOX' }, undefined, mailAccounts)).toEqual({
      kind: 'move',
      origin: source,
    })
  })

  it('does nothing when the card lands back on its own mailbox', () => {
    const thread = message({ account_id: 'acc1', folder_id: 'INBOX' })
    // Case-insensitively: a unified card reports "INBOX", the column stores "inbox".
    expect(resolveKanbanMove(unified, { accountId: 'acc1', folderId: 'inbox' }, thread, mailAccounts).kind).toBe('noop')
    const source = { accountId: 'acc1', folderId: 'INBOX' }
    expect(resolveKanbanMove(source, { accountId: 'acc1', folderId: 'INBOX' }, thread, mailAccounts).kind).toBe('noop')
  })

  it('refuses a unified column as the drop target', () => {
    expect(resolveKanbanMove({ accountId: 'acc1', folderId: 'INBOX' }, unified, message({}), mailAccounts)).toEqual({
      kind: 'blocked',
      reasonKey: KANBAN_DROP_REASONS.unifiedTarget,
    })
  })

  it('refuses a unified drag whose card it cannot find', () => {
    expect(resolveKanbanMove(unified, { accountId: 'acc1', folderId: 'Archive' }, undefined, mailAccounts)).toEqual({
      kind: 'blocked',
      reasonKey: KANBAN_DROP_REASONS.unknownOrigin,
    })
  })

  it('refuses feed/mail mixes in either direction, resolving the feed through a unified column', () => {
    const feedItem = message({ account_id: 'feed1', folder_id: 'inbox' })
    expect(resolveKanbanMove(unified, { accountId: 'acc1', folderId: 'Archive' }, feedItem, mailAccounts)).toEqual({
      kind: 'blocked',
      reasonKey: KANBAN_DROP_REASONS.feedToMail,
    })
    expect(
      resolveKanbanMove(
        { accountId: 'acc1', folderId: 'INBOX' },
        { accountId: 'feed1', folderId: 'inbox' },
        message({}),
        mailAccounts,
      ),
    ).toEqual({ kind: 'blocked', reasonKey: KANBAN_DROP_REASONS.mailToFeed })
  })

  it('allows a feed to move between RSS accounts', () => {
    const feedItem = message({ account_id: 'feed1', folder_id: 'inbox' })
    expect(resolveKanbanMove(unified, { accountId: 'feed2', folderId: 'inbox' }, feedItem, mailAccounts)).toEqual({
      kind: 'move',
      origin: { accountId: 'feed1', folderId: 'inbox' },
    })
  })
})

describe('columnDropTargetClass', () => {
  it('marks a refused column, hovered or not', () => {
    expect(columnDropTargetClass(true, true)).toContain('red')
    expect(columnDropTargetClass(false, true)).toContain('opacity-60')
    expect(columnDropTargetClass(true)).toContain('accent')
    expect(columnDropTargetClass(false)).toBe('')
  })
})

describe('kanban move strings', () => {
  it('name locale keys that every catalog carries', () => {
    const keys = [...Object.values(KANBAN_DROP_REASONS), ...Object.values(KANBAN_MOVE_MESSAGES)]
    for (const key of keys) {
      expect(key.startsWith('kanban.')).toBe(true)
      // t() falls back to the key itself when nothing is catalogued. The value
      // covers the one message that interpolates an error.
      expect(t(key, { error: 'x' })).not.toBe(key)
    }
  })
})
