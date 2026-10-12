type SendBoolean = (value: boolean) => Promise<unknown>

// Wails calls are asynchronous and may otherwise reach Go out of order. Keep
// one tray update in flight and, if state changes while it is running, send the
// newest value immediately afterwards. Intermediate values can be coalesced:
// only the final value matters. Used for both unread state and tray style.
export function createLatestBooleanUpdater(send: SendBoolean, { deduplicate = true } = {}) {
  let desired: boolean | undefined
  let failed = false
  let version = 0
  let running = false

  const flush = async () => {
    if (running) return
    running = true
    let retriedVersion = -1
    try {
      while (true) {
        const sendingVersion = version
        const value = desired
        try {
          await send(value!)
        } catch {
          // A newer value supersedes the failed write. Otherwise retry this
          // value once so a transient bridge failure cannot leave the tray
          // stale forever; a persistent failure still stops without spinning.
          if (sendingVersion !== version) continue
          if (retriedVersion === sendingVersion) {
            failed = true
            return
          }
          retriedVersion = sendingVersion
          continue
        }
        retriedVersion = -1
        failed = false
        if (sendingVersion === version || desired === value) return
      }
    } finally {
      running = false
    }
  }

  return (value: boolean) => {
    if (deduplicate && desired === value && !failed) return
    desired = value
    version += 1
    void flush()
  }
}
