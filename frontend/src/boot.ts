import type { Account, SystemCheck } from './types'
import { invoke } from './lib/bridge'
import { hydrateSettings, SETTINGS_DB_KEYS } from './states/settings'
import { restoreUiSession, UI_SESSION_KEYS, ui$ } from './states/ui'
import { ensureDefaultKanbanBoard, restoreKanbanSession, KANBAN_SESSION_KEYS } from './states/kanban'
import { accounts$ } from './states/accounts'
import { openMailtoCompose, pruneComposerMedia } from './states/compose'

// App bootstrap: load the system check, accounts, and persisted settings in
// parallel, seed the initial selection, and drain any mailto: links the OS
// handed us before the window was ready.
export async function boot() {
  const [systemResult, accountResult, prefsResult] = await Promise.all([
    invoke<SystemCheck>('system.check'),
    invoke<{ accounts: Account[] }>('account.list'),
    invoke<{ prefs: Record<string, unknown> }>('app.prefsGet', {
      keys: [...SETTINGS_DB_KEYS, ...UI_SESSION_KEYS, ...KANBAN_SESSION_KEYS],
    }).catch(() => ({ prefs: {} })),
  ])
  const prefs = prefsResult.prefs || {}
  hydrateSettings(prefs)
  ui$.system.set(systemResult)
  accounts$.set(accountResult.accounts)
  restoreUiSession(prefs, accountResult.accounts)
  restoreKanbanSession(prefs)
  ensureDefaultKanbanBoard()

  const pendingMailto = await invoke<unknown>('mailto.consumePending').catch(() => [])
  if (Array.isArray(pendingMailto)) {
    for (const raw of pendingMailto) {
      if (typeof raw !== 'string') continue
      openMailtoCompose(raw)
    }
  }

  // Reclaim inline-image files orphaned by drafts that were discarded or sent in
  // a previous session. Runs after compose tabs are hydrated at module load.
  pruneComposerMedia()
}
