import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import {
  activateConversationTab,
  closeMessageTab,
  compose$,
  discardQuickReplyDraftIfEmpty,
  isQuickReplyBlank,
  draftShouldOpenConversation,
  openDraftCompose,
  openDraftConversationOrCompose,
  openMessageTab,
  revealMessageRemote,
  openReplyInFullEditor,
  openComposeTab,
  openThreadTab,
  updateComposeDraft,
  openThreadTabById,
  quickReplyCaretOffset,
  quickReplyFromState,
  pickReplyTarget,
  resolveQuickReplyFrom,
  saveQuickReplyDraft,
  seedQuickReplySignature,
  sendReply,
  withoutHydratedQuickReplyDraft,
} from './compose'
import { accounts$ } from './accounts'
import { settings$ } from './settings'
import { ui$ } from './ui'
import { mail$ } from './mail'
import { resetThreadView, thread$ } from './thread'

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'm1',
  account_id: 'acc-notification',
  folder_id: 'inbox',
  thread_id: 't-notification',
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Notification thread',
  preview: '',
  body: '',
  date: Math.floor(Date.parse('2026-06-11T12:00:00Z') / 1000),
  unread: true,
  starred: false,
  has_attachments: false,
  ...overrides,
})

describe('openThreadTabById', () => {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.conversationThread.set('')
    mail$.messages.set([])
    mail$.messagesCursor.set('')
    mail$.messagesLoadingMore.set(false)
    mail$.threadLoading.set(false)
    accounts$.set([
      {
        id: 'acc-notification',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
      },
    ])
    ui$.selectedAccount.set('acc-current')
    ui$.selectedFolder.set('work')
    ui$.selectedThread.set('t-current')
    ui$.query.set('needle')
    ui$.mobilePane.set('threads')
    ui$.toast.set('')
    ui$.toastTone.set('success')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.threadRead') {
              return { messages: [message()] }
            }
            return {}
          },
        },
      },
    }
  })

  it('opens a fetched thread tab without changing side navigation selection state', async () => {
    await openThreadTabById('t-notification')

    expect(compose$.activeTab.get()).toBe('thread-t-notification')
    expect(compose$.tabs.get()).toHaveLength(1)
    expect(compose$.tabs.get()[0]).toMatchObject({
      kind: 'thread',
      threadId: 't-notification',
      accountId: 'acc-notification',
      folderId: 'inbox',
      subject: 'Notification thread',
    })
    expect(ui$.selectedThread.get()).toBe('t-notification')
    expect(ui$.mobilePane.get()).toBe('conversation')
    expect(ui$.selectedAccount.get()).toBe('acc-current')
    expect(ui$.selectedFolder.get()).toBe('work')
    expect(ui$.query.get()).toBe('needle')
    expect(calls.filter((call) => call.command === 'mail.threadRead')).toHaveLength(1)
  })

  it('activates an existing thread tab without duplicating or fetching', async () => {
    openThreadTab(message())
    ui$.selectedAccount.set('acc-current')
    ui$.selectedFolder.set('work')
    ui$.selectedThread.set('t-current')
    calls.length = 0

    await openThreadTabById('t-notification')

    expect(compose$.activeTab.get()).toBe('thread-t-notification')
    expect(compose$.tabs.get()).toHaveLength(1)
    expect(ui$.selectedThread.get()).toBe('t-notification')
    expect(ui$.selectedAccount.get()).toBe('acc-current')
    expect(ui$.selectedFolder.get()).toBe('work')
    expect(calls.some((call) => call.command === 'mail.threadRead')).toBe(false)
  })

  it('preserves navigation state and shows an error when the thread cannot load', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') throw new Error('backend unavailable')
      return {}
    }

    await openThreadTabById('t-missing')

    expect(compose$.tabs.get()).toHaveLength(0)
    expect(compose$.activeTab.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('t-current')
    expect(ui$.selectedAccount.get()).toBe('acc-current')
    expect(ui$.selectedFolder.get()).toBe('work')
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toBe('backend unavailable')
  })

  it('opens a Drafts row as an editable compose tab', async () => {
    const draft = message({
      id: 'acc-notification#Drafts#42#99',
      folder_id: 'Drafts',
      thread_id: 'acc-notification#Drafts#42',
      from_addr: 'me@example.com',
      to: 'you@example.com',
      cc: 'copy@example.com',
      bcc: 'blind@example.com',
      message_id: 'draft-id@example.com',
      subject: 'Draft subject',
      body: 'saved body',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [draft] }
      return {}
    }

    const handled = await openDraftCompose(draft)

    expect(handled).toBe(true)
    expect(compose$.tabs.get()).toHaveLength(1)
    expect(compose$.activeTab.get()).toBe(compose$.tabs.get()[0].id)
    expect(compose$.tabs.get()[0]).toMatchObject({
      kind: 'compose',
      subject: 'Draft subject',
      threadId: '',
      compose: {
        accountId: 'acc-notification',
        fromEmail: 'me@example.com',
        to: 'you@example.com',
        cc: 'copy@example.com',
        bcc: 'blind@example.com',
        text: 'saved body',
        draftMessageId: 'draft-id@example.com',
        sourceDraft: {
          threadId: 'acc-notification#Drafts#42',
          messageId: 'acc-notification#Drafts#42#99',
          folderId: 'Drafts',
        },
      },
    })
  })

  it('reuses the open compose tab for repeated Drafts row clicks', async () => {
    const draft = message({
      id: 'acc-notification#Drafts#42#99',
      folder_id: 'Drafts',
      thread_id: 'acc-notification#Drafts#42',
      from_addr: 'me@example.com',
      to: 'you@example.com',
      message_id: 'draft-id@example.com',
      subject: 'Draft subject',
      body: 'saved body',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [draft] }
      return {}
    }

    await openDraftCompose(draft)
    const firstTabId = compose$.activeTab.get()
    await openDraftCompose(draft)

    expect(compose$.tabs.get()).toHaveLength(1)
    expect(compose$.activeTab.get()).toBe(firstTabId)
    expect(calls.filter((call) => call.command === 'mail.threadRead')).toHaveLength(1)
  })

  it('opens a Drafts row with a cached ancestor in the conversation view', async () => {
    const ancestor = message({
      id: 'acc-notification#INBOX#42#1',
      folder_id: 'INBOX',
      thread_id: 'acc-notification#Drafts#42',
      from_addr: 'you@example.com',
      message_id: 'root@example.com',
      subject: 'Thread subject',
    })
    const draft = message({
      id: 'acc-notification#Drafts#42#99',
      folder_id: 'Drafts',
      thread_id: 'acc-notification#Drafts#42',
      from_addr: 'me@example.com',
      to: 'you@example.com',
      subject: 'Re: Thread subject',
      body: 'draft reply',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [ancestor, draft] }
      return {}
    }

    const handled = await openDraftConversationOrCompose(draft)

    expect(handled).toBe(true)
    expect(compose$.tabs.get()).toHaveLength(0)
    expect(compose$.activeTab.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('acc-notification#Drafts#42')
    expect(ui$.mobilePane.get()).toBe('conversation')
    expect(mail$.messages.get()).toEqual([ancestor, draft])
  })

  it('opens a referenced Drafts row in the conversation view even without cached ancestors', async () => {
    const draft = message({
      id: 'acc-notification#Drafts#42#99',
      folder_id: 'Drafts',
      thread_id: 'acc-notification#Drafts#42',
      from_addr: 'me@example.com',
      to: 'you@example.com',
      subject: 'Re: Thread subject',
      references: 'root@example.com',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [draft] }
      return {}
    }

    await openDraftConversationOrCompose(draft)

    expect(compose$.tabs.get()).toHaveLength(0)
    expect(ui$.selectedThread.get()).toBe('acc-notification#Drafts#42')
    expect(mail$.messages.get()).toEqual([draft])
  })

  it('classifies standalone Drafts rows as composer-only', () => {
    const draft = message({ folder_id: 'Drafts', references: '' })

    expect(draftShouldOpenConversation([draft], draft)).toBe(false)
  })

  it('opens a standalone Drafts row through the shared helper as an editable compose tab', async () => {
    const draft = message({
      id: 'acc-notification#Drafts#42#99',
      folder_id: 'Drafts',
      thread_id: 'acc-notification#Drafts#42',
      from_addr: 'me@example.com',
      to: 'you@example.com',
      message_id: 'draft-id@example.com',
      subject: 'Draft subject',
      body: 'saved body',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [draft] }
      return {}
    }

    await openDraftConversationOrCompose(draft)

    expect(compose$.tabs.get()).toHaveLength(1)
    expect(compose$.activeTab.get()).toBe(compose$.tabs.get()[0].id)
    expect(compose$.tabs.get()[0]).toMatchObject({
      kind: 'compose',
      subject: 'Draft subject',
      compose: {
        to: 'you@example.com',
        text: 'saved body',
        draftMessageId: 'draft-id@example.com',
      },
    })
  })
})

describe('tab navigation', () => {
  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.conversationThread.set('')
    ui$.selectedThread.set('')
    accounts$.set([
      {
        id: 'acc-notification',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
      },
    ])
  })

  it('remembers the Current conversation while a thread tab is active', () => {
    // Selecting a thread in the list mirrors into conversationThread.
    ui$.selectedThread.set('t-current')
    expect(compose$.conversationThread.get()).toBe('t-current')

    // Opening a thread tab retargets selectedThread to load its own messages,
    // but must not disturb the Current tab's remembered thread.
    openThreadTab(message({ thread_id: 't-2', id: 'm2', subject: 'Thread 2' }))
    expect(compose$.activeTab.get()).toBe('thread-t-2')
    expect(ui$.selectedThread.get()).toBe('t-2')
    expect(compose$.conversationThread.get()).toBe('t-current')
  })

  it('restores the Current conversation when switching back to the Current tab', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message({ thread_id: 't-2', id: 'm2' }))
    expect(ui$.selectedThread.get()).toBe('t-2')

    activateConversationTab()
    expect(compose$.activeTab.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('t-current')
  })

  it('returns to the originating thread tab when a reader tab opened from it is closed', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message({ thread_id: 't-2', id: 'm2', subject: 'Thread 2' }))

    // Open a message from within the thread tab in its own reader tab.
    openMessageTab(message({ id: 'msg-x', thread_id: 't-2', subject: 'Msg X' }))
    expect(compose$.activeTab.get()).toBe('msg-x')

    // Closing it returns to the thread tab it was opened from, not a neighbor.
    void closeMessageTab('msg-x')
    expect(compose$.activeTab.get()).toBe('thread-t-2')
    expect(ui$.selectedThread.get()).toBe('t-2')
    expect(compose$.tabs.get().map((tab) => tab.id)).toEqual(['thread-t-2'])
  })

  it('snapshots message attachments when opening a reader tab', () => {
    openMessageTab(
      message({
        id: 'msg-image',
        attachments: [
          {
            filename: 'photo.png',
            mime: 'image/png',
            size: 42,
            key: null,
            url: 'data:image/png;base64,aW1hZ2U=',
          },
        ],
      }),
    )

    expect(compose$.tabs.get()[0].attachments).toEqual([
      {
        filename: 'photo.png',
        mime: 'image/png',
        size: 42,
        key: null,
        url: 'data:image/png;base64,aW1hZ2U=',
      },
    ])
  })

  it('falls back to the Current conversation when the last tab is closed', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message({ thread_id: 't-2', id: 'm2' }))

    void closeMessageTab('thread-t-2')
    expect(compose$.activeTab.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('t-current')
    expect(compose$.tabs.get()).toHaveLength(0)
  })
})

describe('quick reply draft sharing', () => {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyFrom.set('')
    mail$.messages.set([])
    mail$.threads.set([])
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
    ui$.selectedThread.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([
      {
        id: 'acc-1',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
      },
    ])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
            if (command === 'mail.folderList') return { folders: [] }
            if (command === 'mail.threadList') return { threads: [] }
            return {}
          },
        },
      },
    }
  })

  it('saves the quick reply as a server draft and reuses the same id across autosaves', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')

    await saveQuickReplyDraft()

    expect(calls.filter((c) => c.command === 'mail.saveDraft')).toHaveLength(1)
    expect(calls.filter((c) => c.command === 'mail.allocateIdentity')).toHaveLength(1)
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
    const firstDraftId = compose$.quickReplyDraftId.get()
    expect(firstDraftId).toBe('draft-core@example.com')

    compose$.composer.set('Hello there, updated')
    await saveQuickReplyDraft()

    const saveCalls = calls.filter((c) => c.command === 'mail.saveDraft')
    expect(saveCalls).toHaveLength(2)
    expect(calls.filter((c) => c.command === 'mail.allocateIdentity')).toHaveLength(1)
    expect(compose$.quickReplyDraftId.get()).toBe(firstDraftId)
    expect((saveCalls[1].payload as { draft_id: string }).draft_id).toBe(firstDraftId)
  })

  it('does nothing when there is no active thread or the composer is empty', async () => {
    await saveQuickReplyDraft()
    expect(calls.some((c) => c.command === 'mail.saveDraft')).toBe(false)
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('discards the quick reply draft once cleared back to blank', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      from_addr: 'me@example.com',
      message_id: 'draft-1@example.com',
      body: 'Hello there',
      date: thread.date + 1,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')
    compose$.quickReplyDraftId.set('draft-1@example.com')
    compose$.quickReplyDraftSaved.set(true)

    compose$.composer.set('')
    await discardQuickReplyDraftIfEmpty()

    expect(calls.find((c) => c.command === 'mail.discardDraft')?.payload).toMatchObject({
      account_id: 'acc-1',
      draft_id: 'draft-1@example.com',
      thread_id: 't-1',
    })
    expect(mail$.messages.get()).toEqual([thread])
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('discards the draft when the composer is cleared while the autosave is still in flight', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('abc')

    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      return {}
    }

    const save = saveQuickReplyDraft()
    await started

    // Cleared mid-save: quickReplyDraftSaved is still false here, so this
    // discard alone can't see the draft the RPC is about to create.
    compose$.composer.set('')
    const discard = discardQuickReplyDraftIfEmpty()
    releaseSave()
    await save
    await discard

    expect(calls.filter((c) => c.command === 'mail.saveDraft')).toHaveLength(1)
    const discardCalls = calls.filter((c) => c.command === 'mail.discardDraft')
    expect(discardCalls).toHaveLength(1)
    expect(discardCalls[0].payload).toMatchObject({ account_id: 'acc-1', thread_id: 't-1' })
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('does nothing when there is no saved draft to discard', async () => {
    await discardQuickReplyDraftIfEmpty()
    expect(calls.some((c) => c.command === 'mail.discardDraft')).toBe(false)
  })

  it('hydrates the quick reply from a saved draft at the tail of the conversation', () => {
    ui$.selectedThread.set('t-2')
    const ancestor = message({
      id: 'm1',
      thread_id: 't-2',
      folder_id: 'INBOX',
      date: 1000,
    })
    const draft = message({
      id: 'd1',
      thread_id: 't-2',
      folder_id: 'Drafts',
      from_addr: 'me@example.com',
      message_id: 'draft-1@example.com',
      body: 'saved draft body',
      date: 2000,
    })

    mail$.messages.set([ancestor, draft])

    expect(compose$.composer.get()).toBe('saved draft body')
    expect(compose$.quickReplyDraftId.get()).toBe('draft-1@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
    expect(compose$.quickReplyFrom.get()).toBe('me@example.com')
  })

  it('keeps the hydrated draft hidden after an optimistic sent bubble is appended', () => {
    const ancestor = message({ id: 'm1', folder_id: 'INBOX', message_id: 'root@example.com' })
    const draft = message({
      id: 'd1',
      folder_id: 'Drafts',
      message_id: 'draft-1@example.com',
    })
    const sending = message({ id: 'local-send-1', folder_id: 'INBOX', send_status: 'sending' })

    expect(withoutHydratedQuickReplyDraft([ancestor, draft, sending], 'draft-1@example.com', true)).toEqual([
      ancestor,
      sending,
    ])
  })

  it('does not rehydrate the consumed draft when the thread refreshes during send', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      from_addr: 'me@example.com',
      message_id: 'reply-draft@example.com',
      body: 'Hello there',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')
    compose$.quickReplyDraftId.set('reply-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignature.set(null)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        // A mail.synced refresh can land before sendReply gets to discard the
        // server draft, temporarily making that draft the conversation tail.
        mail$.messages.set([thread, draft])
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('keeps the consumed draft guarded while the post-send discard is still in flight', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'reply-draft@example.com',
      body: 'Consumed reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Consumed reply')
    compose$.quickReplyDraftId.set('reply-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignature.set(null)
    settings$.signature.set('')

    let releaseDiscard!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.discardDraft') {
        await new Promise<void>((resolve) => (releaseDiscard = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    // SMTP has returned, so the pending payload is gone and a refresh can
    // already have swapped the optimistic bubble for the canonical Sent copy —
    // while the server draft this reply consumed is still there, and still the
    // conversation tail. The guard has to outlive its bubble until the discard
    // resolves, or the just-sent text lands back in the cleared composer.
    const sentCopy = message({
      id: 'sent-row',
      account_id: 'acc-1',
      folder_id: 'Sent',
      thread_id: 't-1',
      message_id: 'sent@example.com',
      date: 1500,
    })
    mail$.messages.set([thread, sentCopy, draft])

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')

    releaseDiscard()
    await new Promise((resolve) => setTimeout(resolve, 0))
  })

  it('allows a different draft to hydrate after the sent draft cleanup settles', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const sentDraft = message({
      id: 'sent-draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'sent-draft@example.com',
      body: 'First reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, sentDraft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('sent-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignature.set(null)
    settings$.signature.set('')

    await sendReply()
    await new Promise((resolve) => setTimeout(resolve, 0))

    const newerDraft = message({
      id: 'new-draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'different-draft@example.com',
      body: 'Written elsewhere',
      date: 3000,
    })
    mail$.messages.set([thread, newerDraft])

    expect(compose$.composer.get()).toBe('Written elsewhere')
    expect(compose$.quickReplyDraftId.get()).toBe('different-draft@example.com')
  })

  it('keeps the consumed draft guarded across a navigation round trip during send', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const other = message({ id: 'other', account_id: 'acc-1', thread_id: 't-2', message_id: 'other@example.com' })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'reply-draft@example.com',
      body: 'Consumed reply',
      date: 2000,
    })
    mail$.threads.set([thread, other])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Consumed reply')
    compose$.quickReplyDraftId.set('reply-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignature.set(null)
    settings$.signature.set('')

    let sendStarted!: () => void
    const started = new Promise<void>((resolve) => (sendStarted = resolve))
    let releaseSend!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        sendStarted()
        await new Promise<void>((resolve) => (releaseSend = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await started
    ui$.selectedThread.set('t-2')
    ui$.selectedThread.set('t-1')
    mail$.messages.set([thread, draft])

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)

    releaseSend()
    await send
  })

  it('keeps unrelated older drafts visible', () => {
    const olderDraft = message({ id: 'd0', folder_id: 'Drafts', message_id: 'draft-0@example.com' })
    const activeDraft = message({ id: 'd1', folder_id: 'Drafts', message_id: 'draft-1@example.com' })

    expect(withoutHydratedQuickReplyDraft([olderDraft, activeDraft], 'draft-1@example.com', true)).toEqual([olderDraft])
  })

  it('does not hydrate when the tail message is not a draft', () => {
    ui$.selectedThread.set('t-3')
    compose$.composer.set('unrelated text')
    const onlyMessage = message({ id: 'm1', thread_id: 't-3', folder_id: 'INBOX', date: 1000 })

    mail$.messages.set([onlyMessage])

    expect(compose$.composer.get()).toBe('unrelated text')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('clears quick reply state when the active conversation is closed', () => {
    ui$.selectedThread.set('t-4')
    compose$.composer.set('abc')
    compose$.quickReplyDraftId.set('draft-1@example.com')
    compose$.quickReplyDraftSaved.set(true)

    ui$.selectedThread.set('')

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('hands off the saved quick reply draft id when escalating to the full editor', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')
    await saveQuickReplyDraft()
    const draftId = compose$.quickReplyDraftId.get()

    openReplyInFullEditor()

    expect(compose$.tabs.get()).toHaveLength(1)
    expect(compose$.tabs.get()[0].compose?.draftMessageId).toBe(draftId)
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })
})

describe('quick reply send-as identity', () => {
  const calls: { command: string; payload: unknown }[] = []

  const setUpThread = (overrides: Partial<Message> = {}) => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      to: 'sales@example.com',
      message_id: 'root@example.com',
      ...overrides,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    return thread
  }

  beforeEach(() => {
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyFrom.set('')
    mail$.messages.set([])
    mail$.threads.set([])
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
    ui$.selectedThread.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([
      {
        id: 'acc-1',
        email: 'me@example.com',
        display_name: 'Me',
        sender_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
        aliases: [{ email: 'sales@example.com', name: 'Sales' }],
      },
    ])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
            return {}
          },
        },
      },
    }
  })

  it('offers every identity and preselects the alias the original was delivered to', () => {
    setUpThread()

    const { identities, selected } = quickReplyFromState()

    expect(identities.map((id) => id.email)).toEqual(['me@example.com', 'sales@example.com'])
    expect(selected?.email).toBe('sales@example.com')
  })

  it('continues with the alias used by the newest outgoing reply', () => {
    accounts$.set([
      {
        ...accounts$.get()[0],
        aliases: [
          { email: 'sales@example.com', name: 'Sales' },
          { email: 'support@example.com', name: 'Support' },
        ],
      },
    ])
    const inbound = setUpThread()
    mail$.messages.set([
      inbound,
      message({
        id: 'sent',
        account_id: 'acc-1',
        thread_id: 't-1',
        folder_id: 'Sent',
        from_addr: 'support@example.com',
        to: 'them@example.com',
        outgoing: true,
        date: inbound.date + 1,
      }),
    ])

    expect(quickReplyFromState().selected?.email).toBe('support@example.com')
    expect(resolveQuickReplyFrom(inbound, accounts$.get()[0])).toBe('support@example.com')
  })

  it('ignores an inbound message from a shared address configured as an alias', () => {
    accounts$.set([
      {
        ...accounts$.get()[0],
        aliases: [
          { email: 'sales@example.com', name: 'Sales' },
          { email: 'support@example.com', name: 'Support' },
        ],
      },
    ])
    const inbound = setUpThread()
    // A colleague sending from the shared support address. The core flags it
    // outgoing because its From matches a configured identity, but it was
    // delivered to our inbox, so it is not mail we sent.
    const colleague = message({
      id: 'colleague',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'support@example.com',
      to: 'sales@example.com',
      outgoing: true,
      date: inbound.date + 1,
    })
    mail$.messages.set([inbound, colleague])
    const thread = mail$.threads.get()[0]

    // The colleague's message is still the one we reply to, and the From falls
    // through to the alias the thread was delivered to.
    expect(pickReplyTarget(thread)).toEqual(colleague)
    expect(resolveQuickReplyFrom(inbound, accounts$.get()[0])).toBe('sales@example.com')
  })

  it('hides the picker when the account has a single identity', () => {
    accounts$.set(accounts$.get().map((acc) => ({ ...acc, aliases: [] })))
    setUpThread()

    expect(quickReplyFromState().identities).toEqual([])
  })

  it('prefers an explicit override over the detected alias', () => {
    const thread = setUpThread()
    compose$.quickReplyFrom.set('me@example.com')

    expect(quickReplyFromState().selected?.email).toBe('me@example.com')
    // The primary normalizes back to "", which the send path reads as the default.
    expect(resolveQuickReplyFrom(thread, accounts$.get()[0])).toBe('')
  })

  it('sends the overridden alias as the draft From', async () => {
    setUpThread({ to: 'me@example.com' })
    compose$.composer.set('Hello there')
    compose$.quickReplyFrom.set('sales@example.com')

    await saveQuickReplyDraft()

    const saved = calls.find((c) => c.command === 'mail.saveDraft')?.payload as { from?: string }
    expect(saved.from).toBe('sales@example.com')
  })

  it('carries the override into the full editor and clears it with the quick reply', () => {
    setUpThread({ to: 'me@example.com' })
    compose$.composer.set('Hello there')
    compose$.quickReplyFrom.set('sales@example.com')

    openReplyInFullEditor()

    expect(compose$.tabs.get()[0].compose?.fromEmail).toBe('sales@example.com')
    expect(compose$.quickReplyFrom.get()).toBe('')
  })
})

describe('signatures in a new compose tab', () => {
  const sendable = {
    id: 'acc-1',
    email: 'me@example.com',
    display_name: 'Me',
    provider: 'custom',
    auth_type: 'password' as const,
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
  }

  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([sendable])
    settings$.signature.set('')
  })

  const draftOf = (id: string | undefined) => compose$.tabs.get().find((tab) => tab.id === id)?.compose

  it('seeds a rich draft with the app-wide signature', () => {
    settings$.signature.set('<p>Ping</p>')

    expect(draftOf(openComposeTab())?.html).toBe('<p></p><p>Ping</p>')
  })

  it('lets an account override or opt out', () => {
    settings$.signature.set('<p>Ping</p>')
    accounts$.set([{ ...sendable, signature: { mode: 'custom', html: '<p>Mine</p>' } }])
    expect(draftOf(openComposeTab())?.html).toBe('<p></p><p>Mine</p>')

    accounts$.set([{ ...sendable, signature: { mode: 'none', html: '<p>Mine</p>' } }])
    expect(draftOf(openComposeTab())?.html).toBe('')
  })

  it('skips the signature when re-opening an existing message', () => {
    settings$.signature.set('<p>Ping</p>')

    expect(draftOf(openComposeTab({ noSignature: true }))?.html).toBe('')
  })
})

describe('signature on a change of From account', () => {
  const account = (id: string, signature?: { mode: 'global' | 'none' | 'custom'; html: string }) => ({
    id,
    email: `${id}@example.com`,
    display_name: id,
    provider: 'custom',
    auth_type: 'password' as const,
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
    signature,
  })

  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedAccount.set('a')
    settings$.signature.set('')
    accounts$.set([
      account('a', { mode: 'custom', html: '<p>A</p>' }),
      account('b', { mode: 'custom', html: '<p>B</p>' }),
      account('c', { mode: 'none', html: '' }),
    ])
  })

  const draftOf = (id: string | undefined) => compose$.tabs.get().find((tab) => tab.id === id)?.compose

  it("replaces the old account's signature with the new one's", () => {
    const id = openComposeTab()!
    expect(draftOf(id)?.html).toBe('<p></p><p>A</p>')

    updateComposeDraft(id, { accountId: 'b', fromEmail: 'b@example.com' })

    expect(draftOf(id)?.html).toBe('<p></p><p>B</p>')
    expect(draftOf(id)?.signature?.html).toBe('<p>B</p>')
  })

  it('drops it when the new account sends none', () => {
    const id = openComposeTab()!
    updateComposeDraft(id, { accountId: 'c' })

    // The blank line the signature came with goes too, so switching accounts
    // repeatedly cannot pile up empty paragraphs.
    expect(draftOf(id)?.html).toBe('')
    // Not "unmanaged": the app knows this body has no signature and where one
    // would go, so moving to an account that has one puts it back there.
    expect(draftOf(id)?.signature).toEqual({ html: '', text: '', placement: 'belowText' })

    updateComposeDraft(id, { accountId: 'b' })
    expect(draftOf(id)?.html).toBe('<p></p><p>B</p>')
  })

  it('follows the draft through a switch to plaintext', () => {
    const id = openComposeTab()!
    // What the composer's rich/plain toggle does: the body becomes text, and
    // the tracked signature has to be findable in that form too.
    updateComposeDraft(id, { rich: false, html: '', text: '\n\nA' })
    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.text).toBe('\n\nB')
  })

  it('leaves a reopened draft body alone, rather than giving it two signatures', () => {
    // "Edit as New Message" and saved drafts come with a body this app did not
    // compose; it may already end in a signature.
    const id = openComposeTab({ rich: true, html: '<p>hello</p><p>A</p>', noSignature: true })!
    expect(draftOf(id)?.signature).toBeUndefined()

    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.html).toBe('<p>hello</p><p>A</p>')
    expect(draftOf(id)?.signature).toBeUndefined()
  })

  it('leaves the body alone once the signature has been edited', () => {
    const id = openComposeTab()!
    updateComposeDraft(id, { html: '<p></p><p>A, but mine now</p>' })
    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.html).toBe('<p></p><p>A, but mine now</p>')
  })

  it('does not touch the body when the account is unchanged', () => {
    const id = openComposeTab()!
    updateComposeDraft(id, { accountId: 'a', fromEmail: 'alias@example.com' })

    expect(draftOf(id)?.html).toBe('<p></p><p>A</p>')
  })
})

describe('signature in the quick reply', () => {
  const calls: { command: string; payload: unknown }[] = []

  const sendable = {
    id: 'acc-1',
    email: 'me@example.com',
    display_name: 'Me',
    provider: 'custom',
    auth_type: 'password' as const,
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
  }

  const setUpThread = () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    return thread
  }

  beforeEach(() => {
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyFrom.set('')
    compose$.quickReplySignature.set(null)
    mail$.messages.set([])
    mail$.threads.set([])
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
    ui$.selectedThread.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([sendable])
    settings$.signature.set('<p>from u2</p>')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
            return {}
          },
        },
      },
    }
  })

  it('seeds the box with the signature, under a blank line to type in', () => {
    setUpThread()
    seedQuickReplySignature()

    expect(compose$.composer.get()).toBe('\n\nfrom u2')
    expect(compose$.quickReplySignature.get()?.text).toBe('from u2')
  })

  it('honours a per-account override', () => {
    accounts$.set([{ ...sendable, signature: { mode: 'none', html: '' } }])
    setUpThread()
    seedQuickReplySignature()

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplySignature.get()).toBeNull()
  })

  it('counts a box holding only the signature as blank, and does not save a draft for it', async () => {
    setUpThread()
    seedQuickReplySignature()

    expect(isQuickReplyBlank()).toBe(true)
    await saveQuickReplyDraft()
    expect(calls.filter((c) => c.command === 'mail.saveDraft')).toHaveLength(0)
  })

  it('counts typing above the signature as content', async () => {
    setUpThread()
    seedQuickReplySignature()
    compose$.composer.set('t2\n\nfrom u2')

    expect(isQuickReplyBlank()).toBe(false)
    await saveQuickReplyDraft()

    const saved = calls.find((c) => c.command === 'mail.saveDraft')?.payload as { body?: string }
    expect(saved.body).toBe('t2\n\nfrom u2')
  })

  it("sends the user's own whitespace exactly as written", async () => {
    setUpThread()
    seedQuickReplySignature()
    compose$.composer.set('    indented start\n\nand a trailing blank line\n\n\nfrom u2')

    await saveQuickReplyDraft()

    const saved = calls.find((c) => c.command === 'mail.saveDraft')?.payload as { body?: string }
    expect(saved.body).toBe('    indented start\n\nand a trailing blank line\n\n\nfrom u2')
  })

  it('drops the seeded blank line when nothing was typed above it', async () => {
    setUpThread()
    seedQuickReplySignature()
    // Attachment-only: the body is the untouched signature, which must not go
    // out with the leading newlines it was seeded under.
    compose$.composerAttachments.set([{ id: 'a1', filename: 'x.png', mime: 'image/png', size: 1, data: 'AA==' }])

    await saveQuickReplyDraft()

    const saved = calls.find((c) => c.command === 'mail.saveDraft')?.payload as { body?: string }
    expect(saved.body).toBe('from u2')
  })

  it('puts the caret at the end of the user text, above the signature', () => {
    setUpThread()
    seedQuickReplySignature()
    expect(quickReplyCaretOffset()).toBe(0)

    compose$.composer.set('t2\n\nfrom u2')
    expect(quickReplyCaretOffset()).toBe(2)
  })

  it('leaves a hydrated draft body alone', () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.folders.set([{ id: 'Drafts', account_id: 'acc-1', name: 'Drafts', kind: 'drafts' } as any])
    mail$.foldersByAccount.set({ 'acc-1': [{ id: 'Drafts', name: 'Drafts', kind: 'drafts' } as any] })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-1')
    mail$.messages.set([
      thread,
      message({
        id: 'd1',
        account_id: 'acc-1',
        thread_id: 't-1',
        folder_id: 'Drafts',
        message_id: 'draft@example.com',
        body: 'half typed\n\nfrom u2',
        date: thread.date + 60,
      }),
    ])

    // The saved body already carries its own signature: nothing is appended,
    // and none of it is discounted as "not content".
    expect(compose$.composer.get()).toBe('half typed\n\nfrom u2')
    expect(compose$.quickReplySignature.get()).toBeNull()
    expect(isQuickReplyBlank()).toBe(false)
  })

  it('hands the full editor a stripped body so it inserts and tracks its own', () => {
    setUpThread()
    seedQuickReplySignature()
    compose$.composer.set('t2\n\nfrom u2')

    openReplyInFullEditor()

    const draft = compose$.tabs.get()[0].compose
    expect(draft?.text).toBe('t2\n\nfrom u2')
    expect(draft?.signature).toEqual({ html: '<p>from u2</p>', text: 'from u2', placement: 'belowText' })
    // And the box behind the new tab is a fresh quick reply, not an empty one.
    expect(compose$.composer.get()).toBe('\n\nfrom u2')
  })

  it('does not give the full editor a second copy of an edited signature', () => {
    setUpThread()
    seedQuickReplySignature()
    compose$.composer.set('t2\n\nfrom u2, but edited')

    openReplyInFullEditor()

    const draft = compose$.tabs.get()[0].compose
    expect(draft?.text).toBe('t2\n\nfrom u2, but edited')
    expect(draft?.signature).toBeUndefined()
  })

  it('re-seeds an untouched box when the app signature arrives late', () => {
    settings$.signature.set('')
    setUpThread()
    seedQuickReplySignature()
    expect(compose$.composer.get()).toBe('')

    settings$.signature.set('<p>from u2</p>')
    expect(compose$.composer.get()).toBe('\n\nfrom u2')
  })

  it('does not re-seed over something the user typed', () => {
    setUpThread()
    seedQuickReplySignature()
    compose$.composer.set('t2\n\nfrom u2')

    settings$.signature.set('<p>different</p>')

    expect(compose$.composer.get()).toBe('t2\n\nfrom u2')
  })
})

describe('revealMessageRemote', () => {
  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    resetThreadView()
  })

  it('marks an open reader tab so a thread switch cannot re-block it', () => {
    openMessageTab(message({ id: 'm1', body_html: '<p>hi</p>' }))
    openMessageTab(message({ id: 'm2', body_html: '<p>other</p>' }))
    // Reading the conversation, not the tab: the inactive tab is unmounted, so
    // it cannot copy the reveal itself.
    compose$.activeTab.set('')

    revealMessageRemote('m1')
    expect(thread$.revealedRemote.get().m1).toBe(true)

    // The thread switch clears the conversation's reveal map; the tab keeps its
    // own copy, and the message that was never revealed keeps none.
    resetThreadView()
    const tabs = compose$.tabs.get()
    expect(tabs.find((tab) => tab.messageId === 'm1')?.revealRemote).toBe(true)
    expect(tabs.find((tab) => tab.messageId === 'm2')?.revealRemote).toBe(false)
  })
})
