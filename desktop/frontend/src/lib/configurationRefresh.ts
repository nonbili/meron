import { invoke } from './bridge'
import { accounts$ } from '../states/accounts'
import { hydrateSettings } from '../states/settings'
import type { Account } from '../types'

// Track each key separately: a later account-only event must not discard an
// earlier signature refresh, while a stale read of the same key must not win.
export function configurationRefresh() {
  let active = true
  let revision = 0
  let accountRevision = 0
  const versions = new Map<string, number>()
  return {
    dispose() {
      active = false
    },
    async refresh(detail?: { keys?: string[] }) {
      const current = ++revision
      const keys = detail?.keys || []
      if (!keys.length) {
        const currentAccount = ++accountRevision
        const accounts = await invoke<{ accounts: Account[] }>('account.list')
        if (!active || currentAccount !== accountRevision) return
        // account.list returns [] while the engine restarts. MCP configuration
        // events never delete accounts, so an empty result cannot clear them.
        if (accounts.accounts.length || !accounts$.get().length) accounts$.set(accounts.accounts)
        return
      }
      keys.forEach((key) => versions.set(key, current))
      const settings = await invoke<{ prefs?: Record<string, unknown> }>('app.prefsGet', { keys })
      if (!active) return
      const prefs: Record<string, unknown> = settings.prefs || {}
      const changed = Object.fromEntries(
        keys.filter((key) => versions.get(key) === current && key in prefs).map((key) => [key, prefs[key]]),
      )
      if (Object.keys(changed).length) hydrateSettings(changed)
    },
  }
}
