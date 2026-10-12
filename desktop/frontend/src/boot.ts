import type { Account, SystemCheck } from './types'
import { invoke, invokeRetryingTimeout } from './lib/bridge'
import { hydrateSettings, hydrateTrayStyle, SETTINGS_DB_KEYS, WRITE_SESSION } from './states/settings'
import { restoreUiSession, UI_SESSION_KEYS, ui$ } from './states/ui'
import { ensureDefaultKanbanBoard, restoreKanbanSession, KANBAN_SESSION_KEYS } from './states/kanban'
import { restoreTasksSession, TASKS_SESSION_KEYS } from './states/tasks'
import { accounts$ } from './states/accounts'
import { openMailtoCompose, pruneComposerMedia } from './states/compose'

// App bootstrap: load the system check, accounts, and persisted settings in
// parallel, seed the initial selection, and drain any mailto: links the OS
// handed us before the window was ready.
// Accounts and prefs retry on a timeout: the core answers them from the store,
// which a first start after an upgrade may hold for a while (migrations), and a
// window booted without them stays empty until restarted.
export async function boot() {
  const [systemResult, accountResult, prefsResult, trayStyle] = await Promise.all([
    invoke<SystemCheck>('system.check'),
    invokeRetryingTimeout<{ accounts: Account[] }>('account.list'),
    invokeRetryingTimeout<{ prefs: Record<string, unknown> }>('app.prefsGet', {
      keys: [...SETTINGS_DB_KEYS, ...UI_SESSION_KEYS, ...KANBAN_SESSION_KEYS, ...TASKS_SESSION_KEYS],
      // Reading our prefs is also how this window claims write ordering from
      // whatever session ran before it (see `activate_pref_session` in core).
      session: WRITE_SESSION,
    }).catch(() => ({ prefs: {} })),
    invoke<{ monochrome: boolean }>('tray.getStyle').catch(() => null),
  ])
  const prefs: Record<string, unknown> = prefsResult.prefs || {}
  hydrateSettings(prefs)
  hydrateTrayStyle(trayStyle?.monochrome ?? systemResult.tray_monochrome ?? systemResult.platform !== 'windows')
  ui$.system.set(systemResult)
  accounts$.set(accountResult.accounts)
  restoreUiSession(prefs, accountResult.accounts)
  restoreKanbanSession(prefs)
  await restoreTasksSession(prefs)
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
