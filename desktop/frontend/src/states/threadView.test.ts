import { afterEach, describe, expect, it } from 'bun:test'
import { revealRemote, thread$ } from './thread'
import {
  forgetThreadView,
  forgetThreadViews,
  retainThreadView,
  restoreThreadView,
  snapshotThreadView,
  threadView,
} from './threadView'

afterEach(() => forgetThreadViews())

describe('threadView', () => {
  it('carries a thread view through a plain-data snapshot', () => {
    const view = threadView('t1')
    view.overrides = { a: true, b: false }
    view.expandAll = { expanded: true }
    view.unreadOnArrival.add('c')
    view.scroll = { messageId: 'b', offset: 12, atBottom: false }
    revealRemote('a')
    revealRemote('elsewhere')

    const snapshot = JSON.parse(JSON.stringify(snapshotThreadView('t1', ['a', 'b', 'c'])))
    expect(snapshot.revealedRemote).toEqual(['a'])

    forgetThreadViews()
    restoreThreadView('t1', snapshot)
    const restored = threadView('t1')
    expect(restored.overrides).toEqual({ a: true, b: false })
    expect(restored.expandAll).toEqual({ expanded: true })
    expect([...restored.unreadOnArrival]).toEqual(['c'])
    expect(restored.scroll).toEqual({ messageId: 'b', offset: 12, atBottom: false })
    // Opening the thread goes back to where it was left.
    expect(restored.restoreScroll).toBe(true)
    expect(thread$.revealedRemote.get()).toEqual({ a: true })
  })

  it('evicts the least recently used navigation entry at 200 threads', () => {
    threadView('oldest').overrides = { m: true }
    threadView('kept').overrides = { m: true }
    for (let i = 0; i < 199; i++) threadView(`t${i}`)
    threadView('kept')
    threadView('one-more')
    expect(threadView('oldest').overrides).toEqual({})
    expect(threadView('kept').overrides).toEqual({ m: true })
  })
})

it('protects mounted navigation and tab views without limiting alive tabs', async () => {
  const current = threadView('current')
  current.overrides = { current: true }
  const releaseCurrent = retainThreadView('current')
  const releases: (() => void)[] = []
  for (let i = 0; i < 250; i++) {
    const key = `tab-${i}:thread-${i}`
    releases.push(retainThreadView(key, true))
    threadView(key).overrides = { tab: true }
  }
  for (let i = 0; i < 300; i++) threadView(`navigation-${i}`)
  expect(threadView('current') === current).toBe(true)
  expect(threadView('tab-0:thread-0').overrides).toEqual({ tab: true })
  expect(threadView('tab-249:thread-249').overrides).toEqual({ tab: true })
  releases.forEach((release) => release())
  releaseCurrent()
  await Promise.resolve()
})

it('makes unmounted Current views eligible for eviction again', async () => {
  const release = retainThreadView('current')
  threadView('current').overrides = { remembered: true }
  release()
  await Promise.resolve()
  for (let i = 0; i < 200; i++) threadView(`navigation-${i}`)
  expect(threadView('current').overrides).toEqual({})
})

it('deletes a closing tab after its scroll cleanup has finished', async () => {
  const key = 'tab:thread'
  const release = retainThreadView(key, true)
  forgetThreadView(key)
  // A child's layout cleanup saves after tab removal, before ownership releases.
  threadView(key).scroll = { messageId: 'm', offset: 12, atBottom: false }
  release()
  await Promise.resolve()
  expect(threadView(key).scroll).toBeNull()
})

it('deletes an unmounted tab immediately when it closes', async () => {
  const key = 'tab:thread'
  const release = retainThreadView(key, true)
  threadView(key).overrides = { m: true }
  release()
  await Promise.resolve()
  forgetThreadView(key)
  expect(threadView(key).overrides).toEqual({})
})
