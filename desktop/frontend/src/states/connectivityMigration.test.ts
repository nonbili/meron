import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { connectivity$, watchCoreMigration } from './connectivity'

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))
const NOTICE_WAIT = 600

let handlers: Record<string, () => void>
let status: Promise<{ migrating: boolean }>
let stop: (() => void) | undefined

beforeEach(() => {
  handlers = {}
  status = Promise.resolve({ migrating: false })
  connectivity$.migrating.set(false)
  ;(globalThis as any).window = globalThis
  ;(window as any).runtime = {
    EventsOn: (name: string, handler: () => void) => {
      handlers[name] = handler
      return () => {}
    },
  }
  ;(window as any).go = { main: { App: { Invoke: () => status } } }
})

afterEach(() => {
  stop?.()
  delete (window as any).runtime
  delete (window as any).go
})

describe('watchCoreMigration', () => {
  test('shows a slow upgrade and clears it when done', async () => {
    stop = watchCoreMigration()
    handlers['core.migrating']()
    expect(connectivity$.migrating.get()).toBe(false)
    await delay(NOTICE_WAIT)
    expect(connectivity$.migrating.get()).toBe(true)
    handlers['core.migrationDone']()
    expect(connectivity$.migrating.get()).toBe(false)
  })

  test('a quick upgrade never shows', async () => {
    stop = watchCoreMigration()
    handlers['core.migrating']()
    handlers['core.migrationDone']()
    await delay(NOTICE_WAIT)
    expect(connectivity$.migrating.get()).toBe(false)
  })

  test('picks up an upgrade that began before the window subscribed', async () => {
    status = Promise.resolve({ migrating: true })
    stop = watchCoreMigration()
    await delay(NOTICE_WAIT)
    expect(connectivity$.migrating.get()).toBe(true)
  })

  test('a late status reply does not reopen a finished upgrade', async () => {
    let reply = (_: { migrating: boolean }) => {}
    status = new Promise((resolve) => (reply = resolve))
    stop = watchCoreMigration()
    handlers['core.migrationDone']()
    reply({ migrating: true })
    await delay(NOTICE_WAIT)
    expect(connectivity$.migrating.get()).toBe(false)
  })

  test('a status reply after cleanup does nothing', async () => {
    let reply = (_: { migrating: boolean }) => {}
    status = new Promise((resolve) => (reply = resolve))
    watchCoreMigration()?.()
    reply({ migrating: true })
    await delay(NOTICE_WAIT)
    expect(connectivity$.migrating.get()).toBe(false)
  })
})
