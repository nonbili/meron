import { describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { mergeRefreshedThreadMessages, messageRenderKey } from './mail'
import { retainThreadData } from './threadData'

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

describe('thread message refresh reconciliation', () => {
  it('keeps an optimistic reply until the canonical Sent copy arrives', () => {
    const original = thread({ id: 'm1', thread_id: 't1', message_id: 'root@example.com', date: 100 })
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 200,
    })

    expect(mergeRefreshedThreadMessages([original, optimistic], [original], 't1')).toEqual([original, optimistic])

    const canonical = thread({
      id: 'sent:2',
      folder_id: 'Sent',
      thread_id: 't1',
      message_id: '<REPLY@example.com>',
      date: 200,
    })
    expect(mergeRefreshedThreadMessages([original, optimistic], [original, canonical], 't1')).toEqual([
      original,
      canonical,
    ])
  })

  it('drops an optimistic reply once a Sent copy arrives under a rewritten Message-ID', () => {
    // Proton Bridge stores the Sent copy under an id of its own, so the id we
    // generated never comes back and only the envelope identifies the copy.
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'Sender <sender@example.com>',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 1000,
    })
    const canonical = thread({
      id: 'sent:2',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'abc@protonmail.internalid',
      outgoing: true,
      date: 1004,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [canonical], 't1')).toEqual([canonical])
  })

  it('renders the Sent copy under the bubble it replaces, so the bubble is not mounted again', () => {
    const original = thread({ id: 'm1', thread_id: 't1', message_id: 'root@example.com', date: 100 })
    const optimistic = thread({
      id: 'sent-key-1',
      thread_id: 't1',
      message_id: 'key-1@example.com',
      send_status: 'sent',
      date: 200,
    })
    const canonical = thread({
      id: 'sent:key-1',
      folder_id: 'Sent',
      thread_id: 't1',
      message_id: '<key-1@example.com>',
      date: 200,
    })

    mergeRefreshedThreadMessages([original, optimistic], [original, canonical], 't1')

    expect(messageRenderKey('sent:key-1')).toBe('sent-key-1')
    expect(messageRenderKey('m1')).toBe('m1')
  })

  // #154 review, point 4: the keys are dropped with the thread's page.
  it("forgets a thread's render keys when its conversation is let go", async () => {
    const optimistic = thread({
      id: 'sent-key-9',
      thread_id: 't9',
      message_id: 'key-9@example.com',
      send_status: 'sent',
    })
    const canonical = thread({
      id: 'sent:key-9',
      folder_id: 'Sent',
      thread_id: 't9',
      message_id: '<key-9@example.com>',
    })
    const release = retainThreadData('t9')

    mergeRefreshedThreadMessages([optimistic], [canonical], 't9')
    expect(messageRenderKey('sent:key-9')).toBe('sent-key-9')

    release()
    await Promise.resolve()
    expect(messageRenderKey('sent:key-9')).toBe('sent:key-9')
  })

  it("carries the bubble's key to a copy found by its envelope", () => {
    const optimistic = thread({
      id: 'sent-key-2',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'key-2@example.com',
      send_status: 'sent',
      date: 1000,
    })
    const canonical = thread({
      id: 'sent:key-2',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'key-2@protonmail.internalid',
      outgoing: true,
      date: 1003,
    })

    mergeRefreshedThreadMessages([optimistic], [canonical], 't1')

    expect(messageRenderKey('sent:key-2')).toBe('sent-key-2')
  })

  it('collapses two identical replies onto two server copies, not one', () => {
    const optimistic = (id: string, date: number) =>
      thread({
        id,
        thread_id: 't1',
        from_addr: 'me@example.com',
        to: 'sender@example.com',
        subject: 'Re: Subject',
        message_id: `${id}@example.com`,
        send_status: 'sent',
        date,
      })
    const canonical = (id: string, date: number) =>
      thread({
        id,
        folder_id: 'Sent',
        thread_id: 't1',
        from_addr: 'me@example.com',
        to: 'sender@example.com',
        subject: 'Re: Subject',
        message_id: `${id}@protonmail.internalid`,
        outgoing: true,
        date,
      })

    const first = optimistic('sent-1', 1000)
    const second = optimistic('sent-2', 1010)
    const serverCopy = canonical('sent:1', 1001)

    // One copy back so far: the second reply keeps its bubble.
    expect(mergeRefreshedThreadMessages([first, second], [serverCopy], 't1')).toEqual([serverCopy, second])
    // Both back: no bubbles left over.
    expect(mergeRefreshedThreadMessages([first, second], [serverCopy, canonical('sent:2', 1011)], 't1')).toHaveLength(2)
  })

  it('pairs a Sent copy with the reply it belongs to, not the first bubble', () => {
    // Two replies in one thread share sender, subject, recipients and a
    // timestamp minutes apart — the envelope alone cannot tell them apart. The
    // second one's copy comes back first.
    const reply = (id: string, body: string, date: number) =>
      thread({
        id,
        thread_id: 't1',
        from_addr: 'me@example.com',
        to: 'sender@example.com',
        subject: 'Re: Subject',
        message_id: `${id}@example.com`,
        body,
        send_status: 'sent',
        date,
      })
    const first = reply('sent-1', 'First reply', 1000)
    const second = reply('sent-2', 'Second reply', 1010)
    const secondCopy = thread({
      id: 'sent:b',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'b@protonmail.internalid',
      body: 'Second reply',
      outgoing: true,
      date: 1011,
    })

    // The first reply keeps its bubble; only the second one is resolved.
    expect(mergeRefreshedThreadMessages([first, second], [secondCopy], 't1').map((m) => m.id)).toEqual([
      'sent-1',
      'sent:b',
    ])
  })

  it('falls back to the closest send time when the server reflowed the body', () => {
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      body: 'A reply long enough to wrap',
      send_status: 'sent',
      date: 1000,
    })
    const canonical = thread({
      id: 'sent:1',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'abc@protonmail.internalid',
      // Same words, rewrapped by the submission server.
      body: 'A reply long enough\nto wrap',
      outgoing: true,
      date: 1003,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [canonical], 't1').map((m) => m.id)).toEqual(['sent:1'])
  })

  it('does not let an earlier reply of ours claim a bubble still waiting on its Message-ID', () => {
    // The bubble goes up at the click, before the thread load that back-fills
    // Message-IDs. That load can reveal a reply we sent minutes ago with the
    // same envelope; it cannot be this bubble's copy, since nothing has been
    // handed to SMTP yet.
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'Sender <sender@example.com>',
      subject: 'Re: Subject',
      message_id: '',
      body: 'Second reply',
      send_status: 'sending',
      date: 1000,
    })
    const earlier = thread({
      id: 'sent:1',
      folder_id: 'Sent',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'earlier@example.com',
      body: 'First reply',
      outgoing: true,
      date: 940,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [earlier], 't1').map((m) => m.id)).toEqual([
      'sent:1',
      'sent-local',
    ])
  })

  it('does not let a newly saved draft claim an optimistic reply', () => {
    // The autosaved draft of the *next* reply: outgoing, same envelope, minutes
    // apart — but a draft is not a sent copy.
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 1000,
    })
    const draft = thread({
      id: 'draft:1',
      folder_id: 'Drafts',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'sender@example.com',
      subject: 'Re: Subject',
      message_id: 'next-draft@example.com',
      outgoing: true,
      date: 1005,
    })

    expect(mergeRefreshedThreadMessages([optimistic], [draft], 't1').map((m) => m.id)).toEqual([
      'sent-local',
      'draft:1',
    ])
  })

  it('does not mistake the message being replied to for the Sent copy', () => {
    // A thread the user talks to themselves in: same sender, same recipients,
    // minutes apart. Only the reply is a local send.
    const original = thread({
      id: 'm1',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'me@example.com',
      subject: 'Re: Subject',
      message_id: 'root@example.com',
      outgoing: true,
      date: 1000,
    })
    const optimistic = thread({
      id: 'sent-local',
      thread_id: 't1',
      from_addr: 'me@example.com',
      to: 'me@example.com',
      subject: 'Re: Subject',
      message_id: 'reply@example.com',
      send_status: 'sent',
      date: 1060,
    })

    expect(mergeRefreshedThreadMessages([original, optimistic], [original], 't1')).toEqual([original, optimistic])
  })

  it('does not carry an optimistic reply into another thread', () => {
    const optimistic = thread({ id: 'sent-local', thread_id: 't1', send_status: 'sending' })
    const other = thread({ id: 'm2', thread_id: 't2' })

    expect(mergeRefreshedThreadMessages([optimistic], [other], 't2')).toEqual([other])
  })
})
