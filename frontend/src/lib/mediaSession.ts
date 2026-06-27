export function clearMediaSession(win: Window | null | undefined = window) {
  const session = win?.navigator?.mediaSession
  if (!session) return
  try {
    session.metadata = null
    session.playbackState = 'none'
  } catch {
    // Some WebKitGTK builds expose MediaSession partially.
  }
}
