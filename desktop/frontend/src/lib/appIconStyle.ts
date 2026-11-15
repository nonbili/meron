import { invoke } from './bridge'
import { t } from './i18n'
import { createLatestValueUpdater } from './latestValueUpdater'
import { showToast } from '../states/ui'

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
export function createAppIconUpdater(send: (icon: AppIcon) => Promise<unknown> = sendAppIcon) {
  // Keyed by a string so two equal choices compare equal.
  const update = createLatestValueUpdater((key: string) => send(JSON.parse(key) as AppIcon))
  return (icon: AppIcon) => update(JSON.stringify({ custom: icon.custom, colour: icon.colour }))
}

/**
 * The backend applies the icon even when it cannot save the choice; the user
 * is told, since the icon would go back to the shipped one on the next start.
 */
export async function sendAppIcon(
  icon: AppIcon,
  apply = (value: AppIcon) => invoke<{ persisted?: boolean }>('appIcon.set', value),
  notify = () => showToast(t('settings.appearance.appIconNotSaved'), 'error'),
) {
  const result = await apply(icon)
  if (result?.persisted === false) notify()
}

export const setAppIcon = createAppIconUpdater()
