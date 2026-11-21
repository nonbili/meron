import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'

export type NewerSchema = { found: number; supported: number }

// Set while the core holds a store a newer Meron last used, waiting for the
// user to choose whether to open it; NewerSchemaDialog asks.
export const newerSchema$ = observable<NewerSchema | null>(null)

function parse(detail: unknown): NewerSchema | null {
  const value = detail as Partial<NewerSchema> | null | undefined
  if (typeof value?.found !== 'number' || typeof value?.supported !== 'number') return null
  return { found: value.found, supported: value.supported }
}

// The core starts before this window subscribes, so `core.newerSchema` may
// already have gone by: the status call covers that. `ready` means the store
// opened, whichever window made the choice; `core.fatal` that it couldn't
// start at all.
export function watchNewerSchema() {
  const eventsOn = (window as any).runtime?.EventsOn
  if (!eventsOn) return
  let done = false
  const offNewer = eventsOn('core.newerSchema', (detail: unknown) => {
    if (!done) newerSchema$.set(parse(detail))
  })
  const offReady = eventsOn('ready', () => newerSchema$.set(null))
  // Opened, then failed to start for another reason: its own error says why,
  // and a prompt left up would have nothing left to answer.
  const offFatal = eventsOn('core.fatal', () => newerSchema$.set(null))
  void invoke<{ newerSchema?: unknown }>('core.status')
    .then((status) => {
      const newer = parse(status.newerSchema)
      if (!done && newer) newerSchema$.set(newer)
    })
    .catch(() => {})
  return () => {
    done = true
    if (typeof offNewer === 'function') offNewer()
    if (typeof offReady === 'function') offReady()
    if (typeof offFatal === 'function') offFatal()
  }
}

// Open the store anyway, copying it first when `backup` is set. Resolves to
// the copies made. The prompt stays up until the core says what happened:
// `ready` once the store opened, or a new `core.newerSchema` when it has
// moved past the schema shown — which this reply must not clear.
export async function openNewerStore(backup: boolean): Promise<string[]> {
  const result = await invoke<{ backups?: string[] }>('store.openNewer', { backup })
  return result?.backups ?? []
}
