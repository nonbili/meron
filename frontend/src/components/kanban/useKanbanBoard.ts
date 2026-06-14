import { useEffect, useRef, useState } from 'react'
import type { DragEndEvent, DragStartEvent } from '@dnd-kit/core'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { moveFeed } from '../../states/feeds'
import { showToast } from '../../states/ui'
import {
  getAllKanbanColumns,
  kanbanColumnKey,
  kanban$,
  reorderKanbanColumn,
  type KanbanColumn,
} from '../../states/kanban'
import type { Account, Message } from '../../types'
import {
  SEARCH_DEBOUNCE_MS,
  folderMatches,
  isUnifiedStarredColumn,
  isRSSAccount,
  loadKanbanColumn,
  searchTargets,
} from '../../lib/kanbanData'

// All of the board's data-sync effects: folder watches, the pane reset, the
// debounced search loads, and the mail.synced/newMessages refresh listener.
// Kept out of the view so the component body stays focused on rendering.
export function useKanbanBoardSync(boardId: string, visibleColumns: KanbanColumn[], accounts: Account[]) {
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)
  const searchedColumnKeysRef = useRef<Set<string>>(new Set())
  const watchedKanbanFoldersRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
  }, [boardId])

  useEffect(() => {
    const desired = new Set<string>()
    for (const column of visibleColumns) {
      if (column.accountId === 'unified') continue
      if (column.folderId.toLowerCase() === 'inbox') continue
      const account = accounts.find((item) => item.id === column.accountId)
      if (!account || account.paused || isRSSAccount(column.accountId, accounts)) continue
      desired.add(kanbanColumnKey(column))
    }

    const current = watchedKanbanFoldersRef.current
    for (const key of current) {
      if (desired.has(key)) continue
      const [account, folder] = key.split('\n')
      if (account && folder) void invoke('watch.stop', { account, folder })
      current.delete(key)
    }
    for (const key of desired) {
      if (current.has(key)) continue
      const [account, folder] = key.split('\n')
      if (account && folder) void invoke('watch.start', { account, folder })
      current.add(key)
    }
  }, [accounts, visibleColumns])

  useEffect(() => {
    return () => {
      for (const key of watchedKanbanFoldersRef.current) {
        const [account, folder] = key.split('\n')
        if (account && folder) void invoke('watch.stop', { account, folder })
      }
      watchedKanbanFoldersRef.current.clear()
    }
  }, [])

  useEffect(() => {
    if (searchScope === 'all') return
    if (visibleColumns.some((column) => kanbanColumnKey(column) === searchScope)) return
    kanban$.searchScope.set('all')
  }, [searchScope, visibleColumns])

  useEffect(() => {
    const query = searchQuery.trim()
    const targetColumns = query ? searchTargets(visibleColumns, searchScope) : []
    const targetKeys = new Set(targetColumns.map((column) => kanbanColumnKey(column)))
    const previousKeys = searchedColumnKeysRef.current

    const restoreColumns = visibleColumns.filter((column) => {
      const key = kanbanColumnKey(column)
      return previousKeys.has(key) && !targetKeys.has(key)
    })

    const run = () => {
      for (const column of restoreColumns) {
        void loadKanbanColumn(column, false)
      }
      for (const column of targetColumns) {
        void loadKanbanColumn(column, true, query)
      }
      searchedColumnKeysRef.current = targetKeys
    }

    if (!query) {
      run()
      return
    }

    const timer = window.setTimeout(run, SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [searchQuery, searchScope, visibleColumns])

  // Columns serve cached threads; when a background sync finishes — or new mail
  // arrives via IMAP IDLE — pull the freshly-cached threads into any column that
  // shows that account/folder. `mail.synced` covers flag-only refreshes; new
  // arrivals come as `mail.newMessages`, which carries the same account/folder.
  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (typeof eventsOn !== 'function') return
    // Coalesce reloads. At startup each account is synced separately, so a board
    // showing N accounts gets N mail.synced events back-to-back — and each event
    // would otherwise re-fetch every matching column once. Collect the columns to
    // reload (keyed by column key, latest query wins) and flush them once after a
    // short quiet window so a burst of syncs costs one threadList per column.
    const pending = new Map<string, { column: KanbanColumn; query: string }>()
    let flushTimer: number | undefined
    const flush = () => {
      flushTimer = undefined
      const jobs = [...pending.values()]
      pending.clear()
      for (const job of jobs) {
        void loadKanbanColumn(job.column, false, job.query)
      }
    }
    const reload = (detail: { account?: string; folder?: string }) => {
      const account = detail?.account
      if (!account) return
      const query = kanban$.searchQuery.peek().trim()
      const scope = kanban$.searchScope.peek()
      const syncedInbox = !detail?.folder || detail.folder.toLowerCase() === 'inbox'
      for (const column of getAllKanbanColumns()) {
        const key = kanbanColumnKey(column)
        const columnQuery = query && (scope === 'all' || scope === key) ? query : ''
        if (column.accountId === 'unified') {
          if (isUnifiedStarredColumn(column) || syncedInbox) pending.set(key, { column, query: columnQuery })
        } else if (column.accountId === account && folderMatches(column.folderId, detail?.folder)) {
          pending.set(key, { column, query: columnQuery })
        }
      }
      // Leading-window collect: the first event arms the flush; later events in
      // the window just add to the pending set, bounding latency at ~250ms.
      if (pending.size > 0 && flushTimer === undefined) {
        flushTimer = window.setTimeout(flush, 250)
      }
    }
    const offSynced = eventsOn('mail.synced', reload)
    const offNew = eventsOn('mail.newMessages', reload)
    return () => {
      if (flushTimer !== undefined) window.clearTimeout(flushTimer)
      if (typeof offSynced === 'function') offSynced()
      if (typeof offNew === 'function') offNew()
    }
  }, [])
}

// Thread/column drag-and-drop: the optimistic cross-column move plus the dnd-kit
// drag start/end handlers and the drag-overlay preview state.
export function useKanbanDnd(boardId: string, accounts: Account[]) {
  const [dragPreview, setDragPreview] = useState<{ thread: Message; column: KanbanColumn } | null>(null)

  async function moveThread(threadId: string, source: KanbanColumn, target: KanbanColumn) {
    if (source.folderId === target.folderId && source.accountId === target.accountId) return
    if (source.accountId === 'unified' || target.accountId === 'unified') {
      showToast('Unified columns are read-only. Select an account to move threads.', 'error')
      return
    }
    const queryForColumn = (column: KanbanColumn) => {
      const query = kanban$.searchQuery.peek().trim()
      const scope = kanban$.searchScope.peek()
      const key = kanbanColumnKey(column)
      return query && (scope === 'all' || scope === key) ? query : ''
    }
    const sourceKey = kanbanColumnKey(source)
    const targetKey = kanbanColumnKey(target)
    const sourceBefore = kanban$.threads[sourceKey].peek() ?? []
    const targetBefore = kanban$.threads[targetKey].peek() ?? []
    const movedThread = sourceBefore.find((thread) => thread.thread_id === threadId)
    const sourceRSS = isRSSAccount(source.accountId, accounts)
    const targetRSS = isRSSAccount(target.accountId, accounts)

    if (sourceRSS && targetRSS) {
      kanban$.movingThread.set(threadId)
      try {
        if (movedThread) {
          kanban$.threads[sourceKey].set(sourceBefore.filter((thread) => thread.thread_id !== threadId))
          kanban$.threads[targetKey].set([
            { ...movedThread, account_id: target.accountId, folder_id: target.folderId },
            ...targetBefore.filter((thread) => thread.thread_id !== threadId),
          ])
        }
        await moveFeed(threadId, target.accountId)
        await Promise.all([
          loadKanbanColumn(source, false, queryForColumn(source)),
          loadKanbanColumn(target, false, queryForColumn(target)),
        ])
      } catch {
        kanban$.threads[sourceKey].set(sourceBefore)
        kanban$.threads[targetKey].set(targetBefore)
      } finally {
        kanban$.movingThread.set('')
      }
      return
    }

    if (sourceRSS) {
      showToast('RSS feeds can only be moved to RSS accounts.', 'error')
      return
    }
    if (targetRSS) {
      showToast("Mail threads can't be moved into RSS feeds.", 'error')
      return
    }

    kanban$.movingThread.set(threadId)
    try {
      if (movedThread) {
        kanban$.threads[sourceKey].set(sourceBefore.filter((thread) => thread.thread_id !== threadId))
        kanban$.threads[targetKey].set([
          { ...movedThread, folder_id: target.folderId },
          ...targetBefore.filter((thread) => thread.thread_id !== threadId),
        ])
      }
      if (source.accountId === target.accountId) {
        await invoke('mail.move', { thread_id: threadId, target_folder_id: target.folderId })
        showToast('Thread moved')
      } else {
        const copyRes = await invoke<{ copied?: number }>('mail.copy', {
          thread_id: threadId,
          target_account_id: target.accountId,
          target_folder_id: target.folderId,
        })
        if (typeof copyRes?.copied === 'number' && copyRes.copied <= 0) {
          throw new Error('Copy failed: no matching messages found')
        }
        try {
          const deleteRes = await invoke<{ deleted?: number }>('mail.delete', {
            thread_id: threadId,
            folder: source.folderId,
          })
          if (typeof deleteRes?.deleted === 'number' && deleteRes.deleted <= 0) {
            throw new Error('no matching messages found')
          }
          showToast('Thread moved')
        } catch (error) {
          showToast(
            error instanceof Error
              ? `Copied, but couldn't move original to Trash: ${error.message}`
              : "Copied, but couldn't move original to Trash",
            'error',
          )
        }
      }
      await Promise.all([
        loadKanbanColumn(source, false, queryForColumn(source)),
        loadKanbanColumn(target, false, queryForColumn(target)),
      ])
    } catch (error) {
      kanban$.threads[sourceKey].set(sourceBefore)
      kanban$.threads[targetKey].set(targetBefore)
      showToast(error instanceof Error ? error.message : 'Move failed', 'error')
      await loadKanbanColumn(source, false, queryForColumn(source))
    } finally {
      kanban$.movingThread.set('')
    }
  }

  function handleDragStart(event: DragStartEvent) {
    const data = event.active.data.current
    if (data?.type !== 'thread') {
      setDragPreview(null)
      return
    }
    const source = data.source as KanbanColumn | undefined
    const threadId = String(data.threadId ?? event.active.id)
    if (!source) {
      setDragPreview(null)
      return
    }
    const thread = kanban$.threads[kanbanColumnKey(source)].peek()?.find((item) => item.thread_id === threadId)
    setDragPreview(thread ? { thread, column: source } : null)
  }

  function handleDragEnd(event: DragEndEvent) {
    setDragPreview(null)
    const activeData = event.active.data.current
    const overData = event.over?.data.current

    // Reordering a column: move the dragged column into the over column's slot.
    if (activeData?.type === 'column') {
      const active = activeData.column as KanbanColumn
      const over = overData?.column as KanbanColumn | undefined
      if (over && kanbanColumnKey(over) !== kanbanColumnKey(active)) {
        reorderKanbanColumn(boardId, active, over)
      }
      return
    }

    // Otherwise it's a thread being moved between columns.
    const source = activeData?.source as KanbanColumn | undefined
    const target = overData?.column as KanbanColumn | undefined
    if (!source || !target) return
    void moveThread(String(event.active.id), source, target)
  }

  return { dragPreview, setDragPreview, moveThread, handleDragStart, handleDragEnd }
}
