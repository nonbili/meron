import type { Message } from '../types'
import { rememberThreadTransfer } from './recentThreadTransfers'
import { isLocalAccountId } from './accounts'
import { invoke } from '../lib/bridge'
import { contextualErrorMessage } from '../lib/errors'
import { t } from '../lib/i18n'
import { mailTransferError } from '../lib/mailTransferErrors'
import { clearBulkSelection, confirmAction, ui$, showToast, showUndoToast, type BulkSelectionItem } from './ui'
import { kanban$, kanbanKeyboardCardAfterRemoving, type KanbanCardRef } from './kanban'
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

function removeThreadLocally(threadId: string) {
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
    if (kanban$.activeBoardId.get()) {
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
    rollback: () => {
      unsuppress()
      // Put this thread back only. Restoring the whole list would resurrect
      // threads archived after this one started, which is what a fast burst
      // looks like when an earlier move fails.
      mail$.threads.set(restoreRemovedThread(mail$.threads.get(), previousThreads, threadId))
      const currentMessages = mail$.messages.get()
      const seenMessages = new Set(currentMessages.map((message) => message.id))
      const missingMessages = previousMessages.filter(
        (message) => message.thread_id === threadId && !seenMessages.has(message.id),
      )
      if (missingMessages.length > 0) mail$.messages.set([...currentMessages, ...missingMessages])
      for (const [key, columnThreads] of previousKanbanThreads) {
        if (!columnThreads) continue
        kanban$.threads[key].set(restoreRemovedThread(kanban$.threads[key].get() ?? [], columnThreads, threadId))
      }
      // The reader may already have archived the neighbour. Only come back
      // here when the selection is still the one this removal chose.
      if (ui$.selectedThread.get() === nextSelected) ui$.selectedThread.set(previousSelected)
      if (previousPaneThreadId === threadId && kanban$.paneThreadId.get() === nextSelected) {
        kanban$.paneThreadId.set(previousPaneThreadId)
      }
      if (keyboardCardLeaves && sameCard(kanban$.keyboardCard.get(), nextKeyboardCard)) {
        kanban$.keyboardCard.set(previousKeyboardCard)
      }
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
  if (refresh) await loadThreads(true)
  else await coalescedBackgroundListRefresh()
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

export async function bulkArchiveSelected(items: BulkSelectionItem[]) {
  // Local folders have no Archive folder.
  const targets = uniqueThreadItems(items).filter(
    (item) => !item.draft && !item.trash && !isLocalAccountId(item.accountId),
  )
  if (targets.length === 0) return
  const rollbacks: Array<() => void> = []
  let completed = 0
  try {
    for (const item of targets) {
      const sourceThread = findLocalThread(item.threadId)
      const removal = removeThreadLocally(item.threadId)
      rollbacks.push(removal.rollback)
      const res = await invoke('mail.archive', { thread_id: item.threadId })
      assertMoveAffected(res, 'Archive')
      removal.complete()
      rollbacks.pop()
      completed++
      applyMutationFolderUnreads(res as MutationResult)
      if (sourceThread?.account_id) void refreshAccountFoldersCache(sourceThread.account_id, false)
    }
    await refreshThreadLocation(undefined, false)
    clearBulkSelection()
    showToast(t('mail.toast.archivedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    const message = mailTransferError(error, t('mail.toast.archiveFailed'))
    showToast(
      completed ? `${message} ${t('mail.transferError.bulkProgress', { completed, total: targets.length })}` : message,
      'error',
    )
    if (completed) void refreshThreadLocation(undefined, false)
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
  const rollbacks: Array<() => void> = []
  let completed = 0
  try {
    for (const item of targets) {
      const removal = removeThreadLocally(item.threadId)
      rollbacks.push(removal.rollback)
      const res = await invoke('mail.move', {
        thread_id: item.threadId,
        target_folder_id: targetFolderId,
        ...(targetAccountId ? { target_account_id: targetAccountId } : {}),
      })
      assertMoveAffected(res)
      removal.complete()
      rollbacks.pop()
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
    clearBulkSelection()
    showToast(t('mail.toast.movedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    const message = mailTransferError(error, t('mail.toast.moveFailed'))
    showToast(
      completed ? `${message} ${t('mail.transferError.bulkProgress', { completed, total: targets.length })}` : message,
      'error',
    )
    if (completed) void refreshThreadLocation(undefined, false)
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

export async function bulkDeleteSelected(items: BulkSelectionItem[]) {
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

  const rollbacks: Array<() => void> = []
  try {
    for (const item of targets) {
      const removal = removeThreadLocally(item.threadId)
      rollbacks.push(removal.rollback)
      const res = await invoke('mail.delete', {
        thread_id: item.threadId,
        ...(item.folderId ? { folder: item.folderId } : {}),
      })
      assertDeleteAffected(res)
      removal.complete()
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(undefined, false)
    clearBulkSelection()
    showToast(t('mail.toast.deletedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
  }
}

export async function archiveThread(threadId: string) {
  if (!threadId) return
  const sourceThread = findLocalThread(threadId)
  if (isLocalAccountId(sourceThread?.account_id || threadAccountId(threadId))) return
  const sourceFolder = sourceThread?.folder_id ?? ''
  const { rollback, complete } = removeThreadLocally(threadId)
  try {
    const res = await invoke<{ folder?: string; thread_id?: string } & MovedCopiesResult & MutationResult>(
      'mail.archive',
      {
        thread_id: threadId,
      },
    )
    assertMoveAffected(res, 'Archive')
    complete()
    applyMutationFolderUnreads(res)
    const archivedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.folder)
    const copyIds = movedCopyIds(res)
    await refreshThreadLocation(sourceThread?.account_id, false)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.archiveFailedInSameFolder'), 'error')
    } else if (sourceFolder && copyIds) {
      // Offer to move it back where it came from. Falls back to a plain toast
      // when the origin folder or the archived copies are unknown (nothing
      // reliable to restore to, or from).
      showUndoToast(t('mail.toast.archivedCount', { count: 1 }), () => {
        releaseRemovedThread(threadId)
        void moveThreadToFolder(archivedThreadId, sourceFolder, { undo: false, messageIds: copyIds })
      })
    } else {
      showToast(t('mail.toast.archivedCount', { count: 1 }))
    }
  } catch (error) {
    rollback()
    showToast(mailTransferError(error, t('mail.toast.archiveFailed')), 'error')
  }
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

  const { rollback, complete } = removeThreadLocally(threadId)

  try {
    const res = await invoke<
      { deleted?: number; permanent?: boolean; trash?: string; thread_id?: string } & MovedCopiesResult & MutationResult
    >('mail.delete', {
      thread_id: threadId,
      ...(sourceFolder ? { folder: sourceFolder } : {}),
    })
    assertDeleteAffected(res)
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
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
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

  const previousThreads = mail$.threads.get()
  const previousMessages = mail$.messages.get()
  const viewKey = () =>
    threadListViewKey(ui$.selectedAccount.peek(), ui$.selectedFolder.peek(), ui$.query.peek(), listFilterKey())
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
    await reloadThreadCards(kanbanKeys)
    const selectedAcc = ui$.selectedAccount.get()
    if (selectedAcc) void loadFolders(selectedAcc, false)
    if (message.account_id && message.account_id !== selectedAcc) {
      void loadFolders(message.account_id, false)
    }
    return true
  } catch (error) {
    mail$.messages.set(previousMessages)
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
    return false
  }
}
