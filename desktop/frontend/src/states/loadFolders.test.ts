import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Folder } from '../types'
// Initialize the account/boot dependency graph before importing mail directly.
import './accounts'
import { isDraftFolder, isInboxFolder, loadFolders, mail$ } from './mail'

describe('loadFolders', () => {
  let previousGo: unknown
  let previousFolders: Folder[]
  let previousFoldersByAccount: Record<string, Folder[]>

  beforeEach(() => {
    previousGo = (window as any).go
    previousFolders = mail$.folders.peek()
    previousFoldersByAccount = mail$.foldersByAccount.peek()
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
  })

  afterEach(() => {
    mail$.folders.set(previousFolders)
    mail$.foldersByAccount.set(previousFoldersByAccount)
    if (previousGo === undefined) delete (window as any).go
    else (window as any).go = previousGo
  })

  it.each([{}, { folders: null }])('keeps folder readers usable after an empty response: %j', async (response) => {
    ;(window as any).go = {
      main: { App: { Invoke: async () => response } },
    }

    await loadFolders('acc')

    expect(mail$.folders.get()).toEqual([])
    expect(mail$.foldersByAccount.acc.get()).toEqual([])
    expect(isInboxFolder('INBOX', 'acc')).toBe(true)
    expect(isDraftFolder('Drafts', 'acc')).toBe(true)
  })

  it('keeps server folder roles available to reply and draft readers', async () => {
    const folders: Folder[] = [
      { id: 'received', account_id: 'acc', name: 'Received', role: 'inbox', unread: 2 },
      { id: 'unfinished', account_id: 'acc', name: 'Unfinished', role: 'drafts', unread: 0 },
    ]
    ;(window as any).go = {
      main: { App: { Invoke: async () => ({ folders }) } },
    }

    await loadFolders('acc')

    expect(mail$.folders.get()).toEqual(folders)
    expect(mail$.foldersByAccount.acc.get()).toEqual(folders)
    expect(isInboxFolder('received', 'acc')).toBe(true)
    expect(isDraftFolder('unfinished', 'acc')).toBe(true)
  })
})
