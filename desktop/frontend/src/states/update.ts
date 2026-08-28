import { observable } from '@legendapp/state'
import { checkForUpdate, downloadUpdate, fetchUpdateStatus, installUpdate, type UpdateStatus } from '../lib/update'
import { settings$ } from './settings'

// In-app updater state. The Go side owns the state machine and pushes the whole
// status on every transition (the `update.status` event, wired up in
// useAppEffects); this observable is just the latest snapshot. Nothing here
// decides when to update — the user does, from AboutDialog / UpdateBanner.

export const EMPTY_UPDATE_STATUS: UpdateStatus = {
  state: 'idle',
  channel: 'unknown',
  managed: false,
  supported: false,
  currentVersion: '',
  latestVersion: '',
  pubDate: '',
  downloaded: 0,
  total: 0,
  error: '',
  releasesUrl: '',
}

export const update$ = observable<{ status: UpdateStatus }>({ status: EMPTY_UPDATE_STATUS })

export function applyUpdateStatus(status: UpdateStatus | null | undefined) {
  if (!status || typeof status.state !== 'string') return
  update$.status.set({ ...EMPTY_UPDATE_STATUS, ...status })
}

/** Load the current status without hitting the network. */
export async function loadUpdateStatus() {
  try {
    applyUpdateStatus(await fetchUpdateStatus())
  } catch {
    // No backend (browser dev) or an old build without the command: stay idle.
  }
}

/** Ask the backend to fetch the manifest and compare versions. */
export async function runUpdateCheck() {
  try {
    applyUpdateStatus(await checkForUpdate())
  } catch {
    // Network failures are already reflected in the status the backend emits.
  }
}

export async function startUpdateDownload() {
  try {
    applyUpdateStatus(await downloadUpdate())
  } catch {
    // Same: the error state arrives via the event.
  }
}

export async function applyDownloadedUpdate() {
  try {
    applyUpdateStatus(await installUpdate())
  } catch {
    // Same.
  }
}

/**
 * Whether the "an update is ready" banner should show: an update the user has
 * neither started nor dismissed. Once they dismiss it, About is where they can
 * still get to it.
 */
export function shouldShowUpdateBanner(status: UpdateStatus, dismissedVersion: string | null): boolean {
  if (!status.supported) return false
  if (status.state !== 'available' && status.state !== 'ready') return false
  if (!status.latestVersion) return false
  return dismissedVersion !== status.latestVersion
}

export function dismissUpdateBanner() {
  const version = update$.status.latestVersion.peek()
  if (version) settings$.dismissedUpdateVersion.set(version)
}
