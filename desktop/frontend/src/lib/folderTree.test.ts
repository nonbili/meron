import { describe, expect, it } from 'bun:test'
import type { Folder } from '../types'
import { buildFolderTree, folderLeafName, selectableFolderIds, type TreeNode } from './folderTree'

const folder = (id: string): Folder => ({
  id,
  account_id: 'account',
  name: id,
  role: '',
  unread: 0,
  delimiter: '/',
})

describe('folder tree selection', () => {
  it('selects a real parent folder independently from its children', () => {
    const [parent] = buildFolderTree([folder('t2'), folder('t2/テスト'), folder('t2/t21')])

    expect(selectableFolderIds(parent)).toEqual(['t2'])
    expect(selectableFolderIds(parent.children[0])).toEqual(['t2/テスト'])
  })

  it('group-selects descendants when a path node is not a real folder', () => {
    const structural: TreeNode = {
      name: 't2',
      children: [
        { name: 'one', folder: folder('t2/one'), children: [] },
        { name: 'two', folder: folder('t2/two'), children: [] },
      ],
    }

    expect(selectableFolderIds(structural)).toEqual(['t2/one', 't2/two'])
  })
})

describe('folderLeafName', () => {
  const named = (name: string, delimiter?: string): Folder => ({
    id: name,
    account_id: 'account',
    name,
    role: '',
    unread: 0,
    delimiter,
  })

  it('names a nested folder by its last segment', () => {
    expect(folderLeafName(named('[Gmail]/Now', '/'))).toBe('Now')
    expect(folderLeafName(named('INBOX.Sent', '.'))).toBe('Sent')
  })

  it('keeps the full name when the server reported no delimiter', () => {
    // A NIL delimiter means a flat mailbox: punctuation is part of the name.
    expect(folderLeafName(named('Flat/Independent'))).toBe('Flat/Independent')
    expect(folderLeafName(named('example.com'))).toBe('example.com')
    expect(folderLeafName(named('v1.2', ''))).toBe('v1.2')
  })

  it('leaves a top-level folder alone', () => {
    expect(folderLeafName(named('Receipts', '/'))).toBe('Receipts')
  })

  it('skips empty segments and falls back to the full name', () => {
    expect(folderLeafName(named('Work/', '/'))).toBe('Work')
    expect(folderLeafName(named('/', '/'))).toBe('/')
  })
})
