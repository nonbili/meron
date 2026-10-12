/**
 * Whether the monochrome tray icon is offered, and on by default: the macOS
 * menu bar and Linux panels draw one-colour icons that follow the bar (a
 * template image on macOS, a symbolic icon the shell recolours on Linux), as
 * the backend's default does (tray.go). Windows keeps the colour icon.
 */
export function offersMonochromeTrayIcon(userAgent: string): boolean {
  return /linux|macintosh/i.test(userAgent)
}
