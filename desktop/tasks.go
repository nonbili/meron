package main

// tasksInvoke forwards one Tasks command to the core unchanged.
//
// The core's method names and the frontend's are identical, so there is nothing
// to translate here: the bridge must not become a place where a command means
// one thing on one side and another on the other, the way `rss.addFeed` and
// `feed.add` already do.
func (a *App) tasksInvoke(command string, payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	res, err := a.sidecar.Call(command, payload)
	if err != nil {
		return nil, err
	}
	return res, nil
}
