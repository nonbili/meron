import type { Observable } from '@legendapp/state'
import { invoke } from './bridge'

// Co-located session persistence. "Session" state is window/navigation state that
// should survive a restart but isn't a user preference (preferences live in
// states/settings, persisted there). Each store declares its own persisted fields
// next to the observable via this factory, so there's no central mirror module
// reaching into every store.
//
// `persistedField` writes the observable to the prefs KV store on change and
// seeds it back at boot. The returned `restore` is re-exported by the store for
// boot to call; the suppression guard lives here so seeding a value never echoes
// straight back as a fresh write.
export function persistedField<T>(obs$: Observable<T>, key: string, parse: (raw: unknown) => T | undefined) {
  let restoring = false
  obs$.onChange(({ value }) => {
    if (restoring) return
    void invoke('app.prefsSet', { key, value }).catch(() => {})
  })
  return {
    key,
    restore(prefs: Record<string, unknown>) {
      const value = parse(prefs[key])
      if (value === undefined) return
      restoring = true
      try {
        obs$.set(value)
      } finally {
        restoring = false
      }
    },
  }
}
