import { invoke } from '../lib/bridge'
import { boot } from '../boot'
import { ui$ } from './ui'
import { loadThreads, loadFolders } from './mail'

// Backup / restore of the app's configuration: accounts and their connection
// settings, per-account prefs, RSS subscriptions and the settings table. Cached
// mail is deliberately not included — it re-syncs from the server — so the file
// stays small and portable between machines.
//
// The bridge owns the file dialogs and the filesystem; this module only shapes
// the calls and reports the outcome.

/** Counts returned by a successful restore. */
export type BackupSummary = {
  accounts: number
  /** Accounts already present locally, which a restore never overwrites. */
  skipped: number
  feeds: number
  settings: number
  secrets: number
}

type ExportResult = { saved: boolean; path?: string; encrypted?: boolean }
type ImportResult = Partial<BackupSummary> & { cancelled?: boolean; needsPassphrase?: boolean; path?: string }

/**
 * Write a backup to a file the user picks.
 *
 * A passphrase encrypts the payload; `includeSecrets` additionally puts account
 * passwords and OAuth tokens inside it, which the core refuses to do without
 * one. Returns whether a file was actually written (false = cancelled).
 */
export async function exportBackup(includeSecrets: boolean, passphrase: string): Promise<boolean> {
  const res = await invoke<ExportResult>('backup.export', {
    include_secrets: includeSecrets,
    passphrase,
  })
  return res?.saved === true
}

/**
 * The outcome of a restore attempt.
 * 'needs-passphrase' carries the path of the file already chosen, so the retry
 * can decrypt it without making the user pick the same file again.
 */
export type ImportOutcome =
  | { status: 'done'; summary: BackupSummary }
  | { status: 'cancelled' }
  | { status: 'needs-passphrase'; path: string }

/**
 * Restore a backup. Called with no arguments it opens a file dialog; called
 * with a `path` (from a previous 'needs-passphrase' result) it reuses that file.
 *
 * On success the app is re-bootstrapped, since restored accounts and settings
 * change essentially every piece of loaded state.
 */
export async function importBackup(path?: string, passphrase = ''): Promise<ImportOutcome> {
  const res = await invoke<ImportResult>('backup.import', { path: path ?? '', passphrase })
  if (res?.cancelled) return { status: 'cancelled' }
  if (res?.needsPassphrase) return { status: 'needs-passphrase', path: res.path ?? path ?? '' }

  const summary: BackupSummary = {
    accounts: res?.accounts ?? 0,
    skipped: res?.skipped ?? 0,
    feeds: res?.feeds ?? 0,
    settings: res?.settings ?? 0,
    secrets: res?.secrets ?? 0,
  }
  // Restored rows land straight in the DB, so every cached view is stale:
  // re-bootstrap for accounts + settings, then refresh the open mail panes.
  await boot()
  await Promise.all([loadThreads(false), loadFolders(ui$.selectedAccount.get(), false)]).catch(() => {})
  return { status: 'done', summary }
}

/** Turn an unknown thrown value into a message suitable for a toast. */
export function backupErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : typeof error === 'string' ? error : fallback
}

/** Whether a restore error means the passphrase was wrong (worth re-prompting). */
export function isWrongPassphrase(message: string): boolean {
  return message.includes('wrong passphrase')
}
