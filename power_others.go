//go:build !linux && !darwin && !windows

package main

// No OS resume signal on this platform; IDLE recovery falls back to TCP
// keepalive and the IDLE timeout.
func (a *App) setupResumeListener() {}

func (a *App) closeResumeListener() {}
