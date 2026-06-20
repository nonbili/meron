//go:build linux

package main

import (
	"sync"

	"github.com/godbus/dbus/v5"
)

var (
	resumeBus *dbus.Conn
	resumeMu  sync.Mutex
)

// setupResumeListener watches systemd-logind's PrepareForSleep signal on the
// system bus. logind emits PrepareForSleep(true) right before suspend and
// PrepareForSleep(false) once the system has woken; on the latter we tell the
// sidecar so its IDLE watchers reconnect. Falls back silently (relying on TCP
// keepalive) if the system bus is unavailable.
func (a *App) setupResumeListener() {
	conn, err := dbus.ConnectSystemBus()
	if err != nil {
		a.logf("resume: connect system bus: %v", err)
		return
	}

	if err := conn.AddMatchSignal(
		dbus.WithMatchInterface("org.freedesktop.login1.Manager"),
		dbus.WithMatchMember("PrepareForSleep"),
	); err != nil {
		a.logf("resume: add PrepareForSleep match: %v", err)
		_ = conn.Close()
		return
	}

	resumeMu.Lock()
	resumeBus = conn
	resumeMu.Unlock()

	c := make(chan *dbus.Signal, 4)
	conn.Signal(c)

	go func() {
		for s := range c {
			if s.Name != "org.freedesktop.login1.Manager.PrepareForSleep" {
				continue
			}
			// Body is a single bool: true = about to sleep, false = resumed.
			if len(s.Body) == 1 {
				if sleeping, ok := s.Body[0].(bool); ok && !sleeping {
					a.onSystemResumed()
				}
			}
		}
	}()
}

func (a *App) closeResumeListener() {
	resumeMu.Lock()
	if resumeBus != nil {
		_ = resumeBus.Close()
		resumeBus = nil
	}
	resumeMu.Unlock()
}
