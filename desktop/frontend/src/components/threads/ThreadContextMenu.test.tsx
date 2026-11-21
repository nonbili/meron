import { expect, it, mock } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { mail$, releaseRemovedThread } from '../../states/mail'
import { settings$ } from '../../states/settings'
import type { Folder } from '../../types'
import { ThreadContextMenu, type ThreadContextMenuController } from './ThreadContextMenu'

it('moves to the thread account’s server-designated Junk folder even when a folder named Spam sorts first', async () => {
  const threadId = 'acc#INBOX#thread'
  const junkFolderId = '[Gmail]/Spam'
  const folder = (accountId: string, id: string, specialUse?: string): Folder => ({
    id,
    account_id: accountId,
    name: id,
    role: 'junk',
    special_use: specialUse,
    unread: 0,
  })
  const previousGo = (window as any).go
  const tasksEnabled = settings$.tasksEnabled.peek()
  const invoke = mock(async (command: string, _payload: unknown) => {
    if (command === 'mail.folderList') return { folders: [folder('acc', 'Spam'), folder('acc', junkFolderId, 'junk')] }
    if (command === 'mail.move') return { moved: 1 }
    return {}
  })
  ;(window as any).go = { main: { App: { Invoke: invoke } } }
  settings$.tasksEnabled.set(false)
  mail$.foldersByAccount.set({
    other: [folder('other', 'Junk', 'junk')],
  })
  const onAfterAction = mock(() => {})
  const controller: ThreadContextMenuController = {
    menu: {
      kind: 'mail',
      x: 0,
      y: 0,
      threadId,
      subject: 'Subject',
      accountId: 'acc',
      folderId: 'INBOX',
      unread: false,
      starred: false,
    },
    close: () => {},
    isOpen: () => true,
    open: () => {},
  }
  try {
    const view = render(
      <ThreadContextMenu controller={controller} onOpenThread={() => {}} onAfterAction={onAfterAction} />,
    )
    fireEvent.click(await view.findByRole('button', { name: 'Mark as spam' }))
    await waitFor(() =>
      expect(onAfterAction).toHaveBeenCalledWith('move', threadId, {
        targetAccountId: 'acc',
        targetFolderId: junkFolderId,
      }),
    )
    expect(invoke).toHaveBeenCalledWith('mail.move', { thread_id: threadId, target_folder_id: junkFolderId })
  } finally {
    cleanup()
    releaseRemovedThread(threadId)
    mail$.foldersByAccount.set({})
    settings$.tasksEnabled.set(tasksEnabled)
    ;(window as any).go = previousGo
  }
})
