import { describe, expect, it } from 'bun:test'
import type { Account, Folder } from '../types'
import { boardBadgeAccountIds, boardBadgeSyncTargets, boardUnreadCount } from './boardUnread'

function folder(account_id: string, id: string, unread: number, role = ''): Folder {
  return { id, account_id, name: id, role, unread }
}

const foldersByAccount: Record<string, Folder[]> = {
  a: [folder('a', 'INBOX', 3, 'inbox'), folder('a', 'Later', 2), folder('a', 'Sent', 1, 'sent')],
  b: [folder('b', 'Inbox', 5, 'inbox'), folder('b', 'Projects', 4), folder('b', 'Archive', 7, 'archive')],
  c: [folder('c', 'INBOX', 11, 'inbox')],
}

describe('boardUnreadCount', () => {
  it('adds up the unread counts of every folder column', () => {
    const columns = [
      { accountId: 'a', folderId: 'INBOX' },
      { accountId: 'a', folderId: 'Later' },
      { accountId: 'b', folderId: 'Projects' },
    ]
    expect(boardUnreadCount(columns, foldersByAccount, ['a', 'b'])).toBe(9)
  })

  it('counts a unified column once per account in the unified view, by folder role', () => {
    const columns = [{ accountId: 'unified', folderId: 'inbox' }]
    // c is left out of the unified view, so its inbox does not count.
    expect(boardUnreadCount(columns, foldersByAccount, ['a', 'b'])).toBe(8)
    expect(boardUnreadCount([{ accountId: 'unified', folderId: 'archive' }], foldersByAccount, ['a', 'b'])).toBe(7)
  })

  it('counts a folder only once when two columns show it', () => {
    const columns = [
      { accountId: 'unified', folderId: 'inbox' },
      { accountId: 'a', folderId: 'INBOX' },
      { accountId: 'b', folderId: 'inbox' },
      { accountId: 'b', folderId: 'Projects' },
      { accountId: 'b', folderId: 'Projects' },
    ]
    expect(boardUnreadCount(columns, foldersByAccount, ['a', 'b'])).toBe(12)
  })

  it('leaves out saved search columns and the unified Starred column', () => {
    const columns = [
      { accountId: 'a', folderId: 'Later', query: 'from:boss' },
      { accountId: 'unified', folderId: 'starred' },
      { accountId: 'b', folderId: 'Projects' },
    ]
    expect(boardUnreadCount(columns, foldersByAccount, ['a', 'b'])).toBe(4)
  })

  it('counts nothing for folders or accounts it has no counts for yet', () => {
    const columns = [
      { accountId: 'a', folderId: 'Gone' },
      { accountId: 'unknown', folderId: 'INBOX' },
      { accountId: 'unified', folderId: 'junk' },
    ]
    expect(boardUnreadCount(columns, foldersByAccount, ['a', 'b', 'unknown'])).toBe(0)
    expect(boardUnreadCount([], foldersByAccount, ['a'])).toBe(0)
  })
})

describe('boardBadgeAccountIds', () => {
  it('names only the accounts that switched-on boards count', () => {
    const boards = [
      { columns: [{ accountId: 'a', folderId: 'Later' }], showUnreadBadge: true },
      { columns: [{ accountId: 'b', folderId: 'Projects', query: 'x' }], showUnreadBadge: true },
      { columns: [{ accountId: 'c', folderId: 'INBOX' }] },
    ]
    expect(boardBadgeAccountIds(boards, ['a', 'b', 'c'])).toEqual(['a'])
  })

  it('names the whole unified view for a unified folder column, but not for Starred', () => {
    const unified = { columns: [{ accountId: 'unified', folderId: 'inbox' }], showUnreadBadge: true }
    const starred = { columns: [{ accountId: 'unified', folderId: 'starred' }], showUnreadBadge: true }
    expect(boardBadgeAccountIds([unified], ['a', 'b'])).toEqual(['a', 'b'])
    expect(boardBadgeAccountIds([starred], ['a', 'b'])).toEqual([])
  })
})

describe('boardBadgeSyncTargets', () => {
  const account = (id: string, extra: { paused?: boolean; provider?: string } = {}) =>
    ({ id, provider: 'custom', auth_type: 'password', ...extra }) as Account

  it('syncs the non-inbox folders of switched-on boards, each once', () => {
    const boards = [
      {
        showUnreadBadge: true,
        columns: [
          { accountId: 'a', folderId: 'INBOX' },
          { accountId: 'a', folderId: 'Later' },
          { accountId: 'a', folderId: 'Later' },
        ],
      },
      { columns: [{ accountId: 'b', folderId: 'Projects' }] },
    ]
    expect(boardBadgeSyncTargets(boards, foldersByAccount, [account('a'), account('b')])).toEqual([
      { account_id: 'a', folder: 'Later' },
    ])
  })

  it('leaves out folders the core already watches, and paused or RSS accounts', () => {
    const folders = { a: [{ ...folder('a', 'Later', 2), notify: true }], b: [folder('b', 'Projects', 4)] }
    const boards = [
      {
        showUnreadBadge: true,
        columns: [
          { accountId: 'a', folderId: 'Later' },
          { accountId: 'b', folderId: 'Projects' },
        ],
      },
    ]
    expect(boardBadgeSyncTargets(boards, folders, [account('a'), account('b', { paused: true })])).toEqual([])
    expect(boardBadgeSyncTargets(boards, folders, [account('a'), account('b', { provider: 'rss' })])).toEqual([])
  })
})
