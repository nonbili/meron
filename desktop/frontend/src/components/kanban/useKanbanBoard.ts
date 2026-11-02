import { useEffect, useRef, useState } from 'react'
import type { DragEndEvent, DragStartEvent } from '@dnd-kit/core'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { t } from '../../lib/i18n'
import { mailTransferError } from '../../lib/mailTransferErrors'
import { moveFeed } from '../../states/feeds'
import { setThreadTags } from '../../states/mailTags'
import { showToast } from '../../states/ui'
import { rememberThreadTransfer } from '../../states/recentThreadTransfers'
import { kanbanColumnKey, kanban$, reorderKanbanColumn, type KanbanColumn } from '../../states/kanban'
import type { Account, Message } from '../../types'
import {
  KANBAN_MOVE_MESSAGES,
  SEARCH_DEBOUNCE_MS,
  isRSSAccount,
  loadKanbanColumn,
  resolveKanbanMove,
  searchTargets,
  subscribeKanbanMailReloads,
  useFoldersByAccount,
} from '../../lib/kanbanData'
import { accountFolderForRole, unifiedFolderRole } from '../../lib/unifiedFolders'

// All of the board's data-sync effects: folder watches, the pane reset, the
// debounced search loads, and the mail.synced/newMessages refresh listener.
// Kept out of the view so the component body stays focused on rendering.
export function useKanbanBoardSync(boardId: string, visibleColumns: KanbanColumn[], accounts: Account[]) {
  const searchQuery = useValue(kanban$.searchQuery)
  const searchScope = useValue(kanban$.searchScope)
  const foldersByAccount = useFoldersByAccount()
  const searchedColumnKeysRef = useRef<Set<string>>(new Set())
  const watchedKanbanFoldersRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    const desired = new Set<string>()
    for (const column of visibleColumns) {
      if (column.accountId === 'unified') {
        const role = unifiedFolderRole(column.folderId)
        if (role === 'inbox' || role === 'starred') continue
        for (const account of accounts) {
          if (account.included_in_unified === false || account.paused || isRSSAccount(account.id, accounts)) continue
          const folder = accountFolderForRole(foldersByAccount[account.id], role)
          if (folder) desired.add(kanbanColumnKey({ accountId: account.id, folderId: folder }))
        }
        continue
      }
      if (column.folderId.toLowerCase() === 'inbox') continue
      const account = accounts.find((item) => item.id === column.accountId)
      if (!account || account.paused || isRSSAccount(column.accountId, accounts)) continue
      // Keyed by folder alone: a saved search shares its folder's watch.
      desired.add(kanbanColumnKey({ accountId: column.accountId, folderId: column.folderId }))
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
  }, [accounts, foldersByAccount, visibleColumns])

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
  //
  // Mounted once: the folder cache is read at event time (peek) rather than
  // captured as a dependency. Those very events refresh the folder cache, so a
  // dependency would tear this effect down mid-flush — clearing the pending
  // 250ms timer below — and the new mail would never reach the column.
  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (typeof eventsOn !== 'function') return
    return subscribeKanbanMailReloads(eventsOn)
  }, [])
}

// Thread/column drag-and-drop: the optimistic cross-column move plus the dnd-kit
// drag start/end handlers and the drag-overlay preview state.
export function useKanbanDnd(boardId: string, accounts: Account[]) {
  const [dragPreview, setDragPreview] = useState<{ thread: Message; column: KanbanColumn } | null>(null)

  async function moveThread(threadId: string, source: KanbanColumn, target: KanbanColumn) {
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
    const resolution = resolveKanbanMove(source, target, movedThread, accounts)
    if (resolution.kind === 'noop') return
    if (resolution.kind === 'blocked') {
      showToast(t(resolution.reasonKey), 'error')
      return
    }
    if (resolution.kind === 'tag') {
      const { add, remove } = resolution
      kanban$.movingThread.set(threadId)
      try {
        await setThreadTags(threadId, add ? [add] : [], remove ? [remove] : [])
      } catch (error) {
        showToast(mailTransferError(error, t(KANBAN_MOVE_MESSAGES.failed)), 'error')
      } finally {
        kanban$.movingThread.set('')
      }
      await Promise.all([source, target].map((column) => loadKanbanColumn(column, false, queryForColumn(column))))
      return
    }
    const origin = resolution.origin
    // The account's own column may also be on the board; it shows the same
    // mailbox the thread just left, so it needs the reload too.
    const originKey = kanbanColumnKey(origin)
    const reloadColumns =
      originKey === sourceKey || originKey === targetKey ? [source, target] : [source, target, origin]
    const reloadAll = () =>
      Promise.all(reloadColumns.map((column) => loadKanbanColumn(column, false, queryForColumn(column))))
    // Feed↔mail mismatches were already refused above, so both flags agreeing is
    // the only way either is true here.
    if (isRSSAccount(origin.accountId, accounts) && isRSSAccount(target.accountId, accounts)) {
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
        await reloadAll()
      } catch {
        kanban$.threads[sourceKey].set(sourceBefore)
        kanban$.threads[targetKey].set(targetBefore)
      } finally {
        kanban$.movingThread.set('')
      }
      return
    }

    kanban$.movingThread.set(threadId)
    try {
      // First, while the thread id still names it: the move gives it a new one.
      if (resolution.removeTag) await setThreadTags(threadId, [], [resolution.removeTag])
      if (movedThread) {
        kanban$.threads[sourceKey].set(sourceBefore.filter((thread) => thread.thread_id !== threadId))
        kanban$.threads[targetKey].set([
          { ...movedThread, account_id: target.accountId, folder_id: target.folderId },
          ...targetBefore.filter((thread) => thread.thread_id !== threadId),
        ])
      }
      const result = await invoke<{ moved?: number }>('mail.move', {
        thread_id: threadId,
        target_account_id: target.accountId,
        target_folder_id: target.folderId,
      })
      if (typeof result?.moved === 'number' && result.moved <= 0) {
        throw new Error(t(KANBAN_MOVE_MESSAGES.noMatchingMessages))
      }
      rememberThreadTransfer({
        action: 'move',
        sourceAccountId: origin.accountId,
        targetAccountId: target.accountId,
        targetFolderId: target.folderId,
      })
      showToast(t(KANBAN_MOVE_MESSAGES.moved))
      await reloadAll()
    } catch (error) {
      // A drop that failed changes nothing, tag included.
      if (resolution.removeTag) await setThreadTags(threadId, [resolution.removeTag], []).catch(() => undefined)
      kanban$.threads[sourceKey].set(sourceBefore)
      kanban$.threads[targetKey].set(targetBefore)
      showToast(mailTransferError(error, t(KANBAN_MOVE_MESSAGES.failed)), 'error')
      await reloadAll()
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
    // Starred feed cards drag under their message id, so take the thread id from
    // the payload the card set rather than from the draggable's id.
    void moveThread(String(activeData?.threadId ?? event.active.id), source, target)
  }

  return { dragPreview, setDragPreview, moveThread, handleDragStart, handleDragEnd }
}
