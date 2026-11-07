import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { mail$ } from './mail'
import { ui$ } from './ui'
import { appendSentMessage } from './compose'

const thread: Message = {
  id: 'root',
  account_id: 'acc',
  folder_id: 'INBOX',
  thread_id: 't1',
  from_name: 'Aki',
  from_addr: 'aki@example.com',
  to: 'me@example.com',
  subject: 'Topic',
  preview: '',
  body: '',
  date: 100,
  unread: false,
  starred: false,
  has_attachments: false,
}

const send = (encrypt: boolean) =>
  appendSentMessage({
    threadId: 't1',
    accountId: 'acc',
    from: 'me@example.com',
    to: 'aki@example.com',
    subject: 'Re: Topic',
    rich: false,
    content: encrypt ? 'secret' : 'plain',
    attachments: [],
    encrypt,
  })

describe('appendSentMessage', () => {
  beforeEach(() => {
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t1')
  })
  afterEach(() => {
    mail$.threads.set([])
    mail$.messages.set([])
    ui$.selectedThread.set('')
  })

  it('shows an encrypted send from the full composer with our own verified lock at once', () => {
    send(true)
    send(false)
    const bubbles = mail$.messages.get().filter((m) => m.id !== 'root')
    expect(bubbles.find((m) => m.body === 'secret')?.crypto).toMatchObject({
      encrypted: true,
      own: true,
      trust: 'verified',
    })
    expect(bubbles.find((m) => m.body === 'plain')?.crypto ?? null).toBeNull()
  })
})
