import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import {
  activateConversationTab,
  closeMessageTab,
  compose$,
  discardQuickReplyDraftIfEmpty,
  draftShouldOpenConversation,
  openDraftCompose,
  openDraftConversationOrCompose,
  openMessageTab,
  openReplyInFullEditor,
  openThreadTab,
  openThreadTabById,
  saveQuickReplyDraft,
  withoutHydratedQuickReplyDraft,
} from './compose'
import { accounts$ } from './accounts'
import { ui$ } from './ui'
import { mail$ } from './mail'

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
    closeMessageTab('msg-x')
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

    closeMessageTab('thread-t-2')
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
      message_id: 'draft-1@example.com',
      body: 'saved draft body',
      date: 2000,
    })

    mail$.messages.set([ancestor, draft])

    expect(compose$.composer.get()).toBe('saved draft body')
    expect(compose$.quickReplyDraftId.get()).toBe('draft-1@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
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
