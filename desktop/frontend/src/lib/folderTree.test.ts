import { describe, expect, it } from 'bun:test'
import type { Folder } from '../types'
import { buildFolderTree, selectableFolderIds, type TreeNode } from './folderTree'

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
