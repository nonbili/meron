import { describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { mergeStarredItems } from './starredItems'

const item = (overrides: Partial<Message>): Message =>
  ({
    id: 'x',
    subject: '',
    preview: '',
    from_name: '',
    from_addr: '',
    date: 0,
    starred: true,
    ...overrides,
  }) as Message

const epoch = (iso: string) => Math.floor(Date.parse(iso) / 1000)

const items = [
  item({ id: 'old-mail', subject: 'Quarterly plan', date: epoch('2026-01-01') }),
  item({ id: 'new-feed', subject: 'Release notes', preview: 'v2 shipped', date: epoch('2026-03-01') }),
  item({ id: 'mid-mail', subject: 'Hello', from_name: 'Launch Team', date: epoch('2026-02-01') }),
]

describe('mergeStarredItems', () => {
  it('sorts newest first', () => {
    expect(mergeStarredItems(items, '').map((i) => i.id)).toEqual(['new-feed', 'mid-mail', 'old-mail'])
  })

  it('filters by subject, preview, and sender case-insensitively', () => {
    expect(mergeStarredItems(items, 'quarterly').map((i) => i.id)).toEqual(['old-mail'])
    expect(mergeStarredItems(items, 'SHIPPED').map((i) => i.id)).toEqual(['new-feed'])
    expect(mergeStarredItems(items, 'launch').map((i) => i.id)).toEqual(['mid-mail'])
    expect(mergeStarredItems(items, 'nomatch')).toEqual([])
  })

  it('tolerates missing fields', () => {
    const sparse = [item({ id: 'sparse', preview: undefined as unknown as string })]
    expect(mergeStarredItems(sparse, 'anything')).toEqual([])
    expect(mergeStarredItems(sparse, '').map((i) => i.id)).toEqual(['sparse'])
  })
})
