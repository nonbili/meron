import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { openReplyInFullEditor, replyAllToMessage } from './compose'
import { buildReplyRecipients, quickReplyRecipients, replyAllAddsRecipients, quickReplyAddable } from './composeReply'
import { compose$ } from './composeState'
import { addToQuickReply, cancelQuickReplyDraftSave, saveQuickReplyDraft } from './quickReply'
import { sendReply } from './quickReplySend'
import { accounts$ } from './accounts'
import { settings$ } from './settings'
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

describe('reply-all recipients', () => {
  // The rule itself lives in the core (meron-core/src/reply.rs) and is tested
  // there; what matters here is that the composer uses the recipients the core
  // sent rather than deriving its own.
  const replied = (overrides: Partial<Message['reply']> = {}): Message['reply'] => ({
    to: 'Them <them@example.com>',
    cc: 'bob@example.com',
    all_to: 'Them <them@example.com>',
    all_cc: 'Alice <alice@example.com>, bob@example.com',
    all_adds_recipients: true,
    ...overrides,
  })

  const incoming = message({
    from_name: 'Them',
    from_addr: 'them@example.com',
    to: 'me@example.com, Alice <alice@example.com>',
    cc: 'bob@example.com',
    reply: replied(),
  })

  it('addresses the sender and copies the rest for reply-all', () => {
    expect(buildReplyRecipients(incoming, true)).toEqual({
      to: 'Them <them@example.com>',
      cc: 'Alice <alice@example.com>, bob@example.com',
    })
  })

  it('takes the plain reply form by default', () => {
    expect(buildReplyRecipients(incoming)).toEqual({ to: 'Them <them@example.com>', cc: 'bob@example.com' })
  })

  it('falls back to the sender for a target with no recipients of its own', () => {
    // A thread card standing in for a thread whose messages have not loaded.
    const card = message({ from_name: 'Them', from_addr: 'them@example.com', to: '', reply: undefined })

    expect(buildReplyRecipients(card, true)).toEqual({ to: 'Them <them@example.com>', cc: '' })
  })

  it('offers reply-all only when the core says it reaches someone new', () => {
    expect(replyAllAddsRecipients(incoming)).toBe(true)
    expect(replyAllAddsRecipients(message({ reply: replied({ all_adds_recipients: false }) }))).toBe(false)
    expect(replyAllAddsRecipients(message({ reply: undefined }))).toBe(false)
  })

  it('seeds a composer tab addressed to everyone when replying all to one message', () => {
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
      },
    ])
    compose$.tabs.set([])
    compose$.activeTab.set('')

    replyAllToMessage(
      message({
        account_id: 'acc-1',
        thread_id: 'reply-thread',
        from_name: 'Them',
        from_addr: 'them@example.com',
        to: 'me@example.com, Alice <alice@example.com>',
        cc: 'bob@example.com',
        subject: 'Design',
        message_id: 'root@example.com',
        reply: replied(),
      }),
    )

    expect(compose$.tabs.peek()[0].threadId).toBe('reply-thread')
    const draft = compose$.tabs.get()[0]?.compose
    expect(draft?.to).toBe('Them <them@example.com>')
    expect(draft?.cc).toBe('Alice <alice@example.com>, bob@example.com')
    expect(draft?.showCcBcc).toBe(true)
    expect(draft?.subject).toBe('Re: Design')
    expect(draft?.inReplyTo).toBe('root@example.com')
  })
})

describe('quick reply recipient line', () => {
  beforeEach(() => {
    mail$.threads.set([])
    mail$.messages.set([])
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
      },
    ])
  })

  it('reports the addresses the box would send to', () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_name: 'Them',
      from_addr: 'them@example.com',
      to: 'me@example.com, alice@example.com',
      cc: 'bob@example.com, me@example.com',
      message_id: 'root@example.com',
      reply: {
        to: 'Them <them@example.com>',
        cc: 'bob@example.com',
        all_to: 'Them <them@example.com>',
        all_cc: 'alice@example.com, bob@example.com',
        all_adds_recipients: true,
      },
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')

    // A plain reply: alice is dropped, our own address stays out of the Cc.
    expect(quickReplyRecipients()).toEqual({ to: 'Them <them@example.com>', cc: 'bob@example.com' })
  })

  it('reports nothing without an open conversation, which hides the line', () => {
    expect(quickReplyRecipients()).toEqual({ to: '', cc: '' })
  })
})

describe('adding people to the quick reply', () => {
  // We were copied on a message to Alice and Bob, so a plain reply goes to the
  // sender alone, and Reply all would also copy both.
  const calls: { command: string; payload: unknown }[] = []
  const reply = {
    to: 'Them <them@example.com>',
    cc: '',
    all_to: 'Them <them@example.com>',
    all_cc: 'Alice <alice@example.com>, Bob <bob@example.com>',
    all_adds_recipients: true,
  }
  const setUpThread = (overrides: Partial<Message> = {}) => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_name: 'Them',
      from_addr: 'them@example.com',
      to: 'Alice <alice@example.com>, Bob <bob@example.com>',
      cc: 'me@example.com',
      subject: 'Plans',
      message_id: 'root@example.com',
      date: 1000,
      reply,
      ...overrides,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    return thread
  }

  beforeEach(() => {
    cancelQuickReplyDraftSave()
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyDraftThreadId.set('')
    compose$.quickReplyFrom.set('')
    compose$.quickReplyAdded.set([])
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
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
            if (command === 'mail.threadList') return { threads: mail$.threads.get() }
            return {}
          },
        },
      },
    }
  })

  it('offers the people Reply all would add, one by one, until each is in', () => {
    setUpThread()
    expect(quickReplyAddable()).toEqual(['Alice <alice@example.com>', 'Bob <bob@example.com>'])

    addToQuickReply('Bob <bob@example.com>')

    expect(quickReplyRecipients()).toEqual({ to: 'Them <them@example.com>', cc: 'Bob <bob@example.com>' })
    expect(quickReplyAddable()).toEqual(['Alice <alice@example.com>'])
  })

  it('offers nobody when Reply all reaches no one new', () => {
    setUpThread({ reply: { ...reply, all_cc: '', all_adds_recipients: false } })

    expect(quickReplyAddable()).toEqual([])
  })

  it('saves and sends the reply to the people added', async () => {
    setUpThread()
    addToQuickReply('Alice <alice@example.com>')
    compose$.composer.set('Sounds good')

    await saveQuickReplyDraft()
    const saved = calls.find((c) => c.command === 'mail.saveDraft')?.payload as { to: string; cc: string }
    expect(saved.cc).toBe('Alice <alice@example.com>')

    await sendReply()
    const sent = calls.find((c) => c.command === 'mail.send')?.payload as { to: string; cc: string }
    expect(sent.to).toBe('Them <them@example.com>')
    expect(sent.cc).toBe('Alice <alice@example.com>')
    // The next reply in the box starts as a plain one.
    expect(compose$.quickReplyAdded.get()).toEqual([])
  })

  it('re-saves a reply in progress when someone is added', async () => {
    setUpThread()
    compose$.composer.set('Sounds good')

    addToQuickReply('Alice <alice@example.com>')
    await new Promise((resolve) => setTimeout(resolve, 1300))

    const saved = calls.find((c) => c.command === 'mail.saveDraft')?.payload as { cc: string }
    expect(saved?.cc).toBe('Alice <alice@example.com>')
  })

  it('puts the people added back when a send never gets off the ground', async () => {
    setUpThread()
    addToQuickReply('Alice <alice@example.com>')
    compose$.composer.set('Sounds good')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') throw new Error('offline')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: mail$.threads.get() }
      return {}
    }

    await sendReply().catch(() => undefined)

    expect(compose$.composer.get()).toBe('Sounds good')
    expect(compose$.quickReplyAdded.get()).toEqual(['alice@example.com'])
  })

  it('carries the people added into the full editor', () => {
    setUpThread()
    addToQuickReply('Alice <alice@example.com>')
    compose$.composer.set('Sounds good')

    openReplyInFullEditor()

    expect(compose$.tabs.get()[0].compose?.cc).toBe('Alice <alice@example.com>')
    expect(compose$.quickReplyAdded.get()).toEqual([])
  })

  it('starts the next conversation as a plain reply', () => {
    setUpThread()
    addToQuickReply('Alice <alice@example.com>')

    ui$.selectedThread.set('')

    expect(compose$.quickReplyAdded.get()).toEqual([])
  })

  it('takes back a saved draft with some of them added', () => {
    const thread = setUpThread()
    mail$.messages.set([
      thread,
      message({
        id: 'd1',
        account_id: 'acc-1',
        thread_id: 't-1',
        folder_id: 'Drafts',
        from_addr: 'me@example.com',
        to: 'Them <them@example.com>',
        cc: 'Bob <bob@example.com>',
        subject: 'Re: Plans',
        message_id: 'draft-1@example.com',
        body: 'saved draft body',
        date: 2000,
      }),
    ])

    expect(compose$.composer.get()).toBe('saved draft body')
    expect(compose$.quickReplyAdded.get()).toEqual(['bob@example.com'])
  })

  it('leaves a draft copying someone Reply all would not reach to the full composer', () => {
    const thread = setUpThread()
    mail$.messages.set([
      thread,
      message({
        id: 'd1',
        account_id: 'acc-1',
        thread_id: 't-1',
        folder_id: 'Drafts',
        from_addr: 'me@example.com',
        to: 'Them <them@example.com>',
        cc: 'Bob <bob@example.com>, Carol <carol@example.com>',
        subject: 'Re: Plans',
        message_id: 'draft-1@example.com',
        body: 'saved draft body',
        date: 2000,
      }),
    ])

    expect(compose$.composer.get()).toBe('')
  })
})
