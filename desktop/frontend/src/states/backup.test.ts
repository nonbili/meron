import { beforeEach, describe, expect, it } from 'bun:test'
import { backupErrorMessage, exportBackup, importBackup, isWrongPassphrase } from './backup'
import { accounts$ } from './accounts'
import { mail$ } from './mail'
import { ui$ } from './ui'

type Call = { command: string; payload: any }

/** Stub the Wails bridge with a per-command response map. */
function stubBridge(calls: Call[], responses: Record<string, unknown>) {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: unknown) => {
          calls.push({ command, payload })
          const response = responses[command]
          if (response instanceof Error) throw response
          return response ?? { ok: true }
        },
      },
    },
  }
}

/** The calls `importBackup` makes to re-bootstrap after a successful restore. */
const bootResponses = {
  'system.check': {},
  'account.list': { accounts: [] },
  'app.prefsGet': { prefs: {} },
  'mailto.consumePending': [],
  'mail.threadList': { threads: [] },
  'mail.folderList': { folders: [] },
}

describe('exportBackup', () => {
  const calls: Call[] = []

  beforeEach(() => {
    calls.length = 0
  })

  it('passes the passphrase and secrets flag through to the bridge', async () => {
    stubBridge(calls, { 'backup.export': { saved: true, path: '/tmp/meron-backup.json' } })

    await expect(exportBackup(true, 'correct horse')).resolves.toBe(true)
    expect(calls[0]).toEqual({
      command: 'backup.export',
      payload: { include_secrets: true, passphrase: 'correct horse' },
    })
  })

  it('reports a cancelled save dialog as not saved', async () => {
    stubBridge(calls, { 'backup.export': { saved: false } })

    await expect(exportBackup(false, '')).resolves.toBe(false)
  })

  it('propagates a bridge failure so the caller can surface it', async () => {
    stubBridge(calls, { 'backup.export': new Error('write backup: permission denied') })

    await expect(exportBackup(false, '')).rejects.toThrow('permission denied')
  })
})

describe('importBackup', () => {
  const calls: Call[] = []

  beforeEach(() => {
    calls.length = 0
    accounts$.set([])
    mail$.threads.set([])
    ui$.selectedAccount.set('')
  })

  it('opens a file dialog when called with no path', async () => {
    stubBridge(calls, {
      ...bootResponses,
      'backup.import': { accounts: 2, skipped: 1, feeds: 3, settings: 4, secrets: 2 },
    })

    const outcome = await importBackup()

    expect(calls[0]).toEqual({ command: 'backup.import', payload: { path: '', passphrase: '' } })
    expect(outcome).toEqual({
      status: 'done',
      summary: { accounts: 2, skipped: 1, feeds: 3, settings: 4, secrets: 2 },
    })
  })

  it('re-bootstraps the app after a successful restore', async () => {
    stubBridge(calls, { ...bootResponses, 'backup.import': { accounts: 1 } })

    await importBackup()

    // Restored accounts and settings only reach the UI through a fresh boot.
    const commands = calls.map((call) => call.command)
    expect(commands).toContain('account.list')
    expect(commands).toContain('app.prefsGet')
  })

  it('reports a cancelled file dialog without re-bootstrapping', async () => {
    stubBridge(calls, { ...bootResponses, 'backup.import': { cancelled: true } })

    await expect(importBackup()).resolves.toEqual({ status: 'cancelled' })
    expect(calls.map((call) => call.command)).toEqual(['backup.import'])
  })

  it('returns the chosen path when the file turns out to be encrypted', async () => {
    stubBridge(calls, {
      ...bootResponses,
      'backup.import': { needsPassphrase: true, path: '/tmp/secret-backup.json' },
    })

    const outcome = await importBackup()

    expect(outcome).toEqual({ status: 'needs-passphrase', path: '/tmp/secret-backup.json' })
    // No boot: nothing was restored.
    expect(calls.map((call) => call.command)).toEqual(['backup.import'])
  })

  it('reuses the path on the retry so the file dialog is not shown twice', async () => {
    stubBridge(calls, { ...bootResponses, 'backup.import': { accounts: 1 } })

    await importBackup('/tmp/secret-backup.json', 'correct horse')

    expect(calls[0].payload).toEqual({ path: '/tmp/secret-backup.json', passphrase: 'correct horse' })
  })

  it('defaults every missing count to zero', async () => {
    stubBridge(calls, { ...bootResponses, 'backup.import': {} })

    const outcome = await importBackup()

    expect(outcome).toEqual({
      status: 'done',
      summary: { accounts: 0, skipped: 0, feeds: 0, settings: 0, secrets: 0 },
    })
  })
})

describe('error helpers', () => {
  it('unwraps Errors, strings and unknown values', () => {
    expect(backupErrorMessage(new Error('boom'), 'fallback')).toBe('boom')
    expect(backupErrorMessage('plain string', 'fallback')).toBe('plain string')
    expect(backupErrorMessage(undefined, 'fallback')).toBe('fallback')
    expect(backupErrorMessage({ code: 5 }, 'fallback')).toBe('fallback')
  })

  it('recognizes the core wrong-passphrase message', () => {
    expect(isWrongPassphrase('wrong passphrase, or the backup file is damaged')).toBe(true)
    expect(isWrongPassphrase('this backup is encrypted; a passphrase is required')).toBe(false)
    expect(isWrongPassphrase('read backup: no such file')).toBe(false)
  })
})
