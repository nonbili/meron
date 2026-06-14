//go:build linux

package main

import (
	"sync"

	"github.com/gen2brain/beeep"
	"github.com/godbus/dbus/v5"
)

var (
	dbusConn    *dbus.Conn
	dbusMu      sync.Mutex
	dbusTargets = make(map[uint32]notificationTarget)
)

type notificationTarget struct {
	account  string
	threadID string
}

// setupNotificationListener opens a session-bus connection and watches for the
// freedesktop ActionInvoked/NotificationClosed signals so a clicked notification
// can open its thread. Falls back silently (beeep, no click) if the bus is
// unavailable.
func (a *App) setupNotificationListener() {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		a.logf("failed to connect to session bus: %v", err)
		return
	}

	dbusMu.Lock()
	dbusConn = conn
	dbusMu.Unlock()

	err = conn.AddMatchSignal(
		dbus.WithMatchInterface("org.freedesktop.Notifications"),
		dbus.WithMatchMember("ActionInvoked"),
	)
	if err != nil {
		a.logf("failed to add ActionInvoked match rule: %v", err)
		return
	}

	err = conn.AddMatchSignal(
		dbus.WithMatchInterface("org.freedesktop.Notifications"),
		dbus.WithMatchMember("NotificationClosed"),
	)
	if err != nil {
		a.logf("failed to add NotificationClosed match rule: %v", err)
		return
	}

	c := make(chan *dbus.Signal, 10)
	conn.Signal(c)

	go func() {
		for s := range c {
			switch s.Name {
			case "org.freedesktop.Notifications.ActionInvoked":
				if len(s.Body) >= 2 {
					id, ok1 := s.Body[0].(uint32)
					actionKey, ok2 := s.Body[1].(string)
					if ok1 && ok2 && actionKey == "default" {
						dbusMu.Lock()
						target, found := dbusTargets[id]
						delete(dbusTargets, id)
						dbusMu.Unlock()

						if found {
							a.openThreadFromNotification(target.account, target.threadID)
						}
					}
				}
			case "org.freedesktop.Notifications.NotificationClosed":
				if len(s.Body) >= 1 {
					id, ok := s.Body[0].(uint32)
					if ok {
						dbusMu.Lock()
						delete(dbusTargets, id)
						dbusMu.Unlock()
					}
				}
			}
		}
	}()
}

func (a *App) closeNotificationListener() {
	dbusMu.Lock()
	if dbusConn != nil {
		_ = dbusConn.Close()
		dbusConn = nil
	}
	dbusMu.Unlock()
}

// deliverNotification raises the notification over the session bus (so we can map
// the returned id back to a thread on click) and falls back to beeep when the bus
// is down — in which case the notification still shows but isn't clickable.
func (a *App) deliverNotification(n notification) {
	dbusMu.Lock()
	conn := dbusConn
	dbusMu.Unlock()

	if conn != nil {
		obj := conn.Object("org.freedesktop.Notifications", "/org/freedesktop/Notifications")
		call := obj.Call("org.freedesktop.Notifications.Notify", 0,
			"Meron",
			uint32(0),
			notifyIcon(),
			n.title,
			n.body,
			[]string{"default", ""},
			map[string]dbus.Variant{},
			int32(-1),
		)
		if call.Err == nil {
			var id uint32
			if err := call.Store(&id); err == nil {
				if n.threadID != "" {
					dbusMu.Lock()
					dbusTargets[id] = notificationTarget{account: n.account, threadID: n.threadID}
					dbusMu.Unlock()
				}
				return
			}
		} else {
			a.logf("dbus notify failed: %v, falling back to beeep", call.Err)
		}
	}

	if err := beeep.Notify(n.title, n.body, notifyIcon()); err != nil {
		a.logf("notify new mail failed: %v", err)
	}
}
