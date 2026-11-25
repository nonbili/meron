import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { kanban$ } from './kanban'
import { mail$ } from './mail'
import { bulkMarkSelectedUnread } from './mailFlags'
import { archiveThread, bulkArchiveSelected, bulkDeleteSelected } from './mailMoves'
import { runToastUndo, settleConfirm, toggleBulkSelection, ui$, type BulkSelectionItem } from './ui'

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

describe('archiveThread undo', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.archive': [{ ok: true, moved: 1, folder: 'Archive', thread_id: 'acc#Archive#t.MQ', target_uids: [9] }],
      'mail.threadList': [{ threads: [] }, { threads: [thread({ thread_id: 'acc#inbox#t.MQ' })] }],
      'mail.move': [{ ok: true, moved: 1 }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Archive', account_id: 'acc', name: 'Archive', role: 'archive', unread: 0 },
    ])
    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Archive', account_id: 'acc', name: 'Archive', role: 'archive', unread: 0 },
      ],
    })
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('moves the archived copy back to the original folder', async () => {
    await archiveThread('acc#inbox#t.MQ')

    expect(mail$.threads.get()).toHaveLength(0)
    expect(ui$.toast.get()).toBe('Thread archived')
    expect(ui$.toastUndo.peek()).toBeTruthy()

    runToastUndo()
    await nextTick()

    expect(calls.find((call) => call.command === 'mail.move')?.payload).toMatchObject({
      thread_id: 'acc#Archive#t.MQ',
      target_folder_id: 'inbox',
      message_ids: ['9'],
    })
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['acc#inbox#t.MQ'])
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Thread moved')
  })

  it('rolls back and reports an error when archive moves no messages', async () => {
    responses['mail.archive'] = [{ ok: true, moved: 0, folder: 'Archive', thread_id: 'acc#Archive#t.MQ' }]
    responses['mail.threadList'] = []

    await archiveThread('acc#inbox#t.MQ')

    expect(calls.some((call) => call.command === 'mail.threadList')).toBe(false)
    expect(mail$.threads.get()).toHaveLength(1)
    expect(ui$.selectedThread.get()).toBe('acc#inbox#t.MQ')
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('Archive failed: no matching messages found')
  })

  // Gmail archives to All Mail, which lists Inbox mail too: archiving there
  // takes the Inbox copies out and leaves the thread listed where it is. Real
  // Gmail reports no target_uids (no Undo); they stand in for a plain IMAP
  // Archive folder, which does report them.
  it('archives a thread viewed in the archive folder and undoes into the Inbox', async () => {
    const allMailThread = 'acc#[Gmail]/All Mail#t.MQ'
    mail$.threads.set([thread({ thread_id: allMailThread, folder_id: '[Gmail]/All Mail' })])
    ui$.selectedThread.set(allMailThread)
    ui$.selectedFolder.set('[Gmail]/All Mail')
    responses['mail.archive'] = [
      {
        ok: true,
        moved: 1,
        folder: '[Gmail]/All Mail',
        source_folder: 'INBOX',
        thread_id: allMailThread,
        target_uids: [9],
      },
    ]
    responses['mail.move'] = [{ ok: true, moved: 1, source_folder: '[Gmail]/All Mail', target_uids: [4] }]
    responses['mail.threadList'] = [
      { threads: [thread({ thread_id: allMailThread, folder_id: '[Gmail]/All Mail' })] },
      { threads: [thread({ thread_id: allMailThread, folder_id: '[Gmail]/All Mail' })] },
    ]

    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: '[Gmail]/All Mail', account_id: 'acc', name: 'All Mail', role: 'archive', unread: 0 },
      ],
    })
    // The card stays put the whole time: it is still in All Mail afterwards,
    // so taking it off and back on would only make it blink.
    const invoke = (window as any).go.main.App.Invoke
    const listedDuringArchive: string[][] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command === 'mail.archive' || command === 'mail.move') {
        listedDuringArchive.push(mail$.threads.get().map((item) => item.thread_id))
      }
      return invoke(command, payload)
    }

    await archiveThread(allMailThread)

    expect(listedDuringArchive).toEqual([[allMailThread]])
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([allMailThread])
    expect(ui$.selectedThread.get()).toBe(allMailThread)
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Thread archived')

    runToastUndo()
    await nextTick()

    expect(calls.find((call) => call.command === 'mail.move')?.payload).toMatchObject({
      thread_id: allMailThread,
      target_folder_id: 'INBOX',
      message_ids: ['9'],
    })
    await nextTick()
    await nextTick()
    // Undo leaves the card listed too: it's still in All Mail, which is not a failure.
    expect(listedDuringArchive).toEqual([[allMailThread], [allMailThread]])
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([allMailThread])
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Thread moved')
  })

  // A Gmail label named "Archive" looks like the archive folder by name, but the
  // account archives to All Mail: the thread leaves, so its card goes too.
  it('takes the card off when an archive-named folder is not the archive target', async () => {
    const labelThread = 'acc#Archive#t.MQ'
    mail$.threads.set([thread({ thread_id: labelThread, folder_id: 'Archive' })])
    ui$.selectedThread.set(labelThread)
    responses['mail.archive'] = [
      { ok: true, moved: 1, folder: '[Gmail]/All Mail', thread_id: 'acc#[Gmail]/All Mail#t.MQ', target_uids: [9] },
    ]
    responses['mail.threadList'] = [{ threads: [] }]
    // Off before the list reload, not only because of it.
    const invoke = (window as any).go.main.App.Invoke
    const listedAtReload: string[][] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command === 'mail.threadList') listedAtReload.push(mail$.threads.get().map((item) => item.thread_id))
      return invoke(command, payload)
    }

    await archiveThread(labelThread)

    expect(listedAtReload[0]).toEqual([])
    expect(mail$.threads.get()).toHaveLength(0)
    expect(ui$.toast.get()).toBe('Thread archived')
  })

  // A localized Archive folder whose metadata isn't loaded yet looks like any
  // other folder, so the card leaves at first; the core archived in place, so it
  // comes back with its selection.
  it('puts the card back when an unrecognised folder turns out to be the archive', async () => {
    const archivio = 'acc#Archivio#t.MQ'
    mail$.foldersByAccount.set({})
    mail$.threads.set([thread({ thread_id: archivio, folder_id: 'Archivio' })])
    ui$.selectedThread.set(archivio)
    responses['mail.archive'] = [
      { ok: true, moved: 1, folder: 'Archivio', source_folder: 'INBOX', thread_id: archivio, target_uids: [9] },
    ]
    responses['mail.threadList'] = [{ threads: [thread({ thread_id: archivio, folder_id: 'Archivio' })] }]
    // Back before the list reload, not only because of it.
    const invoke = (window as any).go.main.App.Invoke
    const listedAtReload: string[][] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command === 'mail.threadList') listedAtReload.push(mail$.threads.get().map((item) => item.thread_id))
      return invoke(command, payload)
    }

    await archiveThread(archivio)

    expect(listedAtReload[0]).toEqual([archivio])
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([archivio])
    expect(ui$.selectedThread.get()).toBe(archivio)
    expect(ui$.toast.get()).toBe('Thread archived')
  })

  // The copies left the Inbox: a card for the Inbox copy (a Kanban Inbox
  // column, say) goes, while the All Mail one stays.
  it('takes the Inbox copy card off after archiving in the archive folder', async () => {
    const allMailThread = 'acc#[Gmail]/All Mail#t.MQ'
    const inboxThread = 'acc#INBOX#t.MQ'
    mail$.foldersByAccount.set({
      acc: [{ id: '[Gmail]/All Mail', account_id: 'acc', name: 'All Mail', role: 'archive', unread: 0 }],
    })
    mail$.threads.set([
      thread({ thread_id: allMailThread, folder_id: '[Gmail]/All Mail' }),
      thread({ id: 'm2', thread_id: inboxThread, folder_id: 'INBOX' }),
    ])
    ui$.selectedThread.set(allMailThread)
    responses['mail.archive'] = [
      { ok: true, moved: 1, folder: '[Gmail]/All Mail', source_folder: 'INBOX', thread_id: allMailThread },
    ]
    responses['mail.threadList'] = [{ threads: [thread({ thread_id: allMailThread, folder_id: '[Gmail]/All Mail' })] }]
    const invoke = (window as any).go.main.App.Invoke
    const listedAtReload: string[][] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command === 'mail.threadList') listedAtReload.push(mail$.threads.get().map((item) => item.thread_id))
      return invoke(command, payload)
    }

    await archiveThread(allMailThread)

    expect(listedAtReload[0]).toEqual([allMailThread])
    expect(ui$.selectedThread.get()).toBe(allMailThread)
  })

  it('says a thread with no Inbox copy is already archived', async () => {
    const allMailThread = 'acc#[Gmail]/All Mail#t.MQ'
    mail$.threads.set([thread({ thread_id: allMailThread, folder_id: '[Gmail]/All Mail' })])
    ui$.selectedThread.set(allMailThread)
    responses['mail.archive'] = [
      { ok: true, moved: 0, already_archived: true, folder: '[Gmail]/All Mail', thread_id: allMailThread },
    ]

    await archiveThread(allMailThread)

    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([allMailThread])
    expect(ui$.selectedThread.get()).toBe(allMailThread)
    expect(ui$.toastTone.get()).toBe('success')
    expect(ui$.toast.get()).toBe('Already archived')
  })
})

describe('bulk thread actions', () => {
  const calls: { command: string; payload: unknown }[] = []
  let responses: Record<string, unknown[]> = {}

  beforeEach(() => {
    calls.length = 0
    responses = {
      'mail.markRead': [{ ok: true }],
      'mail.archive': [{ ok: true, moved: 1 }],
      'mail.delete': [{ ok: true, deleted: 1 }],
      'mail.threadList': [{ threads: [] }],
      'mail.folderList': [{ folders: [] }],
    }
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' })])
    mail$.messages.set([])
    mail$.folders.set([
      { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
      { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
    ])
    mail$.foldersByAccount.set({
      acc: [
        { id: 'inbox', account_id: 'acc', name: 'Inbox', role: 'inbox', unread: 0 },
        { id: 'Trash', account_id: 'acc', name: 'Trash', role: 'trash', unread: 0 },
      ],
    })
    kanban$.threads.set({})
    ui$.selectedThread.set('acc#inbox#t.MQ')
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('inbox')
    ui$.bulkSelection.set({})
    ui$.bulkAnchorKey.set('')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ui$.toastUndo.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return responses[command]?.shift() ?? {}
          },
        },
      },
    }
  })

  it('dedupes bulk unread by thread id', async () => {
    const first = thread({ id: 'm1', thread_id: 'acc#inbox#t.MQ', unread: false })
    const second = thread({ id: 'm2', thread_id: 'acc#inbox#t.MQ', unread: false })
    mail$.threads.set([first, second])

    await bulkMarkSelectedUnread([bulkItem(first), bulkItem(second)])

    expect(calls.filter((call) => call.command === 'mail.markRead').map((call) => call.payload)).toEqual([
      { thread_id: 'acc#inbox#t.MQ', seen: false },
    ])
    expect(ui$.toast.get()).toBe('Marked unread')
  })

  it('keeps completed bulk archives removed and rolls back the failed thread', async () => {
    const second = thread({ id: 'm2', thread_id: 'acc#inbox#t.NQ' })
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' }), second])
    responses['mail.archive'] = [
      { ok: true, moved: 1 },
      { ok: true, moved: 0 },
    ]
    responses['mail.threadList'] = []

    await bulkArchiveSelected(mail$.threads.get().map((item) => bulkItem(item)))

    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['acc#inbox#t.NQ'])
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe(
      'Archive failed: no matching messages found Completed 1 of 2 selected threads before the failure. Check the destination before retrying.',
    )
  })

  it('leaves already archived threads listed in a bulk archive', async () => {
    const allMail = thread({ id: 'm2', thread_id: 'acc#[Gmail]/All Mail#t.NQ', folder_id: '[Gmail]/All Mail' })
    mail$.foldersByAccount.set({
      acc: [{ id: '[Gmail]/All Mail', account_id: 'acc', name: 'All Mail', role: 'archive', unread: 0 }],
    })
    mail$.threads.set([thread({ thread_id: 'acc#inbox#t.MQ' }), allMail])
    responses['mail.archive'] = [
      { ok: true, moved: 1 },
      { ok: true, moved: 0, already_archived: true },
    ]
    // The server still lists the All Mail thread after the reload.
    responses['mail.threadList'] = [{ threads: [allMail] }]

    const invoke = (window as any).go.main.App.Invoke
    const listedDuringArchive: string[][] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command === 'mail.archive') listedDuringArchive.push(mail$.threads.get().map((item) => item.thread_id))
      return invoke(command, payload)
    }

    await bulkArchiveSelected(mail$.threads.get().map((item) => bulkItem(item)))

    // The Inbox card leaves at once; the All Mail one never does.
    expect(listedDuringArchive).toEqual([['acc#[Gmail]/All Mail#t.NQ'], ['acc#[Gmail]/All Mail#t.NQ']])
    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual(['acc#[Gmail]/All Mail#t.NQ'])
    expect(ui$.toastTone.get()).toBe('success')
  })

  it('says already archived when no thread in a bulk archive moved', async () => {
    const allMail = thread({ thread_id: 'acc#[Gmail]/All Mail#t.NQ', folder_id: '[Gmail]/All Mail' })
    mail$.threads.set([allMail])
    responses['mail.archive'] = [{ ok: true, moved: 0, already_archived: true }]
    responses['mail.threadList'] = [{ threads: [allMail] }]

    await bulkArchiveSelected([bulkItem(allMail)])

    expect(mail$.threads.get().map((item) => item.thread_id)).toEqual([allMail.thread_id])
    expect(ui$.toast.get()).toBe('Already archived')
  })

  it('takes an archive-named label card off in a bulk archive', async () => {
    const label = thread({ thread_id: 'acc#Archive#t.MQ', folder_id: 'Archive' })
    mail$.threads.set([label])
    responses['mail.archive'] = [{ ok: true, moved: 1, thread_id: 'acc#[Gmail]/All Mail#t.MQ' }]
    const invoke = (window as any).go.main.App.Invoke
    const listedAtReload: string[][] = []
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      if (command === 'mail.threadList') listedAtReload.push(mail$.threads.get().map((item) => item.thread_id))
      return invoke(command, payload)
    }

    await bulkArchiveSelected([bulkItem(label)])

    expect(listedAtReload[0]).toEqual([])
    expect(ui$.toast.get()).toBe('Thread archived')
  })

  it('uses one confirmation for permanent bulk delete', async () => {
    const trash = thread({ id: 'trash', folder_id: 'Trash', thread_id: 'acc#Trash#t.MQ' })
    mail$.threads.set([trash])
    ui$.selectedFolder.set('Trash')
    responses['mail.delete'] = [{ ok: true, deleted: 1, permanent: true }]
    responses['mail.threadList'] = [{ threads: [] }]

    const pending = bulkDeleteSelected([bulkItem(trash, { trash: true })])
    settleConfirm(true)
    await pending

    expect(calls.filter((call) => call.command === 'mail.delete')).toHaveLength(1)
    expect(calls.find((call) => call.command === 'mail.delete')?.payload).toMatchObject({
      thread_id: 'acc#Trash#t.MQ',
      folder: 'Trash',
    })
    expect(ui$.toast.get()).toBe('Thread deleted')
  })

  it('replaces bulk selection when selecting a different group', () => {
    const first = thread({ id: 'm1', thread_id: 'acc#inbox#t.MQ' })
    const second = thread({ id: 'm2', thread_id: 'acc#work#t.NQ', folder_id: 'Work' })

    toggleBulkSelection(bulkItem(first, { groupKey: 'kanban:inbox' }))
    toggleBulkSelection(bulkItem(second, { groupKey: 'kanban:work' }))

    expect(Object.keys(ui$.bulkSelection.get())).toEqual(['test:m2'])
    expect(ui$.bulkSelection['test:m2'].get()?.groupKey).toBe('kanban:work')
  })
})
