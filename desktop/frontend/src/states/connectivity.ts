import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'

// Connectivity / sync health, cleared by successful syncs and reconnects.
// `error` is the last mail sync failure message, or null when mail is syncing
// fine; `account` is the mail account that failed, so a different account's
// success (e.g. RSS) can't dismiss the banner. ConnectivityBanner reads this;
// nothing here talks to the bridge directly, except watchCoreMigration below.
// `migrating` is true while a store upgrade is holding up the core.
export const connectivity$ = observable({
  error: null as string | null,
  account: null as string | null,
  migrating: false,
})

// A quick upgrade finishes unnoticed rather than flashing a banner.
const MIGRATION_NOTICE_DELAY_MS = 500

// Follow the core's store upgrade. The core starts before this window subscribes,
// so `core.migrating` may already have gone by: the status call covers that, and
// `done` stops its reply from reopening an upgrade that has since finished, or
// from arming a timer after cleanup, when nothing would be left to clear it.
export function watchCoreMigration() {
  const eventsOn = (window as any).runtime?.EventsOn
  if (!eventsOn) return
  let done = false
  let timer: number | undefined
  const begin = () => {
    if (done || timer !== undefined) return
    timer = window.setTimeout(() => connectivity$.migrating.set(true), MIGRATION_NOTICE_DELAY_MS)
  }
  const finish = () => {
    done = true
    window.clearTimeout(timer)
    connectivity$.migrating.set(false)
  }
  const offBegin = eventsOn('core.migrating', begin)
  const offDone = eventsOn('core.migrationDone', finish)
  void invoke<{ migrating: boolean }>('core.status')
    .then((status) => {
      if (status.migrating) begin()
    })
    .catch(() => {})
  return () => {
    done = true
    window.clearTimeout(timer)
    if (typeof offBegin === 'function') offBegin()
    if (typeof offDone === 'function') offDone()
  }
}

export function setSyncError(account: string | null, message: string) {
  connectivity$.account.set(account)
  connectivity$.error.set(message)
}

export function dismissSyncError() {
  connectivity$.error.set(null)
  connectivity$.account.set(null)
}

// Clear the banner when a mail sync or reconnect succeeds. Scoped to the failing
// account: success for another account (notably RSS) must not dismiss an outage
// on the account that errored. When either side is unknown, clear conservatively.
export function clearSyncErrorFor(account: string | null) {
  const erroring = connectivity$.account.get()
  if (erroring && account && erroring !== account) return
  connectivity$.error.set(null)
  connectivity$.account.set(null)
}
