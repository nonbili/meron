import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$, LOCAL_ACCOUNT_ID } from './accounts'
import { kanban$ } from './kanban'
import { mail$, releaseRemovedThread } from './mail'
import { markUnreadWithUndo } from './mailFlags'
import {
  archiveThread,
  bulkDeleteSelected,
  bulkMoveSelectedToFolder,
  copyThreadToFolder,
  deleteMessage,
  deleteThread,
  moveThreadToFolder,
} from './mailMoves'
import { settleConfirm, ui$ } from './ui'
import { settings$ } from './settings'
import { recentThreadTransfers$ } from './recentThreadTransfers'

const THREAD = 'acc#INBOX#t.dG9waWM'

const thread = (overrides: Partial<Message> = {}): Message => ({
  id: `${THREAD}#7`,
  account_id: 'acc',
  folder_id: 'INBOX',
  thread_id: THREAD,
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Subject',
  preview: '',
  body: '',
  date: 1,
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

// Undo moves back exactly the copies the action reported (`target_uids`), so
// the undo's mail.move must carry them as message_ids — resolving the thread
// in the target instead would also take older mail of the conversation there,
// and finds nothing for a uid: key.
describe('undo after moving a thread', () => {
  let previousGo: unknown
  let calls: Array<{ command: string; payload: any }> = []
  let responses: Record<string, unknown> = {}

  beforeEach(() => {
    previousGo = (window as any).go
    calls = []
    responses = {}
    recentThreadTransfers$.set([])
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('INBOX')
    ui$.selectedThread.set('')
    ui$.toast.set('')
    ui$.toastUndo.set(null)
    mail$.threads.set([thread()])
    mail$.messages.set([])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            calls.push({ command, payload })
            if (command in responses) {
              const response = responses[command]
              if (response instanceof Error) throw response
              return response
            }
            return { ok: true }
          },
        },
      },
    }
  })

  afterEach(() => {
    recentThreadTransfers$.set([])
    for (const id of [THREAD, 'a', 'b', 'missing']) releaseRemovedThread(id)
    ;(window as any).go = previousGo
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
    kanban$.threads.set({})
  })

  async function runUndo() {
    const undo = ui$.toastUndo.get() as (() => void) | null
    expect(typeof undo).toBe('function')
    calls = []
    undo?.()
    await new Promise((resolve) => setTimeout(resolve, 0))
    return calls.find((call) => call.command === 'mail.move')?.payload
  }

  it('remembers transfers for threads outside the loaded list using their encoded account', async () => {
    mail$.threads.set([])
    responses['mail.move'] = { moved: 1 }
    await moveThreadToFolder(THREAD, 'Work')
    expect(recentThreadTransfers$.get()[0]).toEqual({
      action: 'move',
      sourceAccountId: 'acc',
      targetAccountId: 'acc',
      targetFolderId: 'Work',
    })
    responses['mail.copy'] = { copied: 1 }
    await copyThreadToFolder(THREAD, 'other', 'Receipts')
    expect(recentThreadTransfers$.get()[0]).toEqual({
      action: 'copy',
      sourceAccountId: 'acc',
      targetAccountId: 'other',
      targetFolderId: 'Receipts',
    })
  })

  it('records successful transfers but not failed transfers or undo destinations', async () => {
    responses['mail.move'] = { moved: 1, target_uids: [5] }
    await moveThreadToFolder(THREAD, 'Work')
    expect(recentThreadTransfers$.get()).toEqual([
      { action: 'move', sourceAccountId: 'acc', targetAccountId: 'acc', targetFolderId: 'Work' },
    ])
    await runUndo()
    expect(recentThreadTransfers$.get()).toHaveLength(1)

    mail$.threads.set([thread()])
    responses['mail.copy'] = { copied: 1 }
    await copyThreadToFolder(THREAD, 'other', 'Receipts')
    expect(recentThreadTransfers$.get()[0]).toEqual({
      action: 'copy',
      sourceAccountId: 'acc',
      targetAccountId: 'other',
      targetFolderId: 'Receipts',
    })
    responses['mail.copy'] = new Error('Rejected')
    await copyThreadToFolder(THREAD, 'other', 'Failed')
    responses['mail.move'] = { moved: 0 }
    await moveThreadToFolder(THREAD, 'Failed')
    expect(recentThreadTransfers$.get()).toHaveLength(2)
  })

  it('moves to another account even when the destination folder has the same name', async () => {
    responses['mail.move'] = { moved: 1 }
    expect(await moveThreadToFolder(THREAD, 'INBOX', { targetAccountId: 'other' })).toBe(true)
    expect(calls.find((call) => call.command === 'mail.move')?.payload).toEqual({
      thread_id: THREAD,
      target_folder_id: 'INBOX',
      target_account_id: 'other',
    })
    expect(recentThreadTransfers$.get()[0]).toEqual({
      action: 'move',
      sourceAccountId: 'acc',
      targetAccountId: 'other',
      targetFolderId: 'INBOX',
    })
    expect(ui$.toastUndo.get()).toBe(null)
  })

  it('restores the source after a cross-account removal failure and records no successful move', async () => {
    responses['mail.move'] = new Error('Move copied messages but could not confirm removal from the source folder')
    expect(await moveThreadToFolder(THREAD, 'INBOX', { targetAccountId: 'other' })).toBe(false)
    expect(mail$.threads.get().some((item) => item.thread_id === THREAD)).toBe(true)
    expect(recentThreadTransfers$.get()).toEqual([])
    expect(ui$.toast.get()).toContain('copied')
  })

  it('bulk moves same-named folders across accounts and skips copies already at the destination', async () => {
    responses['mail.move'] = { moved: 1 }
    await bulkMoveSelectedToFolder(
      [
        {
          kind: 'mail',
          key: THREAD,
          groupKey: '',
          surface: 'thread-list',
          unread: false,
          starred: false,
          draft: false,
          trash: false,
          threadId: THREAD,
          accountId: 'acc',
          folderId: 'INBOX',
        },
        {
          kind: 'mail',
          key: 'other',
          groupKey: '',
          surface: 'thread-list',
          unread: false,
          starred: false,
          draft: false,
          trash: false,
          threadId: 'other#INBOX#t.dG9waWM',
          accountId: 'other',
          folderId: 'INBOX',
        },
      ],
      'INBOX',
      'other',
    )
    expect(calls.filter((call) => call.command === 'mail.move').map((call) => call.payload)).toEqual([
      { thread_id: THREAD, target_folder_id: 'INBOX', target_account_id: 'other' },
    ])
  })

  it('moves the reported Trash copies back out of Trash', async () => {
    responses['mail.delete'] = { deleted: 1, trash: 'Trash', thread_id: 'acc#Trash#t.dG9waWM', target_uids: [42, 43] }
    await deleteThread(THREAD)

    expect(await runUndo()).toEqual({
      thread_id: 'acc#Trash#t.dG9waWM',
      target_folder_id: 'INBOX',
      message_ids: ['42', '43'],
    })
  })

  // Without the exact copies there is no safe way back: the thread key in
  // Trash would also take older mail of the conversation.
  it('offers no undo when the copies are unknown', async () => {
    responses['mail.delete'] = { deleted: 1, trash: 'Trash', thread_id: 'acc#Trash#t.dG9waWM', target_uids: [] }
    await deleteThread(THREAD)
    expect(ui$.toastUndo.get()).toBe(null)
    expect(ui$.toast.get()).not.toBe('')

    mail$.threads.set([thread()])
    responses['mail.archive'] = { moved: 1, folder: 'Archive', thread_id: 'acc#Archive#t.dG9waWM' }
    await archiveThread(THREAD)
    expect(ui$.toastUndo.get()).toBe(null)

    mail$.threads.set([thread()])
    responses['mail.move'] = { moved: 1, target_uids: [] }
    await moveThreadToFolder(THREAD, 'Work')
    expect(ui$.toastUndo.get()).toBe(null)
  })

  it('moves the reported archive copies back', async () => {
    responses['mail.archive'] = { moved: 1, folder: 'Archive', thread_id: 'acc#Archive#t.dG9waWM', target_uids: [9] }
    await archiveThread(THREAD)

    expect(await runUndo()).toEqual({
      thread_id: 'acc#Archive#t.dG9waWM',
      target_folder_id: 'INBOX',
      message_ids: ['9'],
    })
  })

  it('moves the reported copies back from a folder', async () => {
    responses['mail.move'] = { moved: 1, target_uids: [5] }
    await moveThreadToFolder(THREAD, 'Work')

    expect(await runUndo()).toEqual({
      thread_id: 'acc#Work#t.dG9waWM',
      target_folder_id: 'INBOX',
      message_ids: ['5'],
    })
  })

  it('shows the server reason when Wails rejects a copy with a string', async () => {
    ;(window as any).go.main.App.Invoke = async () => {
      throw 'sidecar messages.copy error: [LIMIT] Mail size is larger than the maximum size allowed by server configuration'
    }
    await copyThreadToFolder(THREAD, 'other', 'INBOX')
    expect(ui$.toast.get()).toContain('destination server rejected a message')
    expect(ui$.toast.get()).not.toContain('sidecar')
  })

  it('keeps completed bulk moves removed and reports partial success', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'b' })])
    ;(window as any).go.main.App.Invoke = (command: string, payload: { thread_id?: string }) => {
      if (command === 'mail.move') {
        return payload.thread_id === 'a' ? Promise.resolve({ moved: 1 }) : Promise.reject('[NOPERM] permission denied')
      }
      return Promise.resolve({ threads: mail$.threads.get() })
    }
    await bulkMoveSelectedToFolder(
      ['a', 'b'].map((threadId) => ({
        key: threadId,
        groupKey: '',
        threadId,
        accountId: 'acc',
        folderId: 'INBOX',
        surface: 'thread-list',
        kind: 'mail',
        unread: false,
        starred: false,
        draft: false,
        trash: false,
      })),
      'Work',
    )
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['b'])
    expect(ui$.toast.get()).toContain('denied access')
    expect(ui$.toast.get()).toContain('Completed 1 of 2 selected threads')
  })

  // A timed-out delete may still have gone through; the toast says so
  // instead of passing on the bridge's wording.
  it('explains a timed-out delete and puts the thread back', async () => {
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    await deleteThread(THREAD)
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([THREAD])
    expect(ui$.toast.get()).toContain('Delete failed: The operation timed out')
    expect(ui$.toast.get()).not.toContain('sidecar')
  })

  // Whether a delete moves to Trash or expunges in place is the server's call.
  it('asks to check the folder and Trash after a delete with an unknown outcome', async () => {
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    await deleteThread(THREAD)
    expect(ui$.toast.get()).toBe(
      "Delete failed: The operation timed out before completion could be confirmed. Check acc / Inbox and that account's Trash before retrying.",
    )
  })

  it('names the mailbox from the thread id when the thread is not loaded', async () => {
    mail$.threads.set([])
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    await deleteThread(THREAD)
    expect(ui$.toast.get()).toContain("Check acc / Inbox and that account's Trash")
  })

  it('tells apart accounts that share a display name', async () => {
    const accounts = [
      { id: 'acc', email: 'alex@example.com', display_name: 'Alex' },
      { id: 'acc2', email: 'alex@example.org', display_name: 'Alex' },
    ]
    accounts$.set(accounts as any)
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    await deleteThread(THREAD)
    expect(ui$.toast.get()).toContain("Check alex@example.com / Inbox and that account's Trash")
    accounts$.set([])
  })

  it('names the full folder path and keeps Trash for a folder only named Trash', async () => {
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    mail$.foldersByAccount.set({
      acc: [
        { id: 'Personal/Receipts', account_id: 'acc', name: 'Personal/Receipts', role: '', unread: 0 },
        { id: 'Trash', account_id: 'acc', name: 'Trash', role: '', unread: 0 },
      ],
    })
    const receipts = 'acc#Personal/Receipts#t.dG9waWM'
    mail$.threads.set([thread({ id: `${receipts}#7`, thread_id: receipts, folder_id: 'Personal/Receipts' })])
    await deleteThread(receipts)
    expect(ui$.toast.get()).toContain("Check acc / Personal/Receipts and that account's Trash")

    const named = 'acc#Trash#t.dG9waWM'
    mail$.threads.set([thread({ id: `${named}#7`, thread_id: named, folder_id: 'Trash' })])
    const deleting = deleteThread(named)
    settleConfirm(true)
    await deleting
    expect(ui$.toast.get()).toContain("Check acc / Trash and that account's Trash")
    mail$.foldersByAccount.set({})
  })

  it('leaves Trash out of a timed-out local folder delete', async () => {
    const local = `${LOCAL_ACCOUNT_ID}#Keep#t.dG9waWM`
    mail$.threads.set([thread({ id: `${local}#1`, thread_id: local, account_id: LOCAL_ACCOUNT_ID, folder_id: 'Keep' })])
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    const deleting = deleteThread(local)
    settleConfirm(true)
    await deleting
    expect(ui$.toast.get()).toBe('Delete failed: The operation timed out before completion could be confirmed.')
  })

  it('words a local folder refusal as local storage put it', async () => {
    const local = `${LOCAL_ACCOUNT_ID}#Keep#t.dG9waWM`
    mail$.threads.set([thread({ id: `${local}#1`, thread_id: local, account_id: LOCAL_ACCOUNT_ID, folder_id: 'Keep' })])
    responses['mail.delete'] = new Error('sidecar messages.delete error: access permission denied')
    const deleting = deleteThread(local)
    settleConfirm(true)
    await deleting
    expect(ui$.toast.get()).toBe('Delete failed: access permission denied')
  })

  // Even a folder the core calls Drafts may not be the one the server expunges.
  it('keeps the Trash check for a timed-out draft discard', async () => {
    mail$.threads.set([thread({ folder_role: 'drafts' })])
    responses['mail.delete'] = new Error('sidecar messages.delete timeout after 30s')
    const discarding = deleteThread(THREAD)
    settleConfirm(true)
    await discarding
    expect(ui$.toast.get()).toContain("Check acc / Inbox and that account's Trash before retrying.")
  })

  it('says where to look when a delete copied to Trash but left the originals', async () => {
    responses['mail.delete'] = new Error(
      'sidecar messages.delete error: Move copied messages but could not confirm removal from the source folder: connection lost',
    )
    await deleteThread(THREAD)
    expect(ui$.toast.get()).toContain('retrying may create duplicates.')
    expect(ui$.toast.get()).toContain("Check acc / Inbox and that account's Trash before retrying.")
  })

  it('puts a thread back when the server rejects its delete', async () => {
    responses['mail.delete'] = new Error('sidecar messages.delete error: [NOPERM] permission denied')
    await deleteThread(THREAD)
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([THREAD])
    expect(ui$.toast.get()).toContain("denied access. Check your permissions for acc / Inbox and that account's Trash.")
    expect(ui$.toast.get()).not.toContain('destination')
  })

  const bulkItems = (ids: string[]) =>
    ids.map((threadId) => ({
      key: threadId,
      groupKey: '',
      threadId,
      accountId: 'acc',
      folderId: 'INBOX',
      surface: 'thread-list' as const,
      kind: 'mail' as const,
      unread: false,
      starred: false,
      draft: false,
      trash: false,
    }))

  it('keeps completed bulk deletes removed and only the rest selected', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'b' })])
    const items = bulkItems(['a', 'b'])
    ui$.bulkSelection.set(Object.fromEntries(items.map((item) => [item.key, item])))
    ui$.bulkAnchorKey.set('a')
    ;(window as any).go.main.App.Invoke = (command: string, payload: { thread_id?: string }) => {
      if (command === 'mail.delete') {
        return payload.thread_id === 'a'
          ? Promise.resolve({ deleted: 1 })
          : Promise.reject('[NOPERM] permission denied')
      }
      return Promise.resolve({ threads: mail$.threads.get() })
    }
    await bulkDeleteSelected(items)
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['b'])
    expect(Object.keys(ui$.bulkSelection.get())).toEqual(['b'])
    expect(ui$.bulkAnchorKey.get()).toBe('b')
    expect(ui$.toast.get()).toContain('denied access')
    expect(ui$.toast.get()).toContain('Thread deleted.')
  })

  // A bulk selection can span accounts and folders, so the toast names the
  // mailbox the failed thread came from.
  it('names the mailbox of a bulk delete with an unknown outcome', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'b', account_id: 'other' })])
    const items = bulkItems(['a', 'b']).map((item) =>
      item.threadId === 'b' ? { ...item, accountId: 'other', folderId: 'Receipts' } : item,
    )
    ;(window as any).go.main.App.Invoke = (command: string, payload: { thread_id?: string }) =>
      command === 'mail.delete' && payload.thread_id === 'b'
        ? Promise.reject('sidecar messages.delete timeout after 30s')
        : Promise.resolve({ deleted: 1 })
    await bulkDeleteSelected(items)
    expect(ui$.toast.get()).toContain("Check other / Receipts and that account's Trash before retrying.")
  })

  it('keeps the batch selected until it is done, and a new selection after', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'c' })])
    const items = bulkItems(['a'])
    ui$.bulkSelection.set({ a: items[0] })
    let refreshing = false
    let selectedDuringRefresh: string[] = []
    ;(window as any).go.main.App.Invoke = (command: string) => {
      if (command === 'mail.delete') return Promise.resolve({ deleted: 1 })
      if (!refreshing) {
        refreshing = true
        // Shortcuts still act on the batch, not on the next open thread.
        selectedDuringRefresh = Object.keys(ui$.bulkSelection.peek())
        // The user starts a new selection while the list refreshes.
        ui$.bulkSelection.set({ ...ui$.bulkSelection.peek(), c: bulkItems(['c'])[0] })
      }
      return Promise.resolve({ threads: mail$.threads.get() })
    }
    await bulkDeleteSelected(items)
    expect(selectedDuringRefresh).toEqual(['a'])
    expect(Object.keys(ui$.bulkSelection.get())).toEqual(['c'])
  })

  it('clears a leftover range anchor once a batch leaves nothing selected', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'c' })])
    const items = bulkItems(['a'])
    ui$.bulkSelection.set({ a: items[0] })
    // c was deselected last and stayed the anchor.
    ui$.bulkAnchorKey.set('c')
    ;(window as any).go.main.App.Invoke = (command: string) =>
      Promise.resolve(command === 'mail.delete' ? { deleted: 1 } : { threads: mail$.threads.get() })
    await bulkDeleteSelected(items)
    expect(ui$.bulkSelection.get()).toEqual({})
    expect(ui$.bulkAnchorKey.get()).toBe('')
  })

  it('ignores another bulk delete while one is running', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'b' })])
    const items = bulkItems(['a', 'b'])
    const deletes: string[] = []
    let second: Promise<void> | null = null
    let refusal = ''
    ;(window as any).go.main.App.Invoke = (command: string, payload: { thread_id?: string }) => {
      if (command !== 'mail.delete') return Promise.resolve({ threads: mail$.threads.get() })
      deletes.push(payload.thread_id ?? '')
      // Delete pressed again while b is still being deleted.
      if (payload.thread_id === 'b') {
        second = bulkDeleteSelected([items[1]])
        refusal = ui$.toast.peek()
      }
      return Promise.resolve({ deleted: 1 })
    }
    await bulkDeleteSelected(items)
    await second
    expect(deletes).toEqual(['a', 'b'])
    expect(refusal).toBe('Still deleting the previous selection. Try again when it finishes.')
  })

  it('keeps a deleted message deleted when the card reload after it fails', async () => {
    const message = thread()
    mail$.messages.set([message])
    settings$.kanbanBoards.set([{ id: 'board', name: 'Board', columns: [{ accountId: 'acc', folderId: 'INBOX' }] }])
    kanban$.activeBoardId.set('board')
    kanban$.threads['acc\nINBOX'].set([message])
    // The delete went through; the column reload behind the card does not.
    ;(window as any).go.main.App.Invoke = (command: string) =>
      command === 'mail.delete' || command === 'mail.folderList'
        ? Promise.resolve(command === 'mail.delete' ? { deleted: 1 } : { folders: [] })
        : Promise.reject(new Error(`sidecar ${command} timeout after 30s`))
    try {
      const deleting = deleteMessage(message)
      settleConfirm(true)
      expect(await deleting).toBe(true)
      expect(mail$.messages.get()).toEqual([])
      expect(ui$.toast.get()).not.toContain('Delete failed')
    } finally {
      settings$.kanbanBoards.set([])
    }
  })

  it('keeps a Shift-click anchor whose row was not deleted', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'b' }), thread({ thread_id: 'c' })])
    const items = bulkItems(['a', 'b'])
    ui$.bulkSelection.set(Object.fromEntries(items.map((item) => [item.key, item])))
    // The last click deselected c, which stays the anchor.
    ui$.bulkAnchorKey.set('c')
    ;(window as any).go.main.App.Invoke = (command: string, payload: { thread_id?: string }) =>
      command === 'mail.delete' && payload.thread_id === 'b'
        ? Promise.reject('[NOPERM] permission denied')
        : Promise.resolve({ deleted: 1 })
    await bulkDeleteSelected(items)
    expect(Object.keys(ui$.bulkSelection.get())).toEqual(['b'])
    expect(ui$.bulkAnchorKey.get()).toBe('c')
  })

  it('moves a Shift-click anchor off a deselected row that was then deleted', async () => {
    mail$.threads.set([thread({ thread_id: 'a' }), thread({ thread_id: 'b' })])
    const items = bulkItems(['a', 'b'])
    // a was deselected while the delete was queued, and stayed the anchor.
    ui$.bulkSelection.set({ b: items[1] })
    ui$.bulkAnchorKey.set('a')
    ;(window as any).go.main.App.Invoke = (command: string, payload: { thread_id?: string }) =>
      command === 'mail.delete' && payload.thread_id === 'b'
        ? Promise.reject('[NOPERM] permission denied')
        : Promise.resolve({ deleted: 1 })
    await bulkDeleteSelected(items)
    expect(ui$.bulkAnchorKey.get()).toBe('b')
  })

  it('keeps later archives when an earlier one fails', async () => {
    const older = thread({ id: 'c', thread_id: 'c', date: 10 })
    const middle = thread({ id: 'b', thread_id: 'b', date: 20 })
    const newest = thread({ id: 'a', thread_id: 'a', date: 30 })
    mail$.threads.set([newest, middle, older])
    ui$.selectedThread.set('a')
    let rejectFirst: (error: Error) => void = () => {}
    ;(window as any).go = {
      main: {
        App: {
          Invoke: (command: string, payload: { thread_id?: string }) => {
            if (command === 'mail.archive' && payload.thread_id === 'a') {
              return new Promise((_resolve, reject) => {
                rejectFirst = reject
              })
            }
            return new Promise(() => {})
          },
        },
      },
    }

    const first = archiveThread('a')
    void archiveThread('b')
    await Promise.resolve()
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['c'])
    expect(ui$.selectedThread.get()).toBe('c')

    rejectFirst(new Error('archive failed'))
    await first
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['a', 'c'])
    expect(ui$.selectedThread.get()).toBe('c')
  })

  it('stays on a card when the archived thread is already off the board', async () => {
    const next = thread({ id: 'b', thread_id: 'b', date: 20, account_id: 'acc', folder_id: 'INBOX' })
    kanban$.activeBoardId.set('board')
    kanban$.paneColumnKey.set('board\nacc\nINBOX')
    kanban$.paneThreadId.set('missing')
    kanban$.threads['acc\nINBOX'].set([next, thread({ id: 'c', thread_id: 'c', date: 10 })])
    ui$.selectedThread.set('missing')
    mail$.threads.set([])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: () => new Promise(() => {}),
        },
      },
    }

    void archiveThread('missing')
    await Promise.resolve()
    expect(ui$.selectedThread.get()).toBe('b')
    expect(kanban$.paneThreadId.get()).toBe('b')
  })

  it('stays on a visible row when the archived thread is already off the list', async () => {
    const next = thread({ id: 'b', thread_id: 'b', date: 20 })
    mail$.threads.set([next, thread({ id: 'c', thread_id: 'c', date: 10 })])
    ui$.selectedThread.set('missing')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: () => new Promise(() => {}),
        },
      },
    }

    void archiveThread('missing')
    await Promise.resolve()
    expect(ui$.selectedThread.get()).toBe('b')
  })

  it('offers undo for mark unread only once it has landed', async () => {
    markUnreadWithUndo(THREAD)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(typeof ui$.toastUndo.get()).toBe('function')

    ui$.toastUndo.set(null)
    mail$.threads.set([thread()])
    responses['mail.markRead'] = new Error('server refused')
    markUnreadWithUndo(THREAD)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(ui$.toastUndo.get()).toBe(null)
    expect(ui$.toast.get()).toBe('server refused')
    expect(ui$.toastTone.get()).toBe('error')
  })
})
