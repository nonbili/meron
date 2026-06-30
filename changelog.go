package main

import "errors"

// changelogFetch returns the in-app changelog (the GitHub releases atom feed,
// filtered to the desktop `v*` tags). The fetch + parse lives in the shared
// core; we just forward with the desktop variant.
func (a *App) changelogFetch() (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	return a.sidecar.Call("changelog.fetch", map[string]any{"variant": "desktop"})
}
