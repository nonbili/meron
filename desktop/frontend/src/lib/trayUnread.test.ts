import { describe, expect, test } from 'bun:test'
import { createTrayUnreadUpdater, trayUnreadPayload } from './trayUnread'

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

describe('tray unread updater', () => {
  test('serializes native updates and finishes on the newest state', async () => {
    const first = deferred()
    const sent: number[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) await first.promise
    })

    update(0)
    update(1)
    expect(sent).toEqual([0])

    first.resolve()
    await tick()
    expect(sent).toEqual([0, 1])
  })

  test('coalesces intermediate states while an update is in flight', async () => {
    const first = deferred()
    const sent: number[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) await first.promise
    })

    update(1)
    update(0)
    update(1)
    update(0)
    first.resolve()
    await tick()

    expect(sent).toEqual([1, 0])
  })

  test('does not lose a newer state when the in-flight update fails', async () => {
    const first = deferred()
    const sent: number[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) {
        await first.promise
        throw new Error('native update failed')
      }
    })

    update(0)
    update(1)
    first.resolve()
    await tick()

    expect(sent).toEqual([0, 1])
  })

  test('retries the current state once after a transient failure', async () => {
    const sent: number[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) throw new Error('native update failed')
    })

    update(1)
    await tick()

    expect(sent).toEqual([1, 1])
  })

  test('stops after one retry when the failure persists', async () => {
    const sent: number[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      throw new Error('native update failed')
    })

    update(1)
    await tick()

    expect(sent).toEqual([1, 1])
  })

  test('coalesces in-flight duplicates and forwards later refreshes for cache repair', async () => {
    const first = deferred()
    const sent: number[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      await first.promise
    })
    update(1)
    update(1)
    first.resolve()
    await tick()
    update(1)
    await tick()
    expect(sent).toEqual([1, 1])
  })

  test('sends the inbox unread count for the Dock badge with the tray flag', () => {
    expect(trayUnreadPayload(0)).toEqual({ unread: false, count: 0 })
    expect(trayUnreadPayload(5)).toEqual({ unread: true, count: 5 })
  })
})
