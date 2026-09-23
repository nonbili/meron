import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import './accounts'
import { mail$ } from './mail'
import { markUnreadWithUndo } from './mailFlags'
import { archiveThread, deleteThread, moveThreadToFolder } from './mailMoves'
import { ui$ } from './ui'

const THREAD = 'acc#INBOX#t.dG9waWM'

const thread = (overrides: Partial<Message> = {}): Message => ({
  id: `${THREAD}#7`,
  account_id: 'acc',
  folder_id: 'INBOX',
  thread_id: THREAD,
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Subject',
  preview: '',
  body: '',
  date: 1,
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

// Undo moves back exactly the copies the action reported (`target_uids`), so
// the undo's mail.move must carry them as message_ids — resolving the thread
// in the target instead would also take older mail of the conversation there,
// and finds nothing for a uid: key.
describe('undo after moving a thread', () => {
  let previousGo: unknown
  let calls: Array<{ command: string; payload: any }> = []
  let responses: Record<string, unknown> = {}

  beforeEach(() => {
    previousGo = (window as any).go
    calls = []
    responses = {}
    ui$.selectedAccount.set('acc')
    ui$.selectedFolder.set('INBOX')
    ui$.selectedThread.set('')
    ui$.toast.set('')
    ui$.toastUndo.set(null)
    mail$.threads.set([thread()])
    mail$.messages.set([])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            calls.push({ command, payload })
            if (command in responses) {
              const response = responses[command]
              if (response instanceof Error) throw response
              return response
            }
            return { ok: true }
          },
        },
      },
    }
  })

  afterEach(() => {
    ;(window as any).go = previousGo
  })

  async function runUndo() {
    const undo = ui$.toastUndo.get() as (() => void) | null
    expect(typeof undo).toBe('function')
    calls = []
    undo?.()
    await new Promise((resolve) => setTimeout(resolve, 0))
    return calls.find((call) => call.command === 'mail.move')?.payload
  }

  it('moves the reported Trash copies back out of Trash', async () => {
    responses['mail.delete'] = { deleted: 1, trash: 'Trash', thread_id: 'acc#Trash#t.dG9waWM', target_uids: [42, 43] }
    await deleteThread(THREAD)

    expect(await runUndo()).toEqual({
      thread_id: 'acc#Trash#t.dG9waWM',
      target_folder_id: 'INBOX',
      message_ids: ['42', '43'],
    })
  })

  // Without the exact copies there is no safe way back: the thread key in
  // Trash would also take older mail of the conversation.
  it('offers no undo when the copies are unknown', async () => {
    responses['mail.delete'] = { deleted: 1, trash: 'Trash', thread_id: 'acc#Trash#t.dG9waWM', target_uids: [] }
    await deleteThread(THREAD)
    expect(ui$.toastUndo.get()).toBe(null)
    expect(ui$.toast.get()).not.toBe('')

    mail$.threads.set([thread()])
    responses['mail.archive'] = { moved: 1, folder: 'Archive', thread_id: 'acc#Archive#t.dG9waWM' }
    await archiveThread(THREAD)
    expect(ui$.toastUndo.get()).toBe(null)

    mail$.threads.set([thread()])
    responses['mail.move'] = { moved: 1, target_uids: [] }
    await moveThreadToFolder(THREAD, 'Work')
    expect(ui$.toastUndo.get()).toBe(null)
  })

  it('moves the reported archive copies back', async () => {
    responses['mail.archive'] = { moved: 1, folder: 'Archive', thread_id: 'acc#Archive#t.dG9waWM', target_uids: [9] }
    await archiveThread(THREAD)

    expect(await runUndo()).toEqual({
      thread_id: 'acc#Archive#t.dG9waWM',
      target_folder_id: 'INBOX',
      message_ids: ['9'],
    })
  })

  it('moves the reported copies back from a folder', async () => {
    responses['mail.move'] = { moved: 1, target_uids: [5] }
    await moveThreadToFolder(THREAD, 'Work')

    expect(await runUndo()).toEqual({
      thread_id: 'acc#Work#t.dG9waWM',
      target_folder_id: 'INBOX',
      message_ids: ['5'],
    })
  })

  it('offers undo for mark unread only once it has landed', async () => {
    markUnreadWithUndo(THREAD)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(typeof ui$.toastUndo.get()).toBe('function')

    ui$.toastUndo.set(null)
    mail$.threads.set([thread()])
    responses['mail.markRead'] = new Error('server refused')
    markUnreadWithUndo(THREAD)
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(ui$.toastUndo.get()).toBe(null)
    expect(ui$.toast.get()).toBe('server refused')
    expect(ui$.toastTone.get()).toBe('error')
  })
})
