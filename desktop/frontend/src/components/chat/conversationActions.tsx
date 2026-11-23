import { useState } from 'react'
import { Archive, Download, ExternalLink, ListPlus, Printer, ShieldAlert, Star, Trash2 } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { printThread } from '../../lib/printMail'
import { openThreadWindow } from '../../lib/threadWindow'
import { threadWindowSession } from '../../lib/windowSession'
import { isRssAccount } from '../../lib/threadActions'
import { useTranslation } from '../../lib/i18n'
import { accounts$, isLocalAccountId } from '../../states/accounts'
import { exportThreadsAsEml } from '../../states/emlExport'
import { starThread } from '../../states/mailFlags'
import { deletesForever, isDraftFolder } from '../../states/mailFolders'
import { archiveThread, deleteThread, moveThreadToFolder } from '../../states/mailMoves'
import { mail$ } from '../../states/mail'
import { accountFolderForRole } from '../../lib/unifiedFolders'
import { settings$ } from '../../states/settings'
import type { ConversationToolbarAction } from '../../states/settings'
import type { Message } from '../../types'

/** What Settings shows for each action that can be pinned to the toolbar. */
export const CONVERSATION_ACTION_META: Record<ConversationToolbarAction, { icon: LucideIcon; labelKey: string }> = {
  archive: { icon: Archive, labelKey: 'threads.actions.archiveThread' },
  spam: { icon: ShieldAlert, labelKey: 'threads.actions.markAsSpam' },
  delete: { icon: Trash2, labelKey: 'buttons.delete' },
  tasks: { icon: ListPlus, labelKey: 'tasks.addFromMessage' },
  star: { icon: Star, labelKey: 'chat.star' },
  print: { icon: Printer, labelKey: 'chat.actions.printThread' },
  saveEml: { icon: Download, labelKey: 'chat.actions.saveAsEml' },
  openInNewWindow: { icon: ExternalLink, labelKey: 'threads.actions.openInNewWindow' },
}

export type ConversationAction = {
  icon: LucideIcon
  label: string
  /** Whether the open conversation offers it at all (RSS items, local
   *  conversations and settings rule some out). */
  available: boolean
  run: () => void
  danger?: boolean
  disabled?: boolean
  /** On, for a toggle: a starred conversation. */
  active?: boolean
}

/**
 * The open conversation's thread actions, defined once for every place that
 * offers them: the toolbar's pinned buttons and the ⋯ / right-click menu. Add
 * to tasks has no menu entry; the toolbar renders it as AddToTasksButton, which
 * picks a list, so its `run` is never called.
 */
export function useConversationActions(
  thread: Message,
  scope: string,
): { actions: Record<ConversationToolbarAction, ConversationAction>; printing: boolean } {
  const { t } = useTranslation()
  const account = useValue(accounts$).find((acc) => acc.id === thread.account_id)
  const isRSS = isRssAccount(account, thread.account_id)
  const isLocal = isLocalAccountId(thread.account_id)
  const tasksEnabled = useValue(settings$.tasksEnabled)
  const [printing, setPrinting] = useState(false)

  // The same wording, and the same permanence in Trash, as the thread list's menu.
  const inTrash = deletesForever(thread.account_id, thread.folder_id)
  const inDrafts = isDraftFolder(thread.folder_id, thread.account_id)
  // The account's Junk folder, as the thread list's menu finds it: the one the
  // server designates, else the one named for it.
  const junkFolderId = useValue(() => {
    if (isLocal) return undefined
    const folders = mail$.foldersByAccount[thread.account_id].get()
    return folders?.find((folder) => folder.special_use === 'junk')?.id ?? accountFolderForRole(folders, 'junk')
  })

  const actions: Record<ConversationToolbarAction, ConversationAction> = {
    archive: {
      icon: Archive,
      label: t('threads.actions.archiveThread'),
      available: !isRSS && !isLocal,
      run: () => void archiveThread(thread.thread_id),
    },
    spam: {
      icon: ShieldAlert,
      label: t('threads.actions.markAsSpam'),
      available: !isRSS && !!junkFolderId && junkFolderId !== thread.folder_id && !inDrafts,
      run: () => {
        if (junkFolderId) void moveThreadToFolder(thread.thread_id, junkFolderId)
      },
    },
    delete: {
      icon: Trash2,
      label: inTrash
        ? t('threads.actions.deleteForever')
        : inDrafts
          ? t('threads.actions.discardDraft')
          : t('threads.actions.moveToTrash'),
      available: !isRSS,
      danger: true,
      run: () => void deleteThread(thread.thread_id, { permanent: inTrash }),
    },
    tasks: {
      icon: ListPlus,
      label: t('tasks.addFromMessage'),
      available: tasksEnabled && !isRSS,
      run: () => undefined,
    },
    star: {
      icon: Star,
      label: thread.starred ? t('chat.unstar') : t('chat.star'),
      available: true,
      active: !!thread.starred,
      run: () => void starThread(thread.thread_id, !thread.starred),
    },
    print: {
      icon: Printer,
      label: t('chat.actions.printThread'),
      available: !isRSS,
      disabled: printing,
      run: () => {
        setPrinting(true)
        void printThread(thread.thread_id).finally(() => setPrinting(false))
      },
    },
    saveEml: {
      icon: Download,
      label: t('chat.actions.saveAsEml'),
      available: !isRSS,
      run: () =>
        void exportThreadsAsEml([{ threadId: thread.thread_id, folderId: thread.folder_id }], thread.subject || ''),
    },
    openInNewWindow: {
      icon: ExternalLink,
      label: t('threads.actions.openInNewWindow'),
      // Already the conversation's own window.
      available: !threadWindowSession,
      run: () => void openThreadWindow(thread.thread_id, thread, scope),
    },
  }
  return { actions, printing }
}
