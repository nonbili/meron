import { describe, expect, it } from 'bun:test'
import type { Account, Message } from '../types'
import { filterThreads, isRssAccount } from './threadActions'

const thread = (overrides: Partial<Message>): Message =>
  ({ thread_id: 't', unread: false, starred: false, ...overrides }) as Message

const threads = [
  thread({ thread_id: 'a', unread: true }),
  thread({ thread_id: 'b', starred: true }),
  thread({ thread_id: 'c' }),
]

describe('filterThreads', () => {
  it("returns all threads for mode 'all'", () => {
    expect(filterThreads(threads, 'all')).toEqual(threads)
  })

  it("keeps only unread threads for mode 'unread'", () => {
    expect(filterThreads(threads, 'unread').map((t) => t.thread_id)).toEqual(['a'])
  })

  it("keeps only starred threads for mode 'starred'", () => {
    expect(filterThreads(threads, 'starred').map((t) => t.thread_id)).toEqual(['b'])
  })

  it('keeps RSS feeds containing starred items without marking the feed starred', () => {
    const feed = thread({ thread_id: 'feed', starred: false, has_starred_items: true })
    expect(filterThreads([...threads, feed], 'starred').map((t) => t.thread_id)).toEqual(['b', 'feed'])
  })

  it('keeps the open thread visible via keepId even when it no longer matches', () => {
    expect(filterThreads(threads, 'unread', 'c').map((t) => t.thread_id)).toEqual(['a', 'c'])
  })

  it('keeps threads listed in keepIds', () => {
    expect(filterThreads(threads, 'starred', undefined, { c: true }).map((t) => t.thread_id)).toEqual(['b', 'c'])
  })
})

describe('isRssAccount', () => {
  const account = (overrides: Partial<Account>): Account => ({
    id: 'acc1',
    email: 'me@example.com',
    display_name: 'Me',
    provider: 'imap',
    auth_type: 'password',
    imap_host: '',
    imap_port: 993,
    smtp_host: '',
    smtp_port: 465,
    tls: true,
    ...overrides,
  })

  it('detects rss provider, rss auth_type, and rss- id prefix', () => {
    expect(isRssAccount(account({ provider: 'rss' }), 'x')).toBe(true)
    expect(isRssAccount(account({ auth_type: 'rss' }), 'x')).toBe(true)
    expect(isRssAccount(undefined, 'rss-feeds')).toBe(true)
  })

  it('is false for a plain mail account', () => {
    expect(isRssAccount(account({ provider: 'gmail', auth_type: 'gmail_oauth' }), 'acc1')).toBe(false)
  })
})
