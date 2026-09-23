import type { Message } from '../types'
import { invoke } from '../lib/bridge'
import { t } from '../lib/i18n'
import { clearBulkSelection, confirmAction, ui$, showToast, showUndoToast, type BulkSelectionItem } from './ui'
import { kanban$ } from './kanban'
import { isLocalSendId, discardPendingSend, cancelUnsentRescue } from './pendingSends'
import {
  captureKeys,
  findLocalThread,
  getFilteredThreads,
  kanbanKeysWithThread,
  loadThreads,
  mail$,
  normalizeMessageId,
  reloadThreadCards,
  removeKanbanThread,
  requestThreadReselect,
  restoreKanbanColumns,
  uniqueThreadItems,
  updateKanbanThread,
} from './mail'
import {
  applyMutationFolderUnreads,
  isDraftFolder,
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

function threadIdInFolder(threadId: string, accountId: string | undefined, folderId: string | undefined): string {
  const lastHash = threadId.lastIndexOf('#')
  if (!accountId || !folderId || lastHash <= 0) return threadId
  return `${accountId}#${folderId}#${threadId.slice(lastHash + 1)}`
}

// Neighbour of a thread inside the kanban column that holds it (next, or
// previous if it was last). The chat-view getFilteredThreads list doesn't apply
// in kanban, where cards live in per-column lists.
function kanbanNeighbourThreadId(threadId: string): string {
  for (const threads of Object.values(kanban$.threads.get())) {
    const index = threads.findIndex((thread) => thread.thread_id === threadId)
    if (index === -1) continue
    const neighbour = threads[index + 1] ?? threads[index - 1]
    return neighbour?.thread_id ?? ''
  }
  return ''
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
      // can list the same thread once per folder it is starred in.
      const neighbour =
        index === -1
          ? undefined
          : (visible.slice(index + 1).find((item) => item.thread_id !== threadId) ??
            visible
              .slice(0, index)
              .reverse()
              .find((item) => item.thread_id !== threadId))
      nextSelected = neighbour?.thread_id ?? ''
    }
  }
  const nextThreads = previousThreads.filter((thread) => thread.thread_id !== threadId)

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

  return {
    rollback: () => {
      mail$.threads.set(previousThreads)
      mail$.messages.set(previousMessages)
      restoreKanbanColumns(previousKanbanThreads)
      ui$.selectedThread.set(previousSelected)
      if (previousPaneThreadId === threadId) {
        kanban$.paneThreadId.set(previousPaneThreadId)
      }
    },
  }
}

async function refreshThreadLocation(accountId?: string, refresh = false) {
  await loadThreads(refresh)
  const selectedAcc = ui$.selectedAccount.get()
  if (selectedAcc) {
    void loadFolders(selectedAcc, false)
  }
  if (accountId && accountId !== selectedAcc) {
    void loadFolders(accountId, false)
  }
}

export async function moveThreadToFolder(threadId: string, targetFolderId: string, options: { undo?: boolean } = {}) {
  if (!threadId || !targetFolderId) return
  const sourceThread = findLocalThread(threadId)
  const sourceFolder = sourceThread?.folder_id ?? ''
  if (sourceFolder === targetFolderId) return
  const targetThreadId = threadIdInFolder(threadId, sourceThread?.account_id, targetFolderId)

  const { rollback } = removeThreadLocally(threadId)
  try {
    const res = await invoke('mail.move', { thread_id: threadId, target_folder_id: targetFolderId })
    assertMoveAffected(res)
    applyMutationFolderUnreads(res as MutationResult)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.moveFailedInSameFolder'), 'error')
    } else if (options.undo !== false && sourceFolder) {
      showUndoToast(
        t('mail.toast.threadMoved'),
        () => void moveThreadToFolder(targetThreadId, sourceFolder, { undo: false }),
      )
    } else {
      showToast(t('mail.toast.threadMoved'))
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.moveFailed'), 'error')
  }
}

export async function copyThreadToFolder(threadId: string, targetAccountId: string, targetFolderId: string) {
  if (!threadId || !targetAccountId || !targetFolderId) return
  const sourceThread = findLocalThread(threadId)
  try {
    const res = await invoke('mail.copy', {
      thread_id: threadId,
      target_account_id: targetAccountId,
      target_folder_id: targetFolderId,
    })
    assertCopyAffected(res)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (targetAccountId !== sourceThread?.account_id) {
      void loadFolders(targetAccountId, false)
    }
    showToast(t('mail.toast.threadCopied'))
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('mail.toast.copyFailed'), 'error')
  }
}

export async function bulkArchiveSelected(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items).filter((item) => !item.draft && !item.trash)
  if (targets.length === 0) return
  const rollbacks: Array<() => void> = []
  try {
    for (const item of targets) {
      const sourceThread = findLocalThread(item.threadId)
      rollbacks.push(removeThreadLocally(item.threadId).rollback)
      const res = await invoke('mail.archive', { thread_id: item.threadId })
      assertMoveAffected(res, 'Archive')
      applyMutationFolderUnreads(res as MutationResult)
      if (sourceThread?.account_id) void refreshAccountFoldersCache(sourceThread.account_id, false)
    }
    await refreshThreadLocation(undefined, true)
    clearBulkSelection()
    showToast(t('mail.toast.archivedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.archiveFailed'), 'error')
  }
}

export async function bulkMoveSelectedToFolder(items: BulkSelectionItem[], targetFolderId: string) {
  const targets = uniqueThreadItems(items).filter((item) => item.folderId !== targetFolderId)
  if (targets.length === 0) return
  const rollbacks: Array<() => void> = []
  try {
    for (const item of targets) {
      rollbacks.push(removeThreadLocally(item.threadId).rollback)
      const res = await invoke('mail.move', { thread_id: item.threadId, target_folder_id: targetFolderId })
      assertMoveAffected(res)
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(targets[0]?.accountId, true)
    clearBulkSelection()
    showToast(t('mail.toast.movedCount', { count: targets.length }))
  } catch (error) {
    for (const rollback of rollbacks.reverse()) rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.moveFailed'), 'error')
  }
}

export async function bulkCopySelectedToFolder(
  items: BulkSelectionItem[],
  targetAccountId: string,
  targetFolderId: string,
) {
  const targets = uniqueThreadItems(items)
  if (targets.length === 0) return
  try {
    for (const item of targets) {
      const res = await invoke('mail.copy', {
        thread_id: item.threadId,
        target_account_id: targetAccountId,
        target_folder_id: targetFolderId,
      })
      assertCopyAffected(res)
    }
    await refreshThreadLocation(targets[0]?.accountId, true)
    if (targetAccountId !== targets[0]?.accountId) void loadFolders(targetAccountId, false)
    clearBulkSelection()
    showToast(t('mail.toast.copiedCount', { count: targets.length }))
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('mail.toast.copyFailed'), 'error')
  }
}

export async function bulkDeleteSelected(items: BulkSelectionItem[]) {
  const targets = uniqueThreadItems(items)
  if (targets.length === 0) return
  const permanentTargets = targets.filter(
    (item) => item.draft || item.trash || isTrashFolderId(item.accountId, item.folderId),
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
      rollbacks.push(removeThreadLocally(item.threadId).rollback)
      const res = await invoke('mail.delete', {
        thread_id: item.threadId,
        ...(item.folderId ? { folder: item.folderId } : {}),
      })
      assertDeleteAffected(res)
      applyMutationFolderUnreads(res as MutationResult)
    }
    await refreshThreadLocation(undefined, true)
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
  const sourceFolder = sourceThread?.folder_id ?? ''
  const { rollback } = removeThreadLocally(threadId)
  try {
    const res = await invoke<{ folder?: string; thread_id?: string } & MutationResult>('mail.archive', {
      thread_id: threadId,
    })
    assertMoveAffected(res, 'Archive')
    applyMutationFolderUnreads(res)
    const archivedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.folder)
    await refreshThreadLocation(sourceThread?.account_id, true)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.archiveFailedInSameFolder'), 'error')
    } else if (sourceFolder) {
      // Offer to move it back where it came from. Falls back to a plain toast
      // when the origin folder is unknown (nothing reliable to restore to).
      showUndoToast(
        t('mail.toast.archivedCount', { count: 1 }),
        () => void moveThreadToFolder(archivedThreadId, sourceFolder, { undo: false }),
      )
    } else {
      showToast(t('mail.toast.archivedCount', { count: 1 }))
    }
  } catch (error) {
    rollback()
    showToast(error instanceof Error ? error.message : t('mail.toast.archiveFailed'), 'error')
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
  const permanent = options.permanent ?? (isDraft || isTrash)
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

  const { rollback } = removeThreadLocally(threadId)

  try {
    const res = await invoke<
      { deleted?: number; permanent?: boolean; trash?: string; thread_id?: string } & MutationResult
    >('mail.delete', {
      thread_id: threadId,
      ...(sourceFolder ? { folder: sourceFolder } : {}),
    })
    assertDeleteAffected(res)
    applyMutationFolderUnreads(res)
    const trashedThreadId = res.thread_id ?? threadIdInFolder(threadId, sourceThread?.account_id, res.trash)
    const canUndoTrashMove = !!(res.thread_id || res.trash)
    await refreshThreadLocation(undefined, true)
    if (threadStillListed(threadId)) {
      showToast(t('mail.toast.deleteFailedInSameFolder'), 'error')
    } else if (!isDraft && !permanent && !res.permanent && sourceFolder && canUndoTrashMove) {
      showUndoToast(
        t('mail.toast.threadMovedToTrash'),
        () => void moveThreadToFolder(trashedThreadId, sourceFolder, { undo: false }),
      )
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
    await reloadThreadCards(kanbanKeys)
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
  } catch (error) {
    mail$.threads.set(previousThreads)
    mail$.messages.set(previousMessages)
    if (options.throwOnError) throw error
    const message = options.failureMessage ?? t('composer.status.couldNotDiscardDraft')
    showToast(error instanceof Error ? `${message}: ${error.message}` : message, 'error')
    return false
  }
}

// Delete a single message (e.g. one draft) out of a thread, leaving the rest of
// the conversation intact. The message keeps its own folder_id so the delete
// targets the mailbox the message actually lives in, not the thread's folder.
// Drafts are discarded permanently (engine expunges them); other messages go to
// Trash. The confirm/toast wording reflects which.
export async function deleteMessage(message: Message) {
  if (!message?.id) return

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
    return
  }

  const isDraft = isDraftFolder(message.folder_id, message.account_id)
  const confirmMessage = isDraft ? "Discard this draft? This can't be undone." : 'Move this message to Trash?'
  if (
    !(await confirmAction({
      title: isDraft ? 'Discard draft?' : 'Move message to Trash?',
      message: confirmMessage,
      confirmLabel: isDraft ? 'Discard' : 'Move to Trash',
      tone: 'danger',
    }))
  ) {
    return
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
    showToast(isDraft ? t('mail.toast.draftDiscarded') : t('mail.toast.messageMovedToTrash'))
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
  } catch (error) {
    mail$.messages.set(previousMessages)
    showToast(error instanceof Error ? error.message : t('mail.toast.deleteFailed'), 'error')
  }
}
