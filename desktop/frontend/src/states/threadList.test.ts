import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { compose$ } from './composeState'
import { kanban$ } from './kanban'
import { loadMoreThreads, loadThreads, mail$, requestThreadReselect, threadListViewKey } from './mail'
import { bulkStarSelected, starThread, toggleStarWithUndo } from './mailFlags'
import { toggleBulkSelection, ui$, type BulkSelectionItem } from './ui'

const thread = (overrides: Partial<Message> = {}): Message => ({
  id: 'acc:inbox:thread:1#101',
  account_id: 'acc',
  folder_id: 'inbox',
  thread_id: 'acc:inbox:thread:1',
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Subject',
  preview: '',
  body: '',
  date: Math.floor(Date.parse('2026-06-11T12:00:00Z') / 1000),
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

const nextTick = () => new Promise((resolve) => setTimeout(resolve, 0))

const bulkItem = (message: Message, overrides: Partial<BulkSelectionItem> = {}): BulkSelectionItem => ({
  key: `test:${message.id}`,
  groupKey: 'test:column',
  threadId: message.thread_id,
  accountId: message.account_id,
  folderId: message.folder_id,
  surface: 'thread-list',
  kind: 'mail',
  unread: message.unread,
  starred: message.starred,
  draft: false,
  trash: false,
  ...overrides,
})

describe('thread list paging', () => {
  beforeEach(() => {
    kanban$.activeBoardId.set('')
    mail$.threads.set([thread({ subject: 'Old first page' })])
    mail$.threadsCursor.set('old-cursor')
    mail$.threadsLoadingMore.set(false)
    mail$.threadAccountCursors.set({})
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.query.set('old')
    ui$.filterMode.set('all')
  })

  afterEach(() => {
    ui$.query.set('')
  })

  it('drops a load-more response after a newer search replaces the list', async () => {
    let resolveOldPage!: (value: unknown) => void
    const oldPage = new Promise((resolve) => {
      resolveOldPage = resolve
    })
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { before_cursor?: string; query?: string }) => {
            if (payload.before_cursor === 'old-cursor') return oldPage
            if (payload.query === 'new') {
              return {
                threads: [thread({ thread_id: 'acc:inbox:thread:new', subject: 'New result' })],
                next_cursor: 'new-cursor',
              }
            }
            return {}
          },
        },
      },
    }

    const loadingMore = loadMoreThreads()
    ui$.query.set('new')
    await loadThreads()
    resolveOldPage({
      threads: [thread({ thread_id: 'acc:inbox:thread:old-page', subject: 'Stale result' })],
      next_cursor: 'stale-cursor',
    })
    await loadingMore

    expect(mail$.threads.get().map((item) => item.subject)).toEqual(['New result'])
    expect(mail$.threadsCursor.get()).toBe('new-cursor')
  })

  it('paints cached search results before replacing them with live results', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            if (payload.refresh) return livePage
            return {
              threads: [thread({ thread_id: 'acc:inbox:thread:cached', subject: 'Cached result' })],
              next_cursor: 'cached-cursor',
            }
          },
        },
      },
    }

    ui$.query.set('deploy')
    const loading = loadThreads()
    await nextTick()

    expect(refreshes).toEqual([false, true])
    expect(mail$.threads.get().map((item) => item.subject)).toEqual(['Cached result'])
    expect(mail$.threadsCursor.get()).toBe('cached-cursor')
    // Neither stage opens a result on its own — that would mark it read.
    expect(ui$.selectedThread.get()).toBe('')

    resolveLive({
      threads: [thread({ thread_id: 'acc:inbox:thread:live', subject: 'Live result' })],
      next_cursor: 'live-cursor',
    })
    await loading

    expect(mail$.threads.get().map((item) => item.subject)).toEqual(['Live result'])
    expect(mail$.threadsCursor.get()).toBe('live-cursor')
    expect(ui$.selectedThread.get()).toBe('')
  })
})

describe('thread selection on load', () => {
  beforeEach(() => {
    mail$.threads.set([])
    mail$.messages.set([])
    mail$.threadsCursor.set('')
    mail$.threadAccountCursors.set({})
    mail$.threadLoading.set(false)
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.query.set('')
    ui$.filterMode.set('all')
    kanban$.activeBoardId.set('')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async () => ({
            threads: [thread({ thread_id: 'acc:inbox:thread:top', unread: true })],
          }),
        },
      },
    }
  })

  afterEach(() => {
    kanban$.activeBoardId.set('')
  })

  it('leaves the conversation pane closed when a view is opened', async () => {
    await loadThreads()

    expect(mail$.threads.get()).toHaveLength(1)
    // Opening a thread marks its visible messages read, so switching to a view
    // must never do it on the user's behalf.
    expect(ui$.selectedThread.get()).toBe('')
  })

  it('closes a selection the new view does not contain instead of opening another thread', async () => {
    // The conversation is open and fully loaded, as it is after any ordinary
    // click — its messages must not keep it selected in a view it is absent from.
    ui$.selectedThread.set('acc:other:thread:9')
    mail$.messages.set([thread({ id: 'acc:other:thread:9#1', thread_id: 'acc:other:thread:9' })])

    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('')
  })

  it("keeps an active thread tab's selection the new view does not contain", async () => {
    // Leaving a kanban board restores the mail view's folder; a tab opened from
    // a card in another folder still owns the pane and must keep rendering.
    compose$.tabs.set([
      {
        id: 'thread-tab',
        kind: 'thread',
        messageId: '',
        threadId: 'acc:other:thread:9',
        subject: 'Tab',
        from: 'sender@example.com',
        body: '',
        viewMode: 'plain',
      },
    ])
    compose$.activeTab.set('thread-tab')
    ui$.selectedThread.set('acc:other:thread:9')

    try {
      await loadThreads()
      expect(ui$.selectedThread.get()).toBe('acc:other:thread:9')
    } finally {
      compose$.tabs.set([])
      compose$.activeTab.set('')
    }
  })

  it('keeps a selection whose conversation is still loading', async () => {
    ui$.selectedThread.set('acc:other:thread:9')
    mail$.threadLoading.set(true)

    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('acc:other:thread:9')
  })

  it('keeps the open thread when it is still in the loaded view', async () => {
    ui$.selectedThread.set('acc:inbox:thread:top')

    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('acc:inbox:thread:top')
  })

  it('does not replace the waiting mail list while a kanban card owns the folder selection', async () => {
    const sent = thread({ folder_id: 'sent', subject: 'Unified sent' })
    let calls = 0
    mail$.threads.set([sent])
    ui$.selectedFolder.set('[Gmail]/Sent Mail')
    kanban$.activeBoardId.set('board-1')
    ;(window as any).go.main.App.Invoke = async () => {
      calls += 1
      return { threads: [thread({ subject: 'Unified inbox' })] }
    }

    await loadThreads()

    expect(calls).toBe(0)
    expect(mail$.threads.get()).toEqual([sent])
  })

  it('drops a reselect request the skipped kanban load was meant to answer', async () => {
    kanban$.activeBoardId.set('board-1')
    // Deleting the last card of a column clears the pane and asks the load that
    // follows to pick the replacement — a load that never runs behind a board.
    requestThreadReselect()

    await loadThreads()

    kanban$.activeBoardId.set('')
    await loadThreads()

    expect(ui$.selectedThread.get()).toBe('')
  })
})

describe('thread list view identity', () => {
  const currentKey = () =>
    threadListViewKey(ui$.selectedAccount.get(), ui$.selectedFolder.get(), ui$.query.get(), ui$.filterMode.get())

  beforeEach(() => {
    mail$.threads.set([])
    mail$.threadsLoadedKey.set('')
    mail$.threadsCursor.set('')
    mail$.threadAccountCursors.set({})
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.selectedThread.set('')
    ui$.query.set('')
    ui$.filterMode.set('all')
    ;(window as any).go = {
      main: { App: { Invoke: async () => ({ threads: [], next_cursor: '' }) } },
    }
  })

  afterEach(() => {
    ui$.query.set('')
    ui$.filterMode.set('all')
    mail$.threadsLoadedKey.set('')
  })

  it('replaces a starred conversation when its representative folder changes', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    ui$.filterMode.set('starred')
    const old = thread({ starred: false })
    const replacement = thread({
      id: 'archive:1',
      thread_id: 'acc:archive:thread:1',
      folder_id: 'archive',
      starred: true,
    })
    ui$.selectedThread.set(old.thread_id)
    mail$.threads.set([old])
    ;(window as any).go.main.App.Invoke = async () => ({ items: [replacement] })

    await loadThreads(false)

    expect(mail$.threads.get()).toEqual([replacement])
  })

  it('keeps the selected starred conversation after opening clears unread', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    ui$.filterMode.set('unread')
    const row = thread({ thread_id: 'acc#INBOX#t.cm9vdA', starred: true, unread: false })
    ui$.selectedThread.set(row.thread_id)
    mail$.threads.set([row])
    ;(window as any).go.main.App.Invoke = async () => ({ items: [] })
    await loadThreads(false)
    expect(mail$.threads.get()).toEqual([row])
  })

  it('does not retain the selected copy when another folder represents its conversation', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    ui$.filterMode.set('unread')
    const row = thread({ thread_id: 'acc#INBOX#t.cm9vdA', starred: true })
    const replacement = thread({
      id: 'copy',
      thread_id: 'acc#Archive#t.cm9vdA',
      folder_id: 'Archive',
      starred: true,
      unread: true,
    })
    ui$.selectedThread.set(row.thread_id)
    mail$.threads.set([row])
    ;(window as any).go.main.App.Invoke = async () => ({ items: [replacement] })
    await loadThreads(false)
    expect(mail$.threads.get()).toEqual([replacement])
  })

  it('keeps selected UID-based conversations distinct across folders', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    ui$.filterMode.set('unread')
    const row = thread({ thread_id: 'acc#INBOX#1', starred: true })
    const other = thread({ id: 'other', thread_id: 'acc#Archive#1', folder_id: 'Archive', starred: true })
    ui$.selectedThread.set(row.thread_id)
    mail$.threads.set([row])
    ;(window as any).go.main.App.Invoke = async () => ({ items: [other] })
    await loadThreads(false)
    expect(mail$.threads.get()).toEqual([other, row])
  })

  it('restores a failed unstar without reverting a concurrent successful action', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    const row = thread({ starred: true })
    const other = thread({ id: 'other', thread_id: 'other', starred: true })
    mail$.threads.set([row, other])
    mail$.messages.set([row, other])
    kanban$.threads.set({ test: [row, other] })
    let reject!: (error: Error) => void
    ;(window as any).go.main.App.Invoke = async (_command: string, payload: any) => {
      if (payload.thread_id === row.thread_id)
        await new Promise((_resolve, fail) => {
          reject = fail
        })
      return { ok: true }
    }
    const failed = starThread(row.thread_id, false, { refresh: false })
    await starThread(other.thread_id, false, { refresh: false })
    reject(new Error('Unstar rejected'))
    expect(await failed).toBe(false)
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Unstar rejected')
    expect(mail$.threads.get()).toEqual([row])
    expect(mail$.messages.get()).toEqual([row, { ...other, starred: false }])
    expect(kanban$.threads.get().test).toEqual([row, { ...other, starred: false }])
  })

  it('refreshes bulk star once after every mutation settles, including failures', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    const rows = [thread({ starred: true }), thread({ id: 'other', thread_id: 'other', starred: true })]
    mail$.threads.set(rows)
    for (const row of rows) toggleBulkSelection(bulkItem(row))
    let finish!: () => void
    const calls: string[] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push(command)
      if (command === 'mail.starredItems') return { items: [rows[0]] }
      if (payload.thread_id === rows[0].thread_id) throw new Error('Unstar rejected')
      await new Promise<void>((resolve) => {
        finish = resolve
      })
      return { ok: true }
    }
    const result = bulkStarSelected(
      rows.map((row) => bulkItem(row)),
      false,
    )
    await nextTick()
    expect(calls.filter((command) => command === 'mail.starredItems')).toHaveLength(0)
    finish()
    await result
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Unstar rejected')
    expect(ui$.bulkSelection.get()).toEqual({})
    expect(calls.filter((command) => command === 'mail.starredItems')).toHaveLength(1)
    expect(mail$.threads.get()).toEqual([rows[0]])
  })

  it('shows an error instead of an undo toast when the star shortcut fails', async () => {
    const row = thread()
    mail$.threads.set([row])
    ;(window as any).go.main.App.Invoke = async () => {
      throw new Error('Star rejected')
    }
    toggleStarWithUndo(row.thread_id)
    await nextTick()
    expect(mail$.threads.get()).toEqual([row])
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Star rejected')
    expect(ui$.toastUndo.peek()).toBeNull()
  })

  it('refreshes every loaded starred row and uses the fresh cursor', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    const rows = Array.from({ length: 75 }, (_, i) => thread({ id: `m${i}`, thread_id: `t${i}`, starred: true }))
    mail$.threads.set(rows)
    mail$.threadsCursor.set('old-cursor')
    const calls: any[] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      return { items: rows.slice(1), next_cursor: 'new-cursor' }
    }

    await loadThreads(false)

    expect(calls).toEqual([
      { command: 'mail.starredItems', payload: { query: '', filter: 'all', attachments: false, limit: 75 } },
    ])
    expect(mail$.threads.get()).toEqual(rows.slice(1))
    expect(mail$.threadsCursor.get()).toBe('new-cursor')
  })

  it('removes an unstarred thread immediately and restores it on undo', async () => {
    ui$.selectedAccount.set('unified')
    ui$.selectedFolder.set('starred')
    const row = thread({ starred: true })
    mail$.threads.set([row])
    mail$.messages.set([row])
    let storedStar = true
    let finishMutation!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      if (command === 'mail.markStarred') {
        await new Promise<void>((resolve) => {
          finishMutation = resolve
        })
        storedStar = payload.starred
        return { ok: true }
      }
      return { items: storedStar ? [row] : [] }
    }

    const unstar = starThread(row.thread_id, false)
    expect(mail$.threads.get()).toEqual([])
    expect(mail$.messages.get()[0].starred).toBe(false)
    finishMutation()
    await unstar
    expect(mail$.threads.get()).toEqual([])

    const undo = starThread(row.thread_id, true)
    finishMutation()
    await undo
    expect(mail$.threads.get()).toEqual([row])
  })

  it('does not call a folder loaded until its load lands', async () => {
    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    await loadThreads()

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // The flash this guards against: the list repaints on the navigation itself,
  // a frame or more before the effect that calls loadThreads runs, so a flag
  // the loader sets would still be off for that paint.
  it('stops matching the moment the folder or filter changes, before any load runs', async () => {
    await loadThreads()
    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())

    ui$.selectedFolder.set('archive')
    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    await loadThreads()
    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())

    ui$.filterMode.set('unread')
    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())
  })

  // A sync event or feed edit refreshes the list on its own schedule. Landing in
  // the gap between a keystroke and the debounced search, it would otherwise
  // settle the pending query on cache-only rows.
  it('is not settled by a background refresh that lands mid-navigation', async () => {
    await loadThreads()

    ui$.selectedFolder.set('archive')
    await loadThreads(false)

    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    await loadThreads()

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // The overlap: a sync event fires while the view's own load is still out. If
  // the background call took the version, the foreground one would lose
  // `superseded` on arrival, and with no load left to settle the view the list
  // would spin for good.
  it('settles when a sync event fires while the view load is still in flight', async () => {
    let resolveForeground!: (value: unknown) => void
    const foregroundPage = new Promise((resolve) => {
      resolveForeground = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            if (payload.refresh) return foregroundPage
            return { threads: [], next_cursor: '' }
          },
        },
      },
    }

    const foreground = loadThreads()
    await nextTick()
    // The sync event, mid-flight.
    await loadThreads(false)

    expect(refreshes).toEqual([true])

    resolveForeground({ threads: [], next_cursor: '' })
    await foreground

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // Stepping aside is not the same as being answered: the load in flight read the
  // cache before the change the refresh is reacting to. A quick reply's post-send
  // draft discard used to run its refresh while the view's own load was still
  // out, and the rows that load then wrote still counted the discarded draft, so
  // the card showed one message too many until something else reloaded the list.
  it('runs a background refresh that stepped aside once the view load lands', async () => {
    let resolveForeground!: (value: unknown) => void
    const foregroundPage = new Promise((resolve) => {
      resolveForeground = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            if (payload.refresh) return foregroundPage
            return { threads: [], next_cursor: '' }
          },
        },
      },
    }

    const foreground = loadThreads()
    await nextTick()
    // The post-send discard's refresh, while the view load is still out.
    await loadThreads(false)
    expect(refreshes).toEqual([true])

    resolveForeground({ threads: [], next_cursor: '' })
    await foreground
    await nextTick()

    expect(refreshes).toEqual([true, false])
    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })

  // Stepping aside for the load in flight is only right while that load is still
  // going to write. A search superseded by the next keystroke used to hold the
  // claim for good, so every later background refresh of that view was skipped —
  // including the one a post-send draft discard runs, leaving the card's Draft
  // badge and message count stuck at what they were before the send.
  it('refreshes in the background again after a superseded search load', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            return payload.refresh ? livePage : { threads: [], next_cursor: '' }
          },
        },
      },
    }

    ui$.query.set('0821')
    const stale = loadThreads()
    await nextTick()
    // The next keystroke, then a correction back to the same query: the load
    // above lands with nothing left to write.
    ui$.query.set('0821 ')
    resolveLive({ threads: [], next_cursor: '' })
    await stale
    ui$.query.set('0821')

    refreshes.length = 0
    await loadThreads(false)

    expect(refreshes).toEqual([false])
  })

  // Same claim, still out on the wire: a search another load has overtaken will
  // drop whatever it returns, so waiting for it — a slow one can take the whole
  // 15s timeout — buys nothing and costs every background refresh in between.
  it('refreshes in the background while a superseded search load is still out', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    const refreshes: boolean[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) => {
            refreshes.push(payload.refresh ?? true)
            return payload.refresh ? livePage : { threads: [], next_cursor: '' }
          },
        },
      },
    }

    ui$.query.set('0821')
    const stale = loadThreads()
    await nextTick()
    // The next keystroke paints its own cache hits, then the user deletes it
    // again — the search above is now writing for nobody, still on the wire.
    ui$.query.set('0821 ')
    await loadThreads(false, 'cache')
    ui$.query.set('0821')

    refreshes.length = 0
    await loadThreads(false)

    expect(refreshes).toEqual([false])

    resolveLive({ threads: [], next_cursor: '' })
    await stale
  })

  // A search paints local hits first, then the live IMAP results. An empty cache
  // stage must not claim the search is answered while the live half is still out.
  it('waits for the live stage of a search, not the cache stage', async () => {
    let resolveLive!: (value: unknown) => void
    const livePage = new Promise((resolve) => {
      resolveLive = resolve
    })
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (_command: string, payload: { refresh?: boolean }) =>
            payload.refresh ? livePage : { threads: [], next_cursor: '' },
        },
      },
    }

    ui$.query.set('deploy')
    const loading = loadThreads()
    await nextTick()

    expect(mail$.threadsLoadedKey.get()).not.toBe(currentKey())

    resolveLive({ threads: [], next_cursor: '' })
    await loading

    expect(mail$.threadsLoadedKey.get()).toBe(currentKey())
  })
})
