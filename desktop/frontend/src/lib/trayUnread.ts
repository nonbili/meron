import { invoke } from './bridge'
import { createLatestValueUpdater } from './latestValueUpdater'

type SendTrayUnread = (count: number) => Promise<unknown>

/** The tray shows whether there is unread mail; the macOS Dock badge shows how much. */
export function trayUnreadPayload(count: number) {
  return { unread: count > 0, count }
}

export function createTrayUnreadUpdater(
  send: SendTrayUnread = (count) => invoke('tray.setUnread', trayUnreadPayload(count)),
) {
  // Later folder refreshes still reach Go so it can repair a cleared cache.
  // Go deduplicates the native write; in-flight refreshes are coalesced here.
  return createLatestValueUpdater(send, { deduplicate: false })
}

export const setTrayUnread = createTrayUnreadUpdater()
