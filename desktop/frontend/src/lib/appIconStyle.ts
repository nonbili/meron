import { invoke } from './bridge'
import { createLatestValueUpdater } from './latestValueUpdater'

/**
 * Which icon the app shows: the running app's on macOS and Windows, the
 * launcher's on Linux. 'auto' is the light grey icon while the
 * system is light and the dark grey one while it is dark. The backend derives
 * the blue and grey icons from the shipped artwork and owns the saved choice.
 */
export type AppIconStyle = 'default' | 'blue' | 'light' | 'dark' | 'auto'

export const APP_ICON_STYLES: readonly AppIconStyle[] = ['default', 'blue', 'light', 'dark', 'auto']

/** Narrows whatever the backend sent; anything unknown is the shipped icon. */
export function parseAppIconStyle(value: unknown): AppIconStyle {
  return APP_ICON_STYLES.includes(value as AppIconStyle) ? (value as AppIconStyle) : 'default'
}

/**
 * What the choice reaches differs by platform, and the hint says so: the Dock
 * on macOS, the launcher on Linux, the taskbar on Windows.
 */
export function appIconHintKey(platform: string) {
  if (platform === 'linux') return 'settings.appearance.appIconHintLinux'
  if (platform === 'windows') return 'settings.appearance.appIconHintWindows'
  return 'settings.appearance.appIconHint'
}

export function createAppIconStyleUpdater(send = (style: AppIconStyle) => invoke('appIcon.setStyle', { style })) {
  return createLatestValueUpdater(send)
}

export const setAppIconStyle = createAppIconStyleUpdater()
