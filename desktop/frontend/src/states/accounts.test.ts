import { beforeEach, describe, expect, it } from 'bun:test'
import { accounts$, setAccountSignature } from './accounts'
import type { Account } from '../types'

const account = (id: string): Account => ({
  id,
  email: `${id}@example.com`,
  display_name: id,
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'imap.example.com',
  imap_port: 993,
  smtp_host: 'smtp.example.com',
  smtp_port: 465,
  tls: true,
})

describe('setAccountSignature', () => {
  let completed: string[] = []
  let gate: (() => void) | null = null
  // Which call should fail, decided per test: (payload, callIndex) => boolean.
  let failWhen: (payload: any, call: number) => boolean = () => false

  beforeEach(() => {
    completed = []
    gate = null
    failWhen = () => false
    accounts$.set([account('a'), account('b')])
    let calls = 0
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            if (command !== 'account.setSignature') return {}
            calls += 1
            const first = calls === 1
            if (first && gate) {
              await new Promise<void>((resolve) => {
                gate = resolve
              })
            }
            if (failWhen(payload, calls)) throw new Error('write failed')
            completed.push(payload.signature?.mode ?? 'cleared')
            return {}
          },
        },
      },
    }
  })

  it('lands writes for one account in the order they were made', async () => {
    // The settings card fires two in a row: the mode the user picked, then the
    // text its editor had pending. Unordered, the older content wins the race.
    gate = () => {}
    const slow = setAccountSignature('a', { mode: 'none', html: '<p>Mine</p>' })
    const fast = setAccountSignature('a', { mode: 'custom', html: '<p>Mine, edited</p>' })

    gate?.()
    gate = null
    await Promise.all([slow, fast])

    expect(completed).toEqual(['none', 'custom'])
    expect(accounts$.peek()[0].signature).toEqual({ mode: 'custom', html: '<p>Mine, edited</p>' })
  })

  it('does not roll a failed write back over a newer one that succeeded', async () => {
    gate = () => {}
    // Only the first (slow) write fails; the second one lands and must stand.
    failWhen = (_payload, call) => call === 1
    const failing = setAccountSignature('a', { mode: 'none', html: '<p>Mine</p>' })
    const succeeding = setAccountSignature('a', { mode: 'custom', html: '<p>Mine, edited</p>' })

    gate?.()
    gate = null
    await Promise.all([failing, succeeding])

    expect(accounts$.peek()[0].signature).toEqual({ mode: 'custom', html: '<p>Mine, edited</p>' })
  })

  it('falls back to what the store holds, not to a value that never reached it', async () => {
    // Old -> None (fails) -> Custom (fails). Rolling back to "what was on
    // screen when I started" would leave the UI on None, which was never saved.
    accounts$.set([{ ...account('a'), signature: { mode: 'custom', html: '<p>Old</p>' } }, account('b')])
    failWhen = () => true

    // Queued together, as the settings card does: the second must not treat the
    // first one's optimistic value as the baseline to fall back to.
    const first = setAccountSignature('a', { mode: 'none', html: '<p>Old</p>' })
    const second = setAccountSignature('a', { mode: 'custom', html: '<p>Newer</p>' })
    await Promise.all([first, second])

    expect(accounts$.peek()[0].signature).toEqual({ mode: 'custom', html: '<p>Old</p>' })
  })

  it('leaves other accounts untouched when a write fails', async () => {
    await setAccountSignature('b', { mode: 'custom', html: '<p>B</p>' })
    failWhen = (payload) => payload.id === 'a'
    await setAccountSignature('a', { mode: 'custom', html: '<p>A</p>' })

    expect(accounts$.peek()[1].signature).toEqual({ mode: 'custom', html: '<p>B</p>' })
  })
})
