import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { mail$ } from './mail'
import { replySubject } from './composeReply'

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'm1',
  account_id: 'acc',
  folder_id: 'inbox',
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
  ...overrides,
})

describe('replySubject', () => {
  beforeEach(() => mail$.messages.set([]))
  afterEach(() => mail$.messages.set([]))

  it('prefixes a plain subject once, as before', () => {
    expect(replySubject('Topic')).toBe('Re: Topic')
    expect(replySubject('Re: Topic')).toBe('Re: Topic')
    expect(replySubject('')).toBe('Re: ')
  })

  it('takes the decrypted subject of the message being answered instead of a placeholder', () => {
    expect(replySubject('[...]', message({ subject: 'Testing This' }))).toBe('Re: Testing This')
    expect(replySubject('...', message({ subject: 'Re: Testing This' }))).toBe('Re: Testing This')
    expect(replySubject('Re: [...]', message({ subject: 'Testing This' }))).toBe('Re: Testing This')
  })

  it('never takes a subject from another loaded thread', () => {
    // On the Mac a reply in one thread went out as "Re: Testing This", the
    // subject of a different thread.
    mail$.messages.set([message({ id: 'x', subject: 'Testing This', date: 10 })])
    expect(replySubject('[...]', message({ id: 'y', subject: '[...]', date: 200 }))).toBe('')
  })

  it('sends no subject made from the placeholder when the answered message is not decrypted', () => {
    expect(replySubject('[...]', message({ subject: 'Re: [...]' }))).toBe('')
    expect(replySubject('[...]')).toBe('')
  })
})
