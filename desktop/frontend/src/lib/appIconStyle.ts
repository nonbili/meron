import { invoke } from './bridge'
import { createLatestValueUpdater } from './latestValueUpdater'

/**
 * The app icon: the shipped one, or the same drawing in a colour of the user's
 * own. The backend recolours the artwork and owns the saved choice. The colour
 * is kept while the shipped icon is in use, so the picker comes back to it.
 */
export type AppIcon = { custom: boolean; colour: string }

/** What the picker starts on before the user has chosen anything. */
export const DEFAULT_APP_ICON_COLOUR = '#1d6fe0'

/** Narrows whatever the backend sent to "#rrggbb"; anything else is the default. */
export function parseAppIconColour(value: unknown): string {
  return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : DEFAULT_APP_ICON_COLOUR
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

/**
 * Dragging through the colour picker produces a stream of colours; only the
 * one it ends on has to reach the backend, which redraws the icon each time.
 */
export function createAppIconUpdater(send = (icon: AppIcon) => invoke('appIcon.set', icon)) {
  // Keyed by a string so two equal choices compare equal.
  const update = createLatestValueUpdater((key: string) => send(JSON.parse(key) as AppIcon))
  return (icon: AppIcon) => update(JSON.stringify({ custom: icon.custom, colour: icon.colour }))
}

export const setAppIcon = createAppIconUpdater()
