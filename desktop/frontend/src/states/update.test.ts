import { describe, expect, it, beforeEach, afterEach } from 'bun:test'
import type { UpdateStatus } from '../lib/update'
import {
  EMPTY_UPDATE_STATUS,
  applyUpdateStatus,
  loadUpdateStatus,
  runUpdateCheck,
  shouldShowUpdateBanner,
  update$,
} from './update'
import { ui$ } from './ui'

function status(partial: Partial<UpdateStatus>): UpdateStatus {
  return { ...EMPTY_UPDATE_STATUS, ...partial }
}

describe('applyUpdateStatus', () => {
  beforeEach(() => {
    update$.status.set(EMPTY_UPDATE_STATUS)
  })

  it('replaces the whole status', () => {
    applyUpdateStatus(status({ state: 'available', latestVersion: '0.1.13', supported: true }))
    expect(update$.status.state.get()).toBe('available')
    expect(update$.status.latestVersion.get()).toBe('0.1.13')
  })

  it('fills in fields an older backend omits', () => {
    applyUpdateStatus({ state: 'downloading', downloaded: 10 } as UpdateStatus)
    expect(update$.status.total.get()).toBe(0)
    expect(update$.status.channel.get()).toBe('unknown')
  })

  it('ignores garbage rather than blanking a good status', () => {
    applyUpdateStatus(status({ state: 'ready', latestVersion: '0.1.13' }))
    applyUpdateStatus(null)
    applyUpdateStatus(undefined)
    applyUpdateStatus({} as UpdateStatus)
    expect(update$.status.state.get()).toBe('ready')
  })
})

describe('restored installation failure', () => {
  afterEach(() => {
    delete (window as any).go
    update$.status.set(EMPTY_UPDATE_STATUS)
    ui$.toast.set('')
    ui$.toastTone.set('success')
  })

  it('announces the failure at startup and retains it after background polling', async () => {
    const message = 'Windows update failed: elevation cancelled'
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string) =>
            status({
              supported: true,
              state: command === 'update.status' ? 'error' : 'available',
              error: command === 'update.status' ? message : '',
              installError: message,
              latestVersion: '0.1.13',
            }),
        },
      },
    }
    await loadUpdateStatus()
    expect(ui$.toast.get()).toBe(message)
    expect(ui$.toastTone.get()).toBe('error')
    await runUpdateCheck()
    expect(update$.status.state.get()).toBe('available')
    expect(update$.status.installError.get()).toBe(message)
  })

  it('does not announce a normal startup', async () => {
    ui$.toast.set('')
    ;(window as any).go = { main: { App: { Invoke: async () => status({ state: 'idle' }) } } }
    await loadUpdateStatus()
    expect(ui$.toast.get()).toBe('')
  })
})

describe('shouldShowUpdateBanner', () => {
  it('shows for an actionable update the user has not dismissed', () => {
    expect(shouldShowUpdateBanner(status({ supported: true, state: 'available', latestVersion: '0.1.13' }), null)).toBe(
      true,
    )
    expect(shouldShowUpdateBanner(status({ supported: true, state: 'ready', latestVersion: '0.1.13' }), null)).toBe(
      true,
    )
  })

  it('stays hidden once that version is dismissed', () => {
    const available = status({ supported: true, state: 'available', latestVersion: '0.1.13' })
    expect(shouldShowUpdateBanner(available, '0.1.13')).toBe(false)
    // A release other than the dismissed one is worth announcing again.
    expect(shouldShowUpdateBanner(available, '0.1.12')).toBe(true)
    expect(shouldShowUpdateBanner(status({ ...available, latestVersion: '0.1.14' }), '0.1.13')).toBe(true)
  })

  it('stays hidden where updates cannot be applied', () => {
    expect(
      shouldShowUpdateBanner(
        status({ supported: false, managed: true, state: 'available', latestVersion: '0.1.13' }),
        null,
      ),
    ).toBe(false)
  })

  it('stays hidden for states with nothing to click', () => {
    for (const state of ['idle', 'checking', 'downloading', 'installing', 'error'] as const) {
      expect(shouldShowUpdateBanner(status({ supported: true, state, latestVersion: '0.1.13' }), null)).toBe(false)
    }
  })
})
