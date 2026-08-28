import { invoke } from './bridge'

/** Where the updater currently is. Mirrors the Go state machine in update.go. */
export type UpdateState = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'installing' | 'error'

/** How this copy of Meron was installed (see update_channel.go). */
export type UpdateChannel =
  | 'dmg'
  | 'mas'
  | 'appimage'
  | 'tarball'
  | 'nsis'
  | 'portable'
  | 'snap'
  | 'flatpak'
  | 'appx'
  | 'unknown'

export interface UpdateStatus {
  state: UpdateState
  channel: UpdateChannel
  /** A store (Mac App Store, Snap, Flathub, Microsoft Store) owns updates for this build. */
  managed: boolean
  /** Whether in-app updates work here at all. False for managed and dev builds. */
  supported: boolean
  currentVersion: string
  latestVersion: string
  pubDate: string
  downloaded: number
  total: number
  error: string
  releasesUrl: string
}

export function fetchUpdateStatus(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>('update.status')
}

export function checkForUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>('update.check')
}

/** Starts the download; progress arrives as `update.status` events. */
export function downloadUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>('update.download')
}

/** Applies the downloaded update. On success the app quits and relaunches. */
export function installUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>('update.install')
}
