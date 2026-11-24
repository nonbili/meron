import { invoke } from './bridge'
import { createLatestValueUpdater } from './latestValueUpdater'

export interface TrayUnreadState {
  count: number
  taskbarBadge?: boolean
}

type SendTrayUnread = (state: TrayUnreadState) => Promise<unknown>

/** The tray shows whether there is unread mail; the macOS Dock and Windows taskbar badges show how much. */
export function trayUnreadPayload(count: number, taskbarBadge?: boolean) {
  return taskbarBadge === undefined ? { unread: count > 0, count } : { unread: count > 0, count, taskbarBadge }
}

export function createTrayUnreadUpdater(
  send: SendTrayUnread = ({ count, taskbarBadge }) => invoke('tray.setUnread', trayUnreadPayload(count, taskbarBadge)),
) {
  // Later folder refreshes still reach Go so it can repair a cleared cache.
  // Go deduplicates the native write; in-flight refreshes are coalesced here.
  return createLatestValueUpdater(send, {
    deduplicate: false,
    equals: (a, b) => a.count === b.count && a.taskbarBadge === b.taskbarBadge,
  })
}

export const setTrayUnread = createTrayUnreadUpdater()
