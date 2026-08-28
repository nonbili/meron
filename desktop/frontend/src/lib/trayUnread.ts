import { invoke } from './bridge'

type SendTrayUnread = (unread: boolean) => Promise<unknown>

// Wails calls are asynchronous and may otherwise reach Go out of order. Keep
// one tray update in flight and, if state changes while it is running, send the
// newest value immediately afterwards. Intermediate values can be coalesced:
// only the final unread state matters to the icon.
export function createTrayUnreadUpdater(send: SendTrayUnread = (unread) => invoke('tray.setUnread', { unread })) {
  let desired = false
  let version = 0
  let running = false

  const flush = async () => {
    if (running) return
    running = true
    let retriedVersion = -1
    try {
      while (true) {
        const sendingVersion = version
        const unread = desired
        try {
          await send(unread)
        } catch {
          // A newer value supersedes the failed write. Otherwise retry this
          // value once so a transient bridge failure cannot leave the tray
          // stale forever; a persistent failure still stops without spinning.
          if (sendingVersion !== version) continue
          if (retriedVersion === sendingVersion) return
          retriedVersion = sendingVersion
          continue
        }
        retriedVersion = -1
        if (sendingVersion === version) return
      }
    } finally {
      running = false
    }
  }

  return (unread: boolean) => {
    desired = unread
    version += 1
    void flush()
  }
}

export const setTrayUnread = createTrayUnreadUpdater()
