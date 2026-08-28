import { describe, expect, test } from 'bun:test'
import { createTrayUnreadUpdater } from './trayUnread'

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
    const sent: boolean[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) await first.promise
    })

    update(false)
    update(true)
    expect(sent).toEqual([false])

    first.resolve()
    await tick()
    expect(sent).toEqual([false, true])
  })

  test('coalesces intermediate states while an update is in flight', async () => {
    const first = deferred()
    const sent: boolean[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) await first.promise
    })

    update(true)
    update(false)
    update(true)
    update(false)
    first.resolve()
    await tick()

    expect(sent).toEqual([true, false])
  })

  test('does not lose a newer state when the in-flight update fails', async () => {
    const first = deferred()
    const sent: boolean[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) {
        await first.promise
        throw new Error('native update failed')
      }
    })

    update(false)
    update(true)
    first.resolve()
    await tick()

    expect(sent).toEqual([false, true])
  })

  test('retries the current state once after a transient failure', async () => {
    const sent: boolean[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      if (sent.length === 1) throw new Error('native update failed')
    })

    update(true)
    await tick()

    expect(sent).toEqual([true, true])
  })

  test('stops after one retry when the failure persists', async () => {
    const sent: boolean[] = []
    const update = createTrayUnreadUpdater(async (unread) => {
      sent.push(unread)
      throw new Error('native update failed')
    })

    update(true)
    await tick()

    expect(sent).toEqual([true, true])
  })
})
