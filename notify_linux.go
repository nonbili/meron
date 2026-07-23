//go:build linux

package main

import (
	"encoding/json"
	"os"
	"sync"

	"github.com/gen2brain/beeep"
	"github.com/godbus/dbus/v5"
	"github.com/google/uuid"
)

const (
	portalNotificationInterface  = "org.freedesktop.portal.Notification"
	portalNotificationAdd        = portalNotificationInterface + ".AddNotification"
	portalNotificationAction     = portalNotificationInterface + ".ActionInvoked"
	portalNotificationOpenAction = "open-thread"
)

var (
	notificationConn    *dbus.Conn
	notificationSignals chan *dbus.Signal
	notificationDone    chan struct{}
	notificationMu      sync.Mutex
)

type notificationTarget struct {
	Account  string `json:"account"`
	ThreadID string `json:"threadId"`
}

// setupNotificationListener opens a session-bus connection and watches for the
// notification portal's ActionInvoked signal so a clicked notification can open
// its thread.
func (a *App) setupNotificationListener() {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		a.logf("failed to connect to notification portal: %v", err)
		return
	}

	matchOptions := []dbus.MatchOption{
		dbus.WithMatchInterface(portalNotificationInterface),
		dbus.WithMatchMember("ActionInvoked"),
		dbus.WithMatchObjectPath(portalDesktopPath),
	}
	if err := conn.AddMatchSignal(matchOptions...); err != nil {
		_ = conn.Close()
		a.logf("failed to listen for notification portal actions: %v", err)
		return
	}

	signals := make(chan *dbus.Signal, 10)
	done := make(chan struct{})
	conn.Signal(signals)

	notificationMu.Lock()
	notificationConn = conn
	notificationSignals = signals
	notificationDone = done
	notificationMu.Unlock()

	// Portal notification actions include the account/thread payload as their
	// target, so no process-local notification-id map is needed.
	go func() {
		for {
			select {
			case <-done:
				return
			case signal, ok := <-signals:
				if !ok {
					return
				}
				if signal.Name != portalNotificationAction {
					continue
				}
				if target, ok := portalNotificationTarget(signal.Body); ok {
					a.openThreadFromNotification(target.Account, target.ThreadID)
				}
			}
		}
	}()
}

func (a *App) closeNotificationListener() {
	notificationMu.Lock()
	conn := notificationConn
	signals := notificationSignals
	done := notificationDone
	notificationConn = nil
	notificationSignals = nil
	notificationDone = nil
	notificationMu.Unlock()

	if conn == nil {
		return
	}
	close(done)
	conn.RemoveSignal(signals)
	_ = conn.RemoveMatchSignal(
		dbus.WithMatchInterface(portalNotificationInterface),
		dbus.WithMatchMember("ActionInvoked"),
		dbus.WithMatchObjectPath(portalDesktopPath),
	)
	_ = conn.Close()
}

// deliverNotification raises the notification through the desktop portal. The
// legacy notification service is used only as a fallback outside Flatpak, where
// no sandbox permission is needed.
func (a *App) deliverNotification(n notification) {
	notificationMu.Lock()
	conn := notificationConn
	notificationMu.Unlock()

	if conn != nil {
		id := "meron-" + uuid.NewString()
		call := conn.Object(portalDesktopName, portalDesktopPath).Call(
			portalNotificationAdd,
			0,
			id,
			portalNotificationOptions(n),
		)
		if call.Err == nil {
			return
		}
		a.logf("notification portal failed: %v", call.Err)
	}

	if os.Getenv("FLATPAK_ID") != "" {
		return
	}

	if err := beeep.Notify(n.title, n.body, notifyIcon()); err != nil {
		a.logf("notify new mail failed: %v", err)
	}
}

type portalSerializedIcon struct {
	Type  string
	Value dbus.Variant
}

func portalNotificationOptions(n notification) map[string]dbus.Variant {
	options := map[string]dbus.Variant{
		"title":    dbus.MakeVariant(n.title),
		"body":     dbus.MakeVariant(n.body),
		"priority": dbus.MakeVariant("normal"),
		"icon": dbus.MakeVariant(portalSerializedIcon{
			Type:  "themed",
			Value: dbus.MakeVariant([]string{"jp.nonbili.meron"}),
		}),
	}

	if n.account == "" || n.threadID == "" {
		return options
	}
	target, err := json.Marshal(notificationTarget{
		Account:  n.account,
		ThreadID: n.threadID,
	})
	if err != nil {
		return options
	}
	options["default-action"] = dbus.MakeVariant(portalNotificationOpenAction)
	options["default-action-target"] = dbus.MakeVariant(string(target))
	return options
}

func portalNotificationTarget(body []any) (notificationTarget, bool) {
	if len(body) < 3 {
		return notificationTarget{}, false
	}
	action, ok := body[1].(string)
	if !ok || action != portalNotificationOpenAction {
		return notificationTarget{}, false
	}
	parameters, ok := body[2].([]dbus.Variant)
	if !ok || len(parameters) == 0 {
		return notificationTarget{}, false
	}
	raw, ok := parameters[0].Value().(string)
	if !ok {
		return notificationTarget{}, false
	}
	var target notificationTarget
	if err := json.Unmarshal([]byte(raw), &target); err != nil {
		return notificationTarget{}, false
	}
	if target.Account == "" || target.ThreadID == "" {
		return notificationTarget{}, false
	}
	return target, true
}
