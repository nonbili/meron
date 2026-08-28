import { beforeEach, describe, expect, it } from 'bun:test'
import { closeComposeSession, forgetComposeSession } from './composeSessions'

describe('compose session close', () => {
  const tabId = 'restored-compose'

  beforeEach(() => forgetComposeSession(tabId))

  it('deduplicates concurrent fallback closes', async () => {
    let finish!: () => void
    const pending = new Promise<void>((resolve) => {
      finish = resolve
    })
    let closes = 0
    const fallback = async () => {
      closes += 1
      await pending
    }

    const first = closeComposeSession(tabId, fallback)
    const second = closeComposeSession(tabId, fallback)
    expect(first).toBe(second)
    expect(closes).toBe(1)

    finish()
    await first
  })
})
