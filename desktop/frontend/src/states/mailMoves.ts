import { threadWindowSession } from '../lib/windowSession'
import { forgetStandaloneRecoveryDraft } from '../lib/composeWindow'
import type { Folder, Message } from '../types'
import { rememberThreadTransfer } from './recentThreadTransfers'
import { accounts$, isLocalAccountId } from './accounts'
import { accountLabel } from '../lib/kanbanData'
import { invoke } from '../lib/bridge'
import { contextualErrorMessage, errorMessage } from '../lib/errors'
import { t } from '../lib/i18n'
import { mailTransferError } from '../lib/mailTransferErrors'
import {
  clearBulkSelection,
  confirmAction,
  deselectBulkThreads,
  ui$,
  showToast,
  showUndoToast,
  type BulkSelectionItem,
} from './ui'
import { getKanbanColumns, kanban$, kanbanKeyboardCardAfterRemoving, type KanbanCardRef } from './kanban'
import { isLocalSendId, discardPendingSend, cancelUnsentRescue } from './pendingSends'
import {
  captureKeys,
  findLocalThread,
  getFilteredThreads,
  kanbanKeysWithThread,
  listFilterKey,
  loadThreads,
  mail$,
  normalizeMessageId,
  reloadThreadCards,
  removeKanbanThread,
  completeRemovedThread,
  releaseRemovedThread,
  suppressRemovedThread,
  requestThreadReselect,
  threadListViewKey,
  uniqueThreadItems,
  updateKanbanThread,
} from './mail'
import {
  applyMutationFolderUnreads,
  isArchiveFolder,
  isDraftFolder,
  deletesForever,
  isTrashFolderId,
  loadFolders,
  type MutationResult,
  refreshAccountFoldersCache,
} from './mailFolders'

// Moving, copying, archiving, and deleting threads and messages.

function reconcileThreadDraftFromLoadedMessages(threadId: string, messages: Message[]) {
  if (!threadId) return
  const threadMessages = messages.filter((message) => message.thread_id === threadId)
  if (threadMessages.some((message) => isDraftFolder(message.folder_id, message.account_id))) return

  mail$.threads.set(
    mail$.threads.get().map((thread) => (thread.thread_id === threadId ? { ...thread, has_draft: false } : thread)),
  )
  updateKanbanThread(threadId, (thread) => ({ ...thread, has_draft: false }))
}

// Thread ids encode the source folder, so a surviving row with the same id
// means the message copies are still in that folder — the server didn't
// actually apply the change, even though the call reported success. Checked
// after the post-action refresh so the toast reflects what the list shows.
function threadStillListed(threadId: string): boolean {
  return mail$.threads.get().some((item) => item.thread_id === threadId)
}

function assertDeleteAffected(res: unknown) {
  if (!res || typeof res !== 'object') return
  const deleted = (res as { deleted?: unknown }).deleted
  if (typeof deleted === 'number' && deleted <= 0) {
    throw new Error('Delete failed: no matching messages found')
  }
}

type DeleteSource = { accountId?: string; folderId?: string }

/** The account and full folder path a delete came from: the view may not say
 *  (unified folders, search, a bulk selection across folders), and two folders
 *  can share a leaf name. */
function deleteSourceLabel(accountId: string, folderId: string, folder: Folder | undefined) {
  const name = folderId.toUpperCase() === 'INBOX' ? t('folders.roles.inbox') : folder?.name || folderId
  const accounts = accounts$.peek()
  const account = accounts.find((item) => item.id === accountId)
  // Two accounts may share a display name; their addresses tell them apart.
  const shared =
    !!account?.display_name &&
    accounts.some((item) => item.id !== accountId && item.display_name === account.display_name)
  return `${shared ? account.email : accountLabel(accountId, accounts)} / ${name}`
}

/** The account and folder a thread id names (`account#folder#key`), split the
 *  way the bridge splits it, for a thread no longer in any loaded list. */
function threadIdSource(threadId: string): DeleteSource {
  const first = threadId.indexOf('#')
  const last = threadId.lastIndexOf('#')
  if (first <= 0 || last <= first) return {}
  return { accountId: threadId.slice(0, first), folderId: threadId.slice(first + 1, last) }
}

/** A delete that may have moved mail to Trash is checked in its folder and
 *  that account's Trash when its outcome is unknown. Which folder the server
 *  expunges in place rather than moving out of is the server's call (a folder
 *  named, or even classified, as Trash may not be the one it uses), so only a
 *  local folder, which has no Trash, goes without the Trash check. */
function deleteFailedMessage(error: unknown, source: DeleteSource) {
  const { accountId, folderId } = source
  let hint = t('mail.transferError.checkDeleted')
  let permissionReason = t('mail.transferError.deletePermission')
  if (accountId && folderId) {
    const folder = [...(mail$.foldersByAccount.peek()?.[accountId] ?? []), ...mail$.folders.peek()].find(
      (item) => item.account_id === accountId && item.id === folderId,
    )
    const mailbox = deleteSourceLabel(accountId, folderId, folder)
    if (isLocalAccountId(accountId)) {
      // No server and no Trash: a refusal comes from local storage, in its
      // own words.
      hint = ''
      permissionReason = errorMessage(error, '').replace(/^sidecar \S+ error: /, '')
    } else {
      hint = t('mail.transferError.checkDeletedIn', { mailbox })
      permissionReason = t('mail.transferError.deletePermissionIn', { mailbox })
    }
  }
  return mailTransferError(error, t('mail.toast.deleteFailed'), { uncertainHint: hint, permissionReason })
}

function assertMoveAffected(res: unknown, label = 'Move') {
  if (!res || typeof res !== 'object') return
  const moved = (res as { moved?: unknown }).moved
  if (typeof moved === 'number' && moved <= 0) {
    throw new Error(`${label} failed: no matching messages found`)
  }
}

function assertCopyAffected(res: unknown) {
  if (!res || typeof res !== 'object') return
  const copied = (res as { copied?: unknown }).copied
  if (typeof copied === 'number' && copied <= 0) {
    throw new Error('Copy failed: no matching messages found')
  }
}

// UIDs of the copies a move created in its target folder, as the message_ids
// an undo sends back so it moves exactly those — not every message of the
// conversation the target already held, and not by a uid: thread key, whose
// UID the move changed. Undefined when the core couldn't tell the copies apart
// from other mail there: then no Undo is offered, since any other way back
// could move mail the action never touched.
type MovedCopiesResult = { target_uids?: number[] }

function movedCopyIds(res: MovedCopiesResult): string[] | undefined {
  return res.target_uids?.length ? res.target_uids.map(String) : undefined
}

function threadAccountId(threadId: string): string {
  const separator = threadId.indexOf('#')
  return separator > 0 ? threadId.slice(0, separator) : ''
}

function threadIdInFolder(threadId: string, accountId: string | undefined, folderId: string | undefined): string {
  const lastHash = threadId.lastIndexOf('#')
  if (!accountId || !folderId || lastHash <= 0) return threadId
  return `${accountId}#${folderId}#${threadId.slice(lastHash + 1)}`
}

// Neighbour of a thread inside the kanban column that holds it (next, or
// previous if it was last). The chat-view getFilteredThreads list doesn't apply
// in kanban, where cards live in per-column lists.
function threadsInPaneColumn(): Message[] {
  const paneKey = kanban$.paneColumnKey.get()
  const split = paneKey.indexOf('\n')
  if (split === -1) return []
  return kanban$.threads[paneKey.slice(split + 1)].get() ?? []
}

function kanbanNeighbourThreadId(threadId: string): string {
  for (const threads of Object.values(kanban$.threads.get())) {
    const index = threads.findIndex((thread) => thread.thread_id === threadId)
    if (index === -1) continue
    const neighbour = threads[index + 1] ?? threads[index - 1]
    return neighbour?.thread_id ?? ''
  }
  // The open card is already off the board (a column reload dropped it). Stay
  // on a card still in that column instead of closing the conversation.
  return threadsInPaneColumn().find((thread) => thread.thread_id !== threadId)?.thread_id ?? ''
}

function sameCard(a: KanbanCardRef | null, b: KanbanCardRef | null): boolean {
  return a?.threadId === b?.threadId && a?.columnKey === b?.columnKey && a?.itemId === b?.itemId
}

function restoreRemovedThread(current: Message[], previous: Message[], threadId: string): Message[] {
  const thread = previous.find((item) => item.thread_id === threadId)
  if (!thread || current.some((item) => item.thread_id === threadId)) return current
  const positions = new Map(previous.map((item, index) => [item.thread_id, index]))
  return [...current, thread].sort(
    (a, b) =>
      b.date - a.date ||
      (positions.get(a.thread_id) ?? previous.length) - (positions.get(b.thread_id) ?? previous.length),
  )
}

// Which list mail$.threads holds, so a late rollback can tell it was left.
function currentListView() {
  return threadListViewKey(ui$.selectedAccount.peek(), ui$.selectedFolder.peek(), ui$.query.peek(), listFilterKey())
}

// What the board's columns are showing: a column pointed at another folder, or
// a search or filter changed since, means their lists were re-read.
function currentKanbanView() {
  const boardId = kanban$.activeBoardId.peek()
  return JSON.stringify([
    boardId,
    boardId ? getKanbanColumns(boardId) : [],
    kanban$.searchQuery.peek().trim(),
    kanban$.searchScope.peek(),
    kanban$.globalFilter.peek(),
    kanban$.globalAttachmentsOnly.peek(),
    kanban$.filters.peek(),
    kanban$.attachmentsOnly.peek(),
  ])
}

/** Lists a rollback could not put its thread back into, because they now show
 *  something else; they are re-read instead. */
type StaleLists = { list: boolean; columns: string[] }

function reloadStaleLists(stale: StaleLists) {
  // The re-read lists the thread wherever it really is, now that its
  // suppression is lifted; until then it is missing, not misplaced.
  if (stale.list) void coalescedBackgroundListRefresh()
  if (stale.columns.length > 0) void reloadThreadCards(stale.columns)
}

function removeThreadLocally(threadId: string) {
  const previousView = currentListView()
  const previousKanbanView = currentKanbanView()
  // On a board, opening a card points selectedFolder at its mailbox, so the
  // list view key moves without the screen changing.
  const wasOnBoard = !!kanban$.activeBoardId.peek()
  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const previousKanbanThreads = captureKeys(kanban$.threads.get(), kanbanKeysWithThread(threadId))
  const previousSelected = ui$.selectedThread.get()
  const previousPaneThreadId = kanban$.paneThreadId.get()
  // When the selected thread is the one leaving, advance to its neighbour in the
  // visible list (next, or previous if it was last) instead of snapping back to
  // the top — this keeps keyboard triage (e/# on the j/k selection) in place.
  // Computed before mutating mail$.threads, since getFilteredThreads reads it.
  let nextSelected = previousSelected
  if (previousSelected === threadId) {
    if (threadWindowSession) {
      nextSelected = ''
    } else if (kanban$.activeBoardId.get()) {
      nextSelected = kanbanNeighbourThreadId(threadId)
    } else {
      const visible = getFilteredThreads()
      const index = visible.findIndex((thread) => thread.thread_id === threadId)
      // Skip over rows of the deleted thread itself — the unified starred folder
      // can list the same thread once per folder it is starred in. When the
      // open row is already gone (a refresh dropped it mid-burst), stay on the
      // first row still on screen instead of clearing the conversation.
      const neighbour =
        index === -1
          ? visible.find((item) => item.thread_id !== threadId)
          : (visible.slice(index + 1).find((item) => item.thread_id !== threadId) ??
            visible
              .slice(0, index)
              .reverse()
              .find((item) => item.thread_id !== threadId))
      nextSelected = neighbour?.thread_id ?? ''
    }
  }
  // The card selected from the keyboard moves on the same way, so the next
  // action doesn't land on the thread that just left.
  const previousKeyboardCard = kanban$.keyboardCard.get()
  const keyboardCardLeaves = previousKeyboardCard?.threadId === threadId
  const nextKeyboardCard = keyboardCardLeaves ? kanbanKeyboardCardAfterRemoving(previousKeyboardCard) : null
  const nextThreads = previousThreads.filter((thread) => thread.thread_id !== threadId)
  const unsuppress = suppressRemovedThread(threadId)

  mail$.threads.set(nextThreads)
  mail$.messages.set(previousMessages.filter((message) => message.thread_id !== threadId))
  removeKanbanThread(threadId)
  if (previousSelected === threadId) {
    ui$.selectedThread.set(nextSelected)
    // No neighbour in the list we have: let the fresh list pick the replacement.
    if (!nextSelected) requestThreadReselect()
  }
  // If the kanban conversation pane was open on the deleted card, follow the
  // selection so it doesn't keep rendering the removed thread.
  if (previousPaneThreadId === threadId) {
    kanban$.paneThreadId.set(nextSelected)
  }
  if (keyboardCardLeaves) kanban$.keyboardCard.set(nextKeyboardCard)

  return {
    complete: () => completeRemovedThread(threadId),
    // `deferReload` hands the stale lists back for the caller to re-read once
    // for a whole batch.
    rollback: (options: { deferReload?: boolean } = {}): StaleLists => {
      unsuppress()
      // Put this thread back only. Restoring the whole list would resurrect
      // threads archived after this one started, which is what a fast burst
      // looks like when an earlier move fails. Another folder, search or
      // filter may be showing by now (a bulk batch runs for seconds), where
      // this snapshot does not belong.
      const stale: StaleLists = { list: false, columns: [] }
      if (currentListView() === previousView) {
        mail$.threads.set(restoreRemovedThread(mail$.threads.get(), previousThreads, threadId))
      } else stale.list = true
      const currentMessages = mail$.messages.get()
      const seenMessages = new Set(currentMessages.map((message) => message.id))
      const missingMessages = previousMessages.filter(
        (message) => message.thread_id === threadId && !seenMessages.has(message.id),
      )
      if (missingMessages.length > 0) mail$.messages.set([...currentMessages, ...missingMessages])
      const sameBoardView = currentKanbanView() === previousKanbanView
      for (const [key, columnThreads] of previousKanbanThreads) {
        if (!columnThreads) continue
        if (sameBoardView) {
          kanban$.threads[key].set(restoreRemovedThread(kanban$.threads[key].get() ?? [], columnThreads, threadId))
        } else stale.columns.push(key)
      }
      // The reader may already have archived the neighbour, or left for
      // another view, where this thread is not on screen to open. Only come
      // back here when the selection is still the one this removal chose.
      const sameView = previousKanbanView === currentKanbanView() && (wasOnBoard || currentListView() === previousView)
      if (sameView) {
        if (ui$.selectedThread.get() === nextSelected) ui$.selectedThread.set(previousSelected)
        if (previousPaneThreadId === threadId && kanban$.paneThreadId.get() === nextSelected) {
          kanban$.paneThreadId.set(previousPaneThreadId)
        }
        if (keyboardCardLeaves && sameCard(kanban$.keyboardCard.get(), nextKeyboardCard)) {
          kanban$.keyboardCard.set(previousKeyboardCard)
        }
      }
      if (!options.deferReload) reloadStaleLists(stale)
      return stale
    },
  }
}

// One list rewrite for a burst of archives. Each move waits on IMAP for a few
// seconds and then reloads; letting every one of those land repaints the list
// out from under the selection the reader has already moved past.
let backgroundListRefresh: Promise<void> | null = null
let backgroundListRefreshAgain = false

function coalescedBackgroundListRefresh(): Promise<void> {
  if (!backgroundListRefresh) backgroundListRefresh = drainBackgroundListRefresh()
  else backgroundListRefreshAgain = true
  return backgroundListRefresh
}

async function drainBackgroundListRefresh() {
  try {
    do {
      backgroundListRefreshAgain = false
      await loadThreads(false)
    } while (backgroundListRefreshAgain)
  } finally {
    backgroundListRefresh = null
    if (backgroundListRefreshAgain) {
      backgroundListRefreshAgain = false
      void coalescedBackgroundListRefresh()
    }
  }
}

async function refreshThreadLocation(accountId?: string, refresh = false) {
  if (!threadWindowSession) {
    if (refresh) await loadThreads(true)
    else await coalescedBackgroundListRefresh()
  }
  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) {
    void loadFolders(selectedAcc, false)
  }
  if (accountId && accountId !== selectedAcc) {
    void loadFolders(accountId, false)
  }
}

export async function moveThreadToFolder(
  threadId: string,
  targetFolderId: string,
  options: { undo?: boolean; messageIds?: string[]; targetAccountId?: string } = {},
) {
  if (!threadId || !targetFolderId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  const sourceAccountId = sourceThread?.account_id || threadAccountId(threadId)
  const targetAccountId = options.targetAccountId || sourceAccountId
  const crossAccount = targetAccountId !== sourceAccountId
  if (!crossAccount && sourceFolder === targetFolderId) return
  const targetThreadId = threadIdInFolder(threadId, sourceThread?.account_id, targetFolderId)

  const { rollback, complete } = removeThreadLocally(threadId)
  try {
    const res = await invoke<MovedCopiesResult & MutationResult>('mail.move', {
      thread_id: threadId,
      target_folder_id: targetFolderId,
      ...(options.targetAccountId ? { target_account_id: options.targetAccountId } : {}),
      ...(options.messageIds ? { message_ids: options.messageIds } : {}),
    })
    assertMoveAffected(res)
    complete()
    applyMutationFolderUnreads(res)
    const copyIds = movedCopyIds(res)
    // The move already updated the cache. Reloading the first page in place
    // keeps the threads the reader had scrolled to; replacing the whole list
    // would drop them and can close the conversation that just became selected.
    await refreshThreadLocation(sourceThread?.account_id, false)
    if (crossAccount) void loadFolders(targetAccountId, false)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.moveFailedInSameFolder'), 'error')
      return false
    } else {
      if (options.undo !== false && sourceAccountId) {
        rememberThreadTransfer({
          action: 'move',
          sourceAccountId,
          targetAccountId,
          targetFolderId,
        })
      }
      if (!crossAccount && options.undo !== false && sourceFolder && copyIds) {
        showUndoToast(t('mail.toast.threadMoved'), () => {
          releaseRemovedThread(threadId)
          void moveThreadToFolder(targetThreadId, sourceFolder, { undo: false, messageIds: copyIds })
        })
      } else {
        showToast(t('mail.toast.threadMoved'))
      }
    }
    return true
  } catch (error) {
    rollback()
    showToast(mailTransferError(error, t('mail.toast.moveFailed')), 'error')
    if (crossAccount) void loadFolders(targetAccountId, false)
    return false
  }
}

export async function copyThreadToFolder(threadId: string, targetAccountId: string, targetFolderId: string) {
  if (!threadId || !targetAccountId || !targetFolderId) return
  const sourceThread = findLocalThread(threadId)
  const sourceAccountId = sourceThread?.account_id || threadAccountId(threadId)
  try {
    const res = await invoke('mail.copy', {
      thread_id: threadId,
      target_account_id: targetAccountId,
      target_folder_id: targetFolderId,
    })
    assertCopyAffected(res)
    if (sourceAccountId) {
      rememberThreadTransfer({
        action: 'copy',
        sourceAccountId,
        targetAccountId,
        targetFolderId,
      })
    }
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (targetAccountId !== sourceThread?.account_id) {
      void loadFolders(targetAccountId, false)
    }
    showToast(t('mail.toast.threadCopied'))
  } catch (error) {
    showToast(mailTransferError(error, t('mail.toast.copyFailed')), 'error')
  }
}

/** After an archive in place, the conversation's copies left the Inbox
 * (`sourceFolder`): take its Inbox card off wherever it shows, a Kanban Inbox
 * column for one, while its card in the Archive folder stays. */
/** Returns the id of the Inbox card it removed, if any. */
function removeInboxCopyCard(threadId: string, accountId: string | undefined, sourceFolder: string | undefined) {
  if (!sourceFolder) return ''
  const inboxThreadId = threadIdInFolder(threadId, accountId || threadAccountId(threadId), sourceFolder)
  if (inboxThreadId === threadId) return ''
  removeThreadLocally(inboxThreadId).complete()
  return inboxThreadId
}

let bulkRemovalsRunning = 0

/** While a bulk delete, archive or move runs, its selection is already gone:
 *  a Delete or archive shortcut must not fall back to the thread that came
 *  into view behind it. */
export function bulkRemovalInProgress(): boolean {
  return bulkRemovalsRunning > 0
}

// A bulk action takes every row off the list and leaves selection mode up
// front. The server still gets one request at a time, so removing each row just
// before its own request had them trickle away one round-trip apart, under a
// selection bar with nothing left to show. `keepsRow` marks rows that stay
// listed whatever happens, so there is nothing to remove or roll back.
function beginBulkRemoval(
  items: BulkSelectionItem[],
  targets: BulkSelectionItem[],
  keepsRow: (item: BulkSelectionItem) => boolean = () => false,
) {
  const view = currentListView()
  const board = currentKanbanView()
  const removals = targets.map((item) => (keepsRow(item) ? null : removeThreadLocally(item.threadId)))
  const settled = new Set<number>()
  deselectBulkThreads(items)
  bulkRemovalsRunning++
  return {
    removals,
    // A target that an earlier request in the batch already took away for good
    // (archiving a conversation in All Mail clears its Inbox card): its own
    // request will find nothing to do, and must not bring its card back.
    settle(threadId: string) {
      const index = targets.findIndex((item) => item.threadId === threadId)
      if (index === -1) return
      removals[index]?.complete()
      removals[index] = null
      settled.add(index)
    },
    // After a failure at `index`, that thread and the ones not yet sent come
    // back (latest first, so each rollback's selection check sees the state it
    // left) and are selected again for a retry — unless the reader has moved
    // to another view or started a selection elsewhere meanwhile.
    fail(index: number) {
      const stale: StaleLists = { list: false, columns: [] }
      for (const removal of removals.slice(index).reverse()) {
        const result = removal?.rollback({ deferReload: true })
        if (!result) continue
        stale.list ||= result.list
        stale.columns.push(...result.columns.filter((key) => !stale.columns.includes(key)))
      }
      reloadStaleLists(stale)
      const returned = targets.slice(index).filter((_, offset) => !settled.has(index + offset))
      if (returned.length === 0 || currentListView() !== view || currentKanbanView() !== board) return
      const current = ui$.bulkSelection.peek()
      if (Object.values(current).some((item) => item.groupKey !== returned[0].groupKey)) return
      ui$.bulkSelection.set({ ...current, ...Object.fromEntries(returned.map((item) => [item.key, item])) })
      if (!ui$.bulkAnchorKey.peek()) ui$.bulkAnchorKey.set(returned[returned.length - 1].key)
    },
    end() {
      bulkRemovalsRunning--
      // Nothing selected now: a range anchor left on a row deselected before
      // the batch would start the next Shift-click from somewhere unexpected.
      if (Object.keys(ui$.bulkSelection.peek()).length === 0) ui$.bulkAnchorKey.set('')
    },
  }
}

export async function bulkArchiveSelected(items: BulkSelectionItem[]) {
  // Local folders have no Archive folder.
  const targets = uniqueThreadItems(items).filter(
    (item) => !item.draft && !item.trash && !isLocalAccountId(item.accountId),
  )
  if (targets.length === 0) return
  let completed = 0
  let alreadyArchived = 0
  const sourceAccountIds = targets.map((item) => findLocalThread(item.threadId)?.account_id)
  // Threads archived in the Archive folder itself stay listed there (see archiveThread).
  const batch = beginBulkRemoval(items, targets, (item) => isArchiveFolder(item.folderId, item.accountId))
  const { removals } = batch
  let index = 0
  try {
    for (; index < targets.length; index++) {
      const item = targets[index]
      const removal = removals[index]
      const res = await invoke<{ already_archived?: boolean; thread_id?: string; source_folder?: string }>(
        'mail.archive',
        {
          thread_id: item.threadId,
        },
      )
      if (res?.already_archived) {
        // Nothing left in the Inbox: nothing to do, and the thread stays put.
        removal?.rollback()
        completed++
        alreadyArchived++
        continue
      }
      assertMoveAffected(res, 'Archive')
      const archivedInPlace = res?.thread_id === item.threadId
      if (archivedInPlace) {
        removal?.rollback()
        const inboxThreadId = removeInboxCopyCard(item.threadId, item.accountId, res?.source_folder)
        if (inboxThreadId) batch.settle(inboxThreadId)
      } else if (removal) removal.complete()
      else if (res?.thread_id) removeThreadLocally(item.threadId).complete()
      completed++
      applyMutationFolderUnreads(res as MutationResult)
      const sourceAccountId = sourceAccountIds[index]
      if (sourceAccountId) void refreshAccountFoldersCache(sourceAccountId, false)
    }
    await refreshThreadLocation(undefined, false)
    const archived = targets.length - alreadyArchived
    showToast(archived ? t('mail.toast.archivedCount', { count: archived }) : t('mail.toast.alreadyArchived'))
  } catch (error) {
    batch.fail(index)
    const message = mailTransferError(error, t('mail.toast.archiveFailed'))
    showToast(
      completed ? `${message} ${t('mail.transferError.bulkProgress', { completed, total: targets.length })}` : message,
      'error',
    )
    if (completed) void refreshThreadLocation(undefined, false)
  } finally {
    batch.end()
  }
}

export async function bulkMoveSelectedToFolder(
  items: BulkSelectionItem[],
  targetFolderId: string,
  targetAccountId?: string,
) {
  const targets = uniqueThreadItems(items).filter(
    (item) => (targetAccountId && item.accountId !== targetAccountId) || item.folderId !== targetFolderId,
  )
  if (targets.length === 0) return
  let completed = 0
  const batch = beginBulkRemoval(items, targets)
  const { removals } = batch
  let index = 0
  try {
    for (; index < targets.length; index++) {
      const item = targets[index]
      const res = await invoke('mail.move', {
        thread_id: item.threadId,
        target_folder_id: targetFolderId,
        ...(targetAccountId ? { target_account_id: targetAccountId } : {}),
      })
      assertMoveAffected(res)
      removals[index]?.complete()
      completed++
      rememberThreadTransfer({
        action: 'move',
        sourceAccountId: item.accountId,
        targetAccountId: targetAccountId || item.accountId,
        targetFolderId,
      })
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(targets[0]?.accountId, false)
    if (targetAccountId) void loadFolders(targetAccountId, false)
    showToast(t('mail.toast.movedCount', { count: targets.length }))
  } catch (error) {
    batch.fail(index)
    const message = mailTransferError(error, t('mail.toast.moveFailed'))
    showToast(
      completed ? `${message} ${t('mail.transferError.bulkProgress', { completed, total: targets.length })}` : message,
      'error',
    )
    if (completed) void refreshThreadLocation(undefined, false)
  } finally {
    batch.end()
  }
}

export async function bulkCopySelectedToFolder(
  items: BulkSelectionItem[],
  targetAccountId: string,
  targetFolderId: string,
) {
  const targets = uniqueThreadItems(items)
  if (targets.length === 0) return
  let completed = 0
  try {
    for (const item of targets) {
      const res = await invoke('mail.copy', {
        thread_id: item.threadId,
        target_account_id: targetAccountId,
        target_folder_id: targetFolderId,
      })
      assertCopyAffected(res)
      completed++
      rememberThreadTransfer({ action: 'copy', sourceAccountId: item.accountId, targetAccountId, targetFolderId })
    }
    await refreshThreadLocation(targets[0]?.accountId, true)
    if (targetAccountId !== targets[0]?.accountId) void loadFolders(targetAccountId, false)
    clearBulkSelection()
    showToast(t('mail.toast.copiedCount', { count: targets.length }))
  } catch (error) {
    const message = mailTransferError(error, t('mail.toast.copyFailed'))
    showToast(
      completed ? `${message} ${t('mail.transferError.bulkProgress', { completed, total: targets.length })}` : message,
      'error',
    )
    if (completed) void refreshThreadLocation(undefined, false)
  }
}

// One bulk delete at a time: one that fails puts its rest back in the
// selection, and a second batch started meanwhile could send those twice; on a
// server that copies to Trash before expunging, that is two Trash copies.
let bulkDeleteRunning = false

export async function bulkDeleteSelected(items: BulkSelectionItem[]) {
  if (bulkDeleteRunning) {
    if (uniqueThreadItems(items).length > 0) showToast(t('mail.toast.deleteStillRunning'), 'error')
    return
  }
  bulkDeleteRunning = true
  try {
    await deleteBulkTargets(items)
  } finally {
    bulkDeleteRunning = false
  }
}

async function deleteBulkTargets(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items)
  if (targets.length === 0) return
  const permanentTargets = targets.filter(
    (item) => item.draft || item.trash || deletesForever(item.accountId, item.folderId),
  )
  if (permanentTargets.length > 0) {
    const confirmed = await confirmAction({
      title: permanentTargets.length === targets.length ? 'Delete selected forever?' : 'Delete selected threads?',
      message:
        permanentTargets.length === targets.length
          ? `${permanentTargets.length} selected thread(s) will be permanently deleted. This can't be undone.`
          : `${permanentTargets.length} selected thread(s) will be permanently deleted; the rest will move to Trash.`,
      confirmLabel: 'Delete selected',
      tone: 'danger',
    })
    if (!confirmed) return
  }

  const completed: BulkSelectionItem[] = []
  // Read before the rows leave the store.
  const recoveryIds = targets.map((item) => recoveryDraftIdsInThread(item.threadId))
  const batch = beginBulkRemoval(items, targets)
  const { removals } = batch
  let index = 0
  try {
    for (; index < targets.length; index++) {
      const item = targets[index]
      const res = await invoke('mail.delete', {
        thread_id: item.threadId,
        ...(item.folderId ? { folder: item.folderId } : {}),
      })
      assertDeleteAffected(res)
      recoveryIds[index].forEach(({ id, account }) => forgetStandaloneRecoveryDraft(id, account))
      removals[index]?.complete()
      completed.push(item)
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(undefined, false)
    showToast(t('mail.toast.deletedCount', { count: targets.length }))
  } catch (error) {
    // Threads already deleted stay gone, so a retry carries only the rest.
    batch.fail(index)
    const failed = targets[Math.min(index, targets.length - 1)]
    const message = deleteFailedMessage(error, { accountId: failed?.accountId, folderId: failed?.folderId })
    showToast(
      completed.length ? `${message} ${t('mail.toast.deletedCount', { count: completed.length })}.` : message,
      'error',
    )
  } finally {
    batch.end()
  }
}

/** Undo for an archive done in the Archive folder itself: the copies go back
 * to the folder they left, but the thread stays listed here (on Gmail, All Mail
 * still lists it), so the card stays put and "still listed" is not a failure. */
async function restoreArchivedInPlace(
  threadId: string,
  targetFolderId: string,
  messageIds: string[],
  accountId: string | undefined,
) {
  try {
    const res = await invoke<MutationResult>('mail.move', {
      thread_id: threadId,
      target_folder_id: targetFolderId,
      message_ids: messageIds,
    })
    assertMoveAffected(res)
    applyMutationFolderUnreads(res)
    await refreshThreadLocation(accountId, false)
    showToast(t('mail.toast.threadMoved'))
  } catch (error) {
    showToast(mailTransferError(error, t('mail.toast.moveFailed')), 'error')
  }
}

export async function archiveThread(threadId: string) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  if (isLocalAccountId(sourceThread?.account_id || threadAccountId(threadId))) return
  const sourceFolder = sourceThread?.folder_id ?? ''
  // Archiving in the Archive folder itself (Gmail's All Mail) leaves the thread
  // listed there, so the card and the selection stay put instead of leaving
  // and coming back with the next list load.
  const staysListed = !!sourceFolder && isArchiveFolder(sourceFolder, sourceThread?.account_id)
  const removal = staysListed ? null : removeThreadLocally(threadId)
  try {
    const res = await invoke<
      { folder?: string; thread_id?: string; source_folder?: string; already_archived?: boolean } & MovedCopiesResult &
        MutationResult
    >('mail.archive', {
      thread_id: threadId,
    })
    if (res.already_archived) {
      removal?.rollback()
      showToast(t('mail.toast.alreadyArchived'))
      return
    }
    assertMoveAffected(res, 'Archive')
    const archivedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.folder)
    // Viewed in the archive folder itself (Gmail's All Mail): the core took the
    // Inbox copies out, the thread stays listed here, and undo puts them back
    // in the folder they left.
    // Only on the core's word: a reply without a thread_id falls back to the
    // id it came with, which would look the same.
    const archivedInPlace = res.thread_id === threadId
    // The folder wasn't known to be the Archive folder (metadata not loaded
    // yet), but the thread was archived in place: put the card back.
    if (archivedInPlace) {
      removal?.rollback()
      removeInboxCopyCard(threadId, sourceThread?.account_id, res.source_folder)
    } else if (removal) removal.complete()
    // An archive-named folder that isn't the account's Archive target (a Gmail
    // label called "Archive"): the thread did leave, so take the card off now.
    else removeThreadLocally(threadId).complete()
    applyMutationFolderUnreads(res)
    // On Gmail an in-place archive only drops the Inbox label: the All Mail
    // copies keep their UIDs, the core finds no new copies, and no Undo is
    // offered (as for any Gmail archive). A plain IMAP Archive folder gets one.
    const undoFolder = archivedInPlace ? (res.source_folder ?? '') : sourceFolder
    const copyIds = movedCopyIds(res)
    await refreshThreadLocation(sourceThread?.account_id, false)
    if (!archivedInPlace && threadStillListed(threadId)) {
      showToast(t('mail.toast.archiveFailedInSameFolder'), 'error')
    } else if (undoFolder && copyIds) {
      // Offer to move it back where it came from. Falls back to a plain toast
      // when the origin folder or the archived copies are unknown (nothing
      // reliable to restore to, or from).
      showUndoToast(t('mail.toast.archivedCount', { count: 1 }), () => {
        if (archivedInPlace) {
          void restoreArchivedInPlace(threadId, undoFolder, copyIds, sourceThread?.account_id)
          return
        }
        releaseRemovedThread(threadId)
        void moveThreadToFolder(archivedThreadId, undoFolder, { undo: false, messageIds: copyIds })
      })
    } else {
      showToast(t('mail.toast.archivedCount', { count: 1 }))
    }
  } catch (error) {
    removal?.rollback()
    showToast(mailTransferError(error, t('mail.toast.archiveFailed')), 'error')
  }
}

function recoveryDraftIdsInThread(threadId: string) {
  return [
    findLocalThread(threadId),
    ...mail$.messages.peek().filter((message) => message.thread_id === threadId),
  ].flatMap((message) => (message?.message_id ? [{ id: message.message_id, account: message.account_id }] : []))
}

export async function deleteThread(threadId: string, options: { permanent?: boolean } = {}) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  // Drafts are expunged in place by the engine (never moved to Trash), so the
  // delete is permanent there too — but worded as a discard.
  const isDraft =
    sourceThread?.folder_role === 'drafts' ||
    (!sourceThread?.folder_role && isDraftFolder(sourceFolder, sourceThread?.account_id))
  const isTrash =
    sourceThread?.folder_role === 'trash' ||
    (!sourceThread?.folder_role && isTrashFolderId(sourceThread?.account_id ?? '', sourceFolder))
  const permanent = options.permanent ?? (isDraft || isTrash || isLocalAccountId(sourceThread?.account_id))
  if (isDraft || permanent) {
    if (
      !(await confirmAction({
        title: isDraft ? 'Discard draft?' : 'Delete thread forever?',
        message: isDraft
          ? "This draft will be permanently deleted. This can't be undone."
          : "This thread will be permanently deleted. This can't be undone.",
        confirmLabel: isDraft ? 'Discard' : 'Delete forever',
        tone: 'danger',
      }))
    ) {
      return
    }
  }

  const recoveryIds = recoveryDraftIdsInThread(threadId)
  const { rollback, complete } = removeThreadLocally(threadId)

  try {
    const res = await invoke<
      { deleted?: number; permanent?: boolean; trash?: string; thread_id?: string } & MovedCopiesResult & MutationResult
    >('mail.delete', {
      thread_id: threadId,
      ...(sourceFolder ? { folder: sourceFolder } : {}),
    })
    assertDeleteAffected(res)
    recoveryIds.forEach(({ id, account }) => forgetStandaloneRecoveryDraft(id, account))
    complete()
    applyMutationFolderUnreads(res)
    const trashedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.trash)
    const copyIds = movedCopyIds(res)
    const canUndoTrashMove = !!(res.thread_id || res.trash) && !!copyIds
    await refreshThreadLocation(undefined, false)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.deleteFailedInSameFolder'), 'error')
    } else if (!isDraft && !permanent && !res.permanent && sourceFolder && canUndoTrashMove) {
      showUndoToast(t('mail.toast.threadMovedToTrash'), () => {
        releaseRemovedThread(threadId)
        void moveThreadToFolder(trashedThreadId, sourceFolder, { undo: false, messageIds: copyIds })
      })
    } else {
      showToast(
        isDraft
          ? t('mail.toast.draftDiscarded')
          : permanent
            ? t('mail.toast.deletedCount', { count: 1 })
            : t('mail.toast.threadMovedToTrash'),
      )
    }
  } catch (error) {
    rollback()
    showToast(
      deleteFailedMessage(error, {
        accountId: sourceThread?.account_id ?? threadIdSource(threadId).accountId,
        folderId: sourceFolder || threadIdSource(threadId).folderId,
      }),
      'error',
    )
  }
}

export async function discardSavedDraftCopy(
  draft: {
    threadId: string
    messageId: string
    folderId: string
    accountId?: string
    draftMessageId?: string
    /** The conversation a full-editor reply was written into. Unlike
     *  `threadId` — the discarded draft's own thread, which the discard can
     *  empty — this is a thread the reply is only one message of: its card
     *  carries the Draft badge, but it never goes away with the draft. */
    replyThreadId?: string
  },
  options: { throwOnError?: boolean; failureMessage?: string } = {},
): Promise<boolean> {
  if (!draft.accountId && !draft.threadId) return true
  forgetStandaloneRecoveryDraft(draft.draftMessageId, draft.accountId)

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const viewKey = currentListView
  const previousView = viewKey()
  const discardedDraftMessageId = normalizeMessageId(draft.draftMessageId)
  const withoutDiscardedDraft = (messages: Message[]) =>
    messages.filter((message) => {
      if (draft.messageId && message.id === draft.messageId) return false
      if (
        discardedDraftMessageId &&
        normalizeMessageId(message.message_id) === discardedDraftMessageId &&
        isDraftFolder(message.folder_id, message.account_id)
      ) {
        return false
      }
      return true
    })
  const nextMessages = withoutDiscardedDraft(previousMessages)
  const selectedThread = ui$.selectedThread.get()
  const removeThread = !!draft.threadId && !nextMessages.some((message) => message.thread_id === draft.threadId)
  const kanbanKeys = kanbanKeysWithThread(draft.threadId || (draft.replyThreadId ?? ''))

  mail$.messages.set(nextMessages)
  if (removeThread) {
    mail$.threads.set(previousThreads.filter((thread) => thread.thread_id !== draft.threadId))
    removeKanbanThread(draft.threadId)
    if (selectedThread === draft.threadId) {
      ui$.selectedThread.set('')
      requestThreadReselect()
    }
  } else {
    reconcileThreadDraftFromLoadedMessages(draft.threadId, nextMessages)
  }
  // The absence of a draft row proves nothing unless every message of the
  // conversation is here to say so: a thread that isn't open has none of them
  // loaded, and one opened past its first page is still missing its older ones,
  // where a second draft the badge stands for may well sit. Deriving the badge
  // from a partial view would clear it — and clear it again after the card
  // re-read below has correctly put it back. Short of the whole conversation,
  // that re-read is what settles it.
  const reconcileReplyThreadDraft = (loaded: Message[]) => {
    const replyThreadId = draft.replyThreadId ?? ''
    if (!replyThreadId || mail$.messagesCursor.get()) return
    if (!loaded.some((message) => message.thread_id === replyThreadId)) return
    reconcileThreadDraftFromLoadedMessages(replyThreadId, loaded)
  }
  reconcileReplyThreadDraft(nextMessages)

  try {
    if (draft.accountId && draft.draftMessageId) {
      await invoke('mail.discardDraft', {
        account_id: draft.accountId,
        draft_id: draft.draftMessageId,
        thread_id: draft.threadId,
      })
    } else {
      const res = await invoke('mail.delete', {
        thread_id: draft.threadId,
        message_ids: [draft.messageId],
        folder: draft.folderId,
      })
      if (!isDraftFolder(draft.folderId, draft.accountId)) assertDeleteAffected(res)
      applyMutationFolderUnreads(res as MutationResult)
    }
  } catch (error) {
    // Put back what this discard took and nothing else. The tab closes before
    // the discard runs, so the conversation may have changed since: a restored
    // compose tab has none loaded at all, and restoring the lists as they were
    // would blank the one that has loaded behind it.
    const openThread = ui$.selectedThread.get()
    const current = mail$.messages.get()
    const present = new Set([...nextMessages, ...current].map((message) => message.id))
    const missing = previousMessages.filter((message) => message.thread_id === openThread && !present.has(message.id))
    if (missing.length > 0) {
      const restored = [...current]
      for (const message of missing) {
        const later = restored.findIndex((item) => item.date > message.date)
        restored.splice(later === -1 ? restored.length : later, 0, message)
      }
      mail$.messages.set(restored)
    }
    if (viewKey() === previousView) {
      const cards = new Set([draft.threadId, draft.replyThreadId].filter(Boolean))
      const badges = new Map(
        previousThreads
          .filter((thread) => cards.has(thread.thread_id))
          .map((thread) => [thread.thread_id, thread.has_draft]),
      )
      const threads = removeThread
        ? restoreRemovedThread(mail$.threads.get(), previousThreads, draft.threadId)
        : mail$.threads.get()
      mail$.threads.set(
        threads.map((thread) =>
          badges.has(thread.thread_id) ? { ...thread, has_draft: badges.get(thread.thread_id) } : thread,
        ),
      )
    }
    if (options.throwOnError) throw error
    showToast(
      contextualErrorMessage(error, options.failureMessage ?? t('composer.status.couldNotDiscardDraft')),
      'error',
    )
    return false
  }
  // The draft is gone from here on, so a card reload that fails is not a failed
  // discard: the next refresh corrects the cards.
  try {
    await reloadThreadCards(kanbanKeys)
  } catch (error) {
    console.error('Reloading thread cards after a discard failed:', error)
  }
  // A mail.synced thread refresh can land while the server discard is on the
  // wire and reinsert the stale draft row after the optimistic removal above.
  // The discard has now succeeded, so remove that copy again before deriving
  // the conversation and thread-card draft state.
  const reconciledMessages = withoutDiscardedDraft(mail$.messages.get())
  if (reconciledMessages.length !== mail$.messages.get().length) mail$.messages.set(reconciledMessages)
  if (!removeThread) reconcileThreadDraftFromLoadedMessages(draft.threadId, reconciledMessages)
  reconcileReplyThreadDraft(reconciledMessages)
  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) void loadFolders(selectedAcc, false)
  if (draft.accountId && draft.accountId !== selectedAcc) void loadFolders(draft.accountId, false)
  return true
}

// Delete a single message (e.g. one draft) out of a thread, leaving the rest of
// the conversation intact. The message keeps its own folder_id so the delete
// targets the mailbox the message actually lives in, not the thread's folder.
// Drafts are discarded permanently (engine expunges them); other messages go to
// Trash. The confirm/toast wording reflects which.
/** Delete one message, after the user confirms. Resolves true once it is gone. */
export async function deleteMessage(message: Message): Promise<boolean> {
  if (!message?.id) return false

  // Local-only optimistic send (still sending, or failed): it has no
  // server-side copy under this id, so just drop it from the pane and forget
  // any pending retry payload — no backend round-trip, no confirm.
  if (isLocalSendId(message.id)) {
    discardPendingSend(message.id)
    // Also cancel any rescue keyed on it: without this the bubble the user
    // just deleted would be restored on the next thread load, and a rescue
    // still on the wire would leave its draft copy behind.
    cancelUnsentRescue(message.id)
    mail$.messages.set(mail$.messages.get().filter((item) => item.id !== message.id))
    return true
  }

  const isDraft = isDraftFolder(message.folder_id, message.account_id)
  const forever = !isDraft && deletesForever(message.account_id, message.folder_id)
  const confirmMessage = isDraft
    ? "Discard this draft? This can't be undone."
    : forever
      ? "This message will be permanently deleted. This can't be undone."
      : 'Move this message to Trash?'
  if (
    !(await confirmAction({
      title: isDraft ? 'Discard draft?' : forever ? 'Delete message forever?' : 'Move message to Trash?',
      message: confirmMessage,
      confirmLabel: isDraft ? 'Discard' : forever ? 'Delete forever' : 'Move to Trash',
      tone: 'danger',
    }))
  ) {
    return false
  }

  const threadId = message.thread_id
  const previousMessages = mail$.messages.get()
  const nextMessages = previousMessages.filter((item) => item.id !== message.id)
  const kanbanKeys = kanbanKeysWithThread(threadId)

  mail$.messages.set(nextMessages)

  try {
    const res = await invoke('mail.delete', {
      thread_id: threadId,
      message_ids: [message.id],
      folder: message.folder_id,
    })
    if (!isDraft) assertDeleteAffected(res)
    forgetStandaloneRecoveryDraft(message.message_id, message.account_id)
    applyMutationFolderUnreads(res as MutationResult)
    showToast(
      isDraft
        ? t('mail.toast.draftDiscarded')
        : forever
          ? t('mail.toast.deletedCount', { count: 1 })
          : t('mail.toast.messageMovedToTrash'),
    )
    // No messages left from this thread: drop the now-empty conversation.
    if (!nextMessages.some((item) => item.thread_id === threadId)) {
      if (ui$.selectedThread.get() === threadId) {
        ui$.selectedThread.set('')
        requestThreadReselect()
      }
      removeKanbanThread(threadId)
    }
    // The message is gone from here on, so a card reload that fails is not a
    // failed delete: the next refresh corrects the cards.
    try {
      await reloadThreadCards(kanbanKeys)
    } catch (error) {
      console.error('Reloading thread cards after a delete failed:', error)
    }
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) void loadFolders(selectedAcc, false)
    if (message.account_id && message.account_id !== selectedAcc) {
      void loadFolders(message.account_id, false)
    }
    return true
  } catch (error) {
    mail$.messages.set(previousMessages)
    showToast(
      deleteFailedMessage(error, {
        accountId: message.account_id,
        folderId: message.folder_id,
      }),
      'error',
    )
    return false
  }
}
