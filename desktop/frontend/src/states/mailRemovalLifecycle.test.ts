import { openThreadTab } from './compose'
import { compose$ } from './composeState'
import { afterEach, expect, it } from 'bun:test'
import { accounts$ } from './accounts'
import { mail$, loadThreads, releaseRemovedThread } from './mail'
import { archiveThread, bulkArchiveSelected, deleteThread, moveThreadToFolder } from './mailMoves'
import { kanban$, kanbanBoardColumnKey, kanbanColumnKey } from './kanban'
import { settings$ } from './settings'
import { ui$, type BulkSelectionItem } from './ui'
import { loadKanbanColumn } from '../lib/kanbanData'
import type { Account, Message } from '../types'

const id = 'review#INBOX#t.dG9waWM'
const row = (threadId = id, date = 30): Message => ({
  id: threadId + '#7',
  thread_id: threadId,
  account_id: 'review',
  folder_id: 'INBOX',
  date,
  subject: 'Topic',
  from_name: '',
  from_addr: '',
  to: '',
  preview: '',
  body: '',
  unread: false,
  starred: false,
  has_attachments: false,
})
let previousGo: unknown
let previousAccounts: Account[]
function setup(invoke: (command: string, payload: any) => Promise<any>) {
  previousGo = (window as any).go
  previousAccounts = accounts$.get()
  releaseRemovedThread(id)
  compose$.tabs.set([])
  compose$.activeTab.set('')
  ui$.selectedAccount.set('review')
  ui$.selectedFolder.set('INBOX')
  ui$.selectedThread.set('')
  ui$.query.set('')
  ui$.filterMode.set('all')
  ui$.attachmentsOnly.set(false)
  mail$.threads.set([row()])
  mail$.messages.set([])
  mail$.readThreads.set({})
  kanban$.activeBoardId.set('')
  ;(window as any).go = { main: { App: { Invoke: invoke } } }
}
afterEach(() => {
  compose$.tabs.set([])
  compose$.activeTab.set('')
  kanban$.keyboardCard.set(null)
  accounts$.set(previousAccounts)
  releaseRemovedThread(id)
  kanban$.activeBoardId.set('')
  kanban$.threads.set({})
  mail$.readThreads.set({})
  ;(window as any).go = previousGo
})

it('clears the last removed rows and selection on a successful empty refresh', async () => {
  setup(async () => ({ threads: [], next_cursor: '' }))
  ui$.selectedThread.set(id)
  mail$.threadLoading.set(false)
  mail$.threadsCursor.set('old-cursor')
  await loadThreads(false)
  expect(mail$.threads.get()).toEqual([])
  expect(ui$.selectedThread.get()).toBe('')
  expect(mail$.threadsCursor.get()).toBe('')
})

it('keeps live search results when a background refresh finds nothing in the cache', async () => {
  setup(async () => ({ threads: [], next_cursor: '' }))
  ui$.query.set('body only')
  ui$.selectedThread.set(id)
  mail$.threadLoading.set(false)
  mail$.threadsCursor.set('old-cursor')
  await loadThreads(false)
  ui$.query.set('')
  expect(mail$.threads.get()).toEqual([row()])
  expect(ui$.selectedThread.get()).toBe(id)
  expect(mail$.threadsCursor.get()).toBe('old-cursor')
})

it('keeps server-only search results when the cache returns some of the hits', async () => {
  const cached = row('review#INBOX#t.Y2FjaGVk', 20)
  setup(async () => ({ threads: [cached], next_cursor: '' }))
  ui$.query.set('body only')
  mail$.threads.set([row(), cached])
  mail$.threadsCursor.set('old-cursor')
  await loadThreads(false)
  ui$.query.set('')
  expect(mail$.threads.get()).toEqual([row(), cached])
  expect(mail$.threadsCursor.get()).toBe('old-cursor')
})

it('preserves rows, selection and pagination when a background read fails', async () => {
  setup(async () => {
    throw new Error('Read failed')
  })
  ui$.selectedThread.set(id)
  mail$.threadsCursor.set('old-cursor')
  await loadThreads(false)
  expect(mail$.threads.get()).toEqual([row()])
  expect(ui$.selectedThread.get()).toBe(id)
  expect(mail$.threadsCursor.get()).toBe('old-cursor')
})

it('preserves the unified list, selection and pagination when every account fails', async () => {
  setup(async () => ({ threads: [], failures: [{ account_id: 'review', message: 'Read failed' }] }))
  accounts$.set([{ id: 'review' } as Account])
  ui$.selectedAccount.set('unified')
  ui$.selectedFolder.set('inbox')
  ui$.selectedThread.set(id)
  mail$.threadsCursor.set('old-cursor')
  mail$.threadAccountCursors.set({ review: 'account-cursor' })
  await loadThreads(false)
  expect(mail$.threads.get()).toEqual([row()])
  expect(ui$.selectedThread.get()).toBe(id)
  expect(mail$.threadsCursor.get()).toBe('old-cursor')
  expect(mail$.threadAccountCursors.get()).toEqual({ review: 'account-cursor' })
})

it('keeps failed-account rows while removing missing rows from successful accounts', async () => {
  const missing = { ...row('healthy#INBOX#missing', 20), account_id: 'healthy' }
  const fresh = { ...row('healthy#INBOX#fresh', 40), account_id: 'healthy' }
  setup(async () => ({ threads: [fresh], failures: [{ account_id: 'review', message: 'Read failed' }] }))
  accounts$.set([{ id: 'review' } as Account, { id: 'healthy' } as Account])
  ui$.selectedAccount.set('unified')
  ui$.selectedFolder.set('inbox')
  ui$.selectedThread.set(id)
  mail$.threads.set([row(), missing])
  mail$.threadsCursor.set('old-cursor')
  await loadThreads(false)
  expect(mail$.threads.get()).toEqual([fresh, row()])
  expect(ui$.selectedThread.get()).toBe(id)
  expect(mail$.threadsCursor.get()).toBe('old-cursor')
})

it('an empty partial unified response removes only successful-account rows', async () => {
  const missing = { ...row('healthy#INBOX#missing', 20), account_id: 'healthy' }
  setup(async () => ({ threads: [], failures: [{ account_id: 'review', message: 'Read failed' }] }))
  accounts$.set([{ id: 'review' } as Account, { id: 'healthy' } as Account])
  ui$.selectedAccount.set('unified')
  ui$.selectedFolder.set('inbox')
  mail$.threads.set([row(), missing])
  await loadThreads(false)
  expect(mail$.threads.get()).toEqual([row()])
})

it('adopts newly available pagination after the previous list was exhausted', async () => {
  const fetched = Array.from({ length: 50 }, (_, i) => row(`review#INBOX#new-${i}`, 100 - i))
  setup(async () => ({ threads: fetched, next_cursor: 'more-mail' }))
  mail$.threadsCursor.set('')
  await loadThreads(false)
  expect(mail$.threadsCursor.get()).toBe('more-mail')
  expect(mail$.threads.get()).toEqual([...fetched, row()])
})

it('preserves the pagination cursor past previously loaded pages', async () => {
  const fetched = Array.from({ length: 50 }, (_, i) => row(`review#INBOX#new-${i}`, 100 - i))
  setup(async () => ({ threads: fetched, next_cursor: 'first-page' }))
  mail$.threadsCursor.set('loaded-depth')
  mail$.threadAccountCursors.set({ review: 'loaded-account-depth' })
  await loadThreads(false)
  expect(mail$.threadsCursor.get()).toBe('loaded-depth')
  expect(mail$.threadAccountCursors.get()).toEqual({ review: 'loaded-account-depth' })
})

it('keeps previously read rows pinned in Unread and Starred until archived', async () => {
  for (const filter of ['unread', 'starred'] as const) {
    const read = row()
    const selected = { ...row('review#INBOX#t.next', 20), unread: true, starred: true }
    setup(async (command) => (command === 'mail.threadList' ? { threads: [selected] } : { moved: 1 }))
    ui$.filterMode.set(filter)
    ui$.selectedThread.set(selected.thread_id)
    mail$.threads.set([read, selected])
    mail$.readThreads[id].set(true)
    await loadThreads(false)
    expect(mail$.threads.get()).toEqual([read, selected])
    await archiveThread(id)
    expect(mail$.threads.get()).toEqual([selected])
    ;(window as any).go = previousGo
  }
})

it('shows new replies after a successful archive, move, or delete', async () => {
  for (const action of [archiveThread, (threadId: string) => moveThreadToFolder(threadId, 'Work'), deleteThread]) {
    let fetched: Message[] = []
    setup(async (command) => (command === 'mail.threadList' ? { threads: fetched } : { moved: 1, deleted: 1 }))
    await action(id)
    expect(mail$.threads.get()).toEqual([])
    fetched = [row(id, 40)]
    await loadThreads(false)
    expect(mail$.threads.get()).toEqual(fetched)
    ;(window as any).go = previousGo
  }
})

it('shows new replies after a successful bulk archive', async () => {
  let fetched: Message[] = []
  setup(async (command) => (command === 'mail.threadList' ? { threads: fetched } : { moved: 1 }))
  const item: BulkSelectionItem = {
    key: id,
    groupKey: 'review',
    threadId: id,
    accountId: 'review',
    folderId: 'INBOX',
    surface: 'thread-list',
    kind: 'mail',
    unread: false,
    starred: false,
    draft: false,
    trash: false,
  }
  await bulkArchiveSelected([item])
  fetched = [row(id, 40)]
  await loadThreads(false)
  expect(mail$.threads.get()).toEqual(fetched)
})

it('shows a moved-back card after an explicit kanban reload', async () => {
  let fetched: Message[] = []
  setup(async (command) =>
    command === 'mail.archive' ? { moved: 1 } : command === 'mail.threadList' ? { threads: fetched } : {},
  )
  await archiveThread(id)
  kanban$.activeBoardId.set('review-board')
  fetched = [row(id, 40)]
  await loadKanbanColumn({ accountId: 'review', folderId: 'INBOX' }, true)
  expect(kanban$.threads['review\nINBOX'].get()).toEqual(fetched)
})

it('keeps a removal hidden during a manual load while archive is pending', async () => {
  let finish!: (value: unknown) => void
  let fetched = [row()]
  setup(async (command) =>
    command === 'mail.archive'
      ? new Promise((resolve) => {
          finish = resolve
        })
      : command === 'mail.threadList'
        ? { threads: fetched }
        : {},
  )
  const archive = archiveThread(id)
  await loadThreads(true)
  const shownWhilePending = mail$.threads.get()
  fetched = []
  finish({ moved: 1 })
  await archive
  expect(shownWhilePending).toEqual([])
})

it('guards a kanban read started before an archive even when it lands afterwards', async () => {
  let finish!: (value: unknown) => void
  setup(async (command) =>
    command === 'mail.archive'
      ? { moved: 1 }
      : command === 'mail.threadList'
        ? new Promise((resolve) => {
            finish = resolve
          })
        : {},
  )
  kanban$.activeBoardId.set('review-board')
  const column = { accountId: 'review', folderId: 'INBOX' }
  const staleRead = loadKanbanColumn(column, false)
  await archiveThread(id)
  finish({ threads: [row()] })
  await staleRead
  expect(kanban$.threads['review\nINBOX'].get()).toEqual([])
  await loadKanbanColumnAfterMoveBack(column)
  expect(kanban$.threads['review\nINBOX'].get()).toEqual([row(id, 40)])
})

async function loadKanbanColumnAfterMoveBack(column: { accountId: string; folderId: string }) {
  ;(window as any).go.main.App.Invoke = async () => ({ threads: [row(id, 40)] })
  await loadKanbanColumn(column, false)
}

it('restores a failed archive to its kanban date position without restoring later archives', async () => {
  let reject!: (reason: Error) => void
  setup(async (command, payload) => {
    if (command === 'mail.archive' && payload.thread_id === id)
      return new Promise((_resolve, fail) => {
        reject = fail
      })
    return { moved: 1 }
  })
  kanban$.activeBoardId.set('review-board')
  const middleId = 'review#INBOX#t.other'
  kanban$.threads['review\nINBOX'].set([row(), row(middleId, 20), row('review#INBOX#t.last', 10)])
  const failed = archiveThread(id)
  await archiveThread(middleId)
  reject(new Error('Rejected'))
  await failed
  expect(kanban$.threads['review\nINBOX'].get().map((x) => x.date)).toEqual([30, 10])
})

it('moves the keyboard selection off a card as it is archived, and back if that fails', async () => {
  let reject!: (reason: Error) => void
  const archived: string[] = []
  setup(async (command, payload) => {
    if (command !== 'mail.archive') return {}
    archived.push(payload.thread_id)
    return new Promise((_resolve, fail) => {
      reject = fail
    })
  })
  const column = { accountId: 'review', folderId: 'INBOX' }
  const previousBoards = settings$.kanbanBoards.get()
  settings$.kanbanBoards.set([{ id: 'review-board', name: 'Board', columns: [column] }] as any)
  try {
    kanban$.activeBoardId.set('review-board')
    const nextId = 'review#INBOX#t.next'
    kanban$.threads[kanbanColumnKey(column)].set([row(), row(nextId, 20)])
    const columnKey = kanbanBoardColumnKey('review-board', column)
    kanban$.keyboardCard.set({ threadId: id, columnKey })

    const archive = archiveThread(id)
    expect(kanban$.keyboardCard.get()).toEqual({ threadId: nextId, columnKey })

    reject(new Error('Rejected'))
    await archive
    expect(kanban$.keyboardCard.get()).toEqual({ threadId: id, columnKey })
  } finally {
    settings$.kanbanBoards.set(previousBoards)
  }
})

it('moves the keyboard selection past cards the column filters out', async () => {
  setup(async () => ({ moved: 1 }))
  const column = { accountId: 'review', folderId: 'INBOX' }
  const previousBoards = settings$.kanbanBoards.get()
  settings$.kanbanBoards.set([{ id: 'review-board', name: 'Board', columns: [column] }] as any)
  try {
    kanban$.activeBoardId.set('review-board')
    const sourceKey = kanbanColumnKey(column)
    kanban$.filters[sourceKey].set('unread')
    const hiddenId = 'review#INBOX#t.read'
    const shownId = 'review#INBOX#t.unread'
    kanban$.threads[sourceKey].set([
      { ...row(), unread: true },
      row(hiddenId, 20),
      { ...row(shownId, 10), unread: true },
    ])
    const columnKey = kanbanBoardColumnKey('review-board', column)
    kanban$.keyboardCard.set({ threadId: id, columnKey })

    await archiveThread(id)

    expect(kanban$.keyboardCard.get()).toEqual({ threadId: shownId, columnKey })
  } finally {
    kanban$.filters.set({})
    settings$.kanbanBoards.set(previousBoards)
  }
})

it.each([
  ['archive', archiveThread],
  ['delete', deleteThread],
] as const)('deactivates an active thread tab when %s advances selection', async (_label, action) => {
  const neighbour = row('review#INBOX#t.next', 20)
  setup(async (command) => (command === 'mail.threadList' ? { threads: [neighbour] } : { moved: 1, deleted: 1 }))
  mail$.threads.set([row(), neighbour])
  ui$.selectedThread.set(neighbour.thread_id)
  openThreadTab(row())
  expect(compose$.activeTab.peek()).toBe(`thread-${id}`)
  expect(ui$.selectedThread.peek()).toBe(id)

  const pending = action(id)
  // Removal is optimistic: actions must already target the displayed neighbour
  // while the archive/delete request is still in flight.
  expect(compose$.activeTab.peek()).toBe('')
  expect(ui$.selectedThread.peek()).toBe(neighbour.thread_id)
  expect(compose$.conversationThread.peek()).toBe(neighbour.thread_id)
  await pending
  expect(compose$.activeTab.peek()).toBe('')
  expect(ui$.selectedThread.peek()).toBe(neighbour.thread_id)
})
