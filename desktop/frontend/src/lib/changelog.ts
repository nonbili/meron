// In-app changelog: the GitHub releases atom feed, fetched + parsed by the core
// and filtered to this platform's `v*` tags (mobile uses the `android/v*` ones).
import { invoke } from './bridge'

export interface ChangelogRelease {
  version: string
  tag: string
  date: string
  notes: string[]
}

export async function fetchChangelog(): Promise<ChangelogRelease[]> {
  const res = await invoke<{ releases?: ChangelogRelease[] }>('changelog.fetch')
  return res?.releases ?? []
}
