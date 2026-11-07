import { observable } from '@legendapp/state'
export const externalThreadDrafts$ = observable<string[]>([])

// This leaf module must stay independent of bridge/state imports: isolation is
// needed while the stores are first evaluated, before React mounts anything.
export const threadWindowSession = new URLSearchParams(window.location.search).get('threadWindow') || ''
