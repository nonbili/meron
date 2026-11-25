import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { openReplyInFullEditor, replyToMessage, editAsNewMessage, forwardMessage } from './compose'
import { quickReplyFromState, pickReplyTarget, resolveQuickReplyFrom } from './composeReply'
import { compose$ } from './composeState'
import { saveQuickReplyDraft } from './quickReply'
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

  it.each([
    ['an unmatched recipient', 'dev@work.example', '', 'alex@work.example'],
    ['a named recipient with mixed case', '"Dev, Team" <DEV@WORK.EXAMPLE>', '', 'alex@work.example'],
    ['a copied recipient', 'other@elsewhere.test', 'dev@work.example', 'alex@work.example'],
    [
      'an exact alias despite another matching domain',
      'dev@work.example',
      'dev@personal.example',
      'dev@personal.example',
    ],
    [
      'the exact primary despite another matching domain',
      'dev@work.example',
      'alex@personal.example',
      'alex@personal.example',
    ],
    ['an ambiguous domain', 'unknown@personal.example', '', 'alex@personal.example'],
    ['multiple matching domains', 'dev@work.example, unknown@personal.example', '', 'alex@personal.example'],
    ['an unrelated domain with a matching username', 'dev@elsewhere.test', '', 'alex@personal.example'],
    ['a subdomain', 'dev@mail.work.example', '', 'alex@personal.example'],
    ['missing recipients', '', '', 'alex@personal.example'],
  ])('selects the reply identity for %s', (_label, to, cc, expected) => {
    accounts$.set([
      {
        ...accounts$.get()[0],
        email: 'alex@personal.example',
        aliases: [{ email: 'alex@other.example' }, { email: 'dev@personal.example' }, { email: 'alex@work.example' }],
      },
    ])
    const inbound = setUpThread({ to, cc })

    expect(quickReplyFromState().selected?.email).toBe(expected)
    expect(resolveQuickReplyFrom(inbound, accounts$.get()[0]) || accounts$.get()[0].email).toBe(expected)

    replyToMessage(inbound)
    expect(compose$.tabs.get().at(-1)?.compose?.fromEmail || accounts$.get()[0].email).toBe(expected)
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

  it('skips messages whose reply would only reach a no-reply or bounce address', () => {
    const inbound = setUpThread()
    const reply = (to: string, accepts_replies: boolean) => ({
      to,
      cc: '',
      all_to: to,
      all_cc: '',
      all_adds_recipients: false,
      accepts_replies,
    })
    // GitHub's pull request mail, its push notification and a bounce.
    const pullRequest = message({
      ...inbound,
      id: 'pr',
      reply: reply('reply+abc@reply.github.com', true),
      date: inbound.date + 1,
    })
    const push = message({ ...inbound, id: 'push', reply: reply('noreply@github.com', false), date: inbound.date + 2 })
    const bounce = message({
      ...inbound,
      id: 'bounce',
      from_addr: 'postmaster@microsoft.com',
      reply: reply('postmaster@microsoft.com', false),
      date: inbound.date + 3,
    })
    mail$.messages.set([inbound, pullRequest, push, bounce])
    const thread = mail$.threads.get()[0]
    expect(pickReplyTarget(thread).id).toBe('pr')

    // With nothing that accepts replies, the newest received message still wins.
    mail$.messages.set([push, bounce])
    expect(pickReplyTarget(thread).id).toBe('bounce')
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

  it('keeps the sending alias when a sent message is edited as new', async () => {
    const sent = setUpThread({ folder_id: 'Sent', from_addr: 'Sales@example.com', to: 'them@example.com' })

    await editAsNewMessage(sent)

    expect(compose$.tabs.get()[0].compose?.fromEmail).toBe('sales@example.com')
  })

  it('leaves a received message edited as new on the primary', async () => {
    await editAsNewMessage(setUpThread())

    expect(compose$.tabs.get()[0].compose?.fromEmail).toBe('')
  })

  // A forward of encrypted mail carries its decrypted text in the body: it is
  // marked so, which the save and send paths enforce on their own.
  it('marks a forward of encrypted mail as holding decrypted text', async () => {
    await forwardMessage(setUpThread({ crypto: { encrypted: true } } as Partial<Message>))
    expect(compose$.tabs.get()[0].compose).toMatchObject({
      encrypt: true,
      sensitiveSource: 'forward',
      decryptedInBody: true,
    })
  })

  // The copy carries the decrypted body: it can only go out encrypted, even
  // with "encrypt when possible" off.
  it('keeps a copy of encrypted mail edited as new encrypted', async () => {
    await editAsNewMessage(setUpThread({ crypto: { encrypted: true } } as Partial<Message>))
    expect(compose$.tabs.get()[0].compose).toMatchObject({ encrypt: true, decryptedInBody: true })

    compose$.tabs.set([])
    await editAsNewMessage(setUpThread())
    expect(compose$.tabs.get()[0].compose?.encrypt).toBeUndefined()
    expect(compose$.tabs.get()[0].compose?.decryptedInBody).toBeUndefined()
  })
})
