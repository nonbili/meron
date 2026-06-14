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

  it('keeps the open thread visible via keepId even when it no longer matches', () => {
    expect(filterThreads(threads, 'unread', 'c').map((t) => t.thread_id)).toEqual(['a', 'c'])
  })

  it('keeps threads listed in keepIds', () => {
    expect(filterThreads(threads, 'starred', undefined, { c: true }).map((t) => t.thread_id)).toEqual(['b', 'c'])
  })
})

describe('isRssAccount', () => {
  it('detects rss provider, rss auth_type, and rss- id prefix', () => {
    expect(isRssAccount({ provider: 'rss' } as Account, 'x')).toBe(true)
    expect(isRssAccount({ auth_type: 'rss' } as Account, 'x')).toBe(true)
    expect(isRssAccount(undefined, 'rss-feeds')).toBe(true)
  })

  it('is false for a plain mail account', () => {
    expect(isRssAccount({ provider: 'gmail', auth_type: 'oauth' } as Account, 'acc1')).toBe(false)
  })
})
