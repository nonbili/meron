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

// The pre-portal notification service every unsandboxed Linux app talks to.
const (
	fdoNotificationName      = "org.freedesktop.Notifications"
	fdoNotificationPath      = dbus.ObjectPath("/org/freedesktop/Notifications")
	fdoNotificationInterface = "org.freedesktop.Notifications"
	fdoNotificationNotify    = fdoNotificationInterface + ".Notify"
	fdoNotificationAction    = fdoNotificationInterface + ".ActionInvoked"
	fdoNotificationClosed    = fdoNotificationInterface + ".NotificationClosed"
	// The action key a notification server invokes when the body itself is
	// clicked, as opposed to one of the buttons.
	fdoNotificationDefaultAction = "default"
)

var (
	notificationConn    *dbus.Conn
	notificationSignals chan *dbus.Signal
	notificationDone    chan struct{}
	notificationMu      sync.Mutex
	// Thread to open per live notification id, for the freedesktop service —
	// unlike the portal, its ActionInvoked signal carries only the id, so the
	// payload stays here until the notification is clicked or closed.
	fdoNotificationTargets = map[uint32]fdoNotificationTarget{}
)

type notificationTarget struct {
	Account  string `json:"account"`
	ThreadID string `json:"threadId"`
}

type fdoNotificationTarget struct {
	target notificationTarget
	// Signals received before the Notify reply belong to an older use of the
	// same server-generated id, even if their handlers run after this entry is
	// installed.
	responseSequence dbus.Sequence
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
	// Clicks and dismissals on the freedesktop service used outside Flatpak.
	// These are broadcast to every listener, so the handlers below act only on
	// ids this process registered.
	for _, member := range []string{"ActionInvoked", "NotificationClosed"} {
		if err := conn.AddMatchSignal(
			dbus.WithMatchInterface(fdoNotificationInterface),
			dbus.WithMatchMember(member),
		); err != nil {
			a.logf("failed to listen for notification %s: %v", member, err)
		}
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
				switch signal.Name {
				case portalNotificationAction:
					if target, ok := portalNotificationTarget(signal.Body); ok {
						a.openThreadFromNotification(target.Account, target.ThreadID)
					}
				case fdoNotificationAction:
					if target, ok := takeFdoNotificationTarget(signal.Body, signal.Sequence); ok {
						a.openThreadFromNotification(target.Account, target.ThreadID)
					}
				case fdoNotificationClosed:
					// Dismissed or expired: drop the payload so the map only
					// ever holds notifications still on screen.
					closeFdoNotificationTarget(signal.Body, signal.Sequence)
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

// deliverNotification raises the notification, choosing the transport by
// sandbox rather than by trying the portal first.
//
// Inside Flatpak the portal is the only route out, and it works there because
// the sandbox gives the app a real app id. Outside it, the portal hands the
// notification to the desktop's own service keyed by an app id it guesses from
// the process — GNOME Shell then rejects ids it cannot resolve to an installed
// .desktop ("The app by ID \"meron\" could not be found") and drops it. The
// portal call still returns success, so that failure is invisible to us and no
// fallback can key off it. Unsandboxed builds therefore talk to the
// freedesktop notification service directly, as every other native app does.
func (a *App) deliverNotification(n notification) {
	notificationMu.Lock()
	conn := notificationConn
	notificationMu.Unlock()

	if os.Getenv("FLATPAK_ID") != "" {
		if conn == nil {
			return
		}
		id := "meron-" + uuid.NewString()
		call := conn.Object(portalDesktopName, portalDesktopPath).Call(
			portalNotificationAdd,
			0,
			id,
			portalNotificationOptions(n),
		)
		if call.Err != nil {
			a.logf("notification portal failed: %v", call.Err)
		}
		return
	}

	if conn != nil && a.notifyFreedesktop(conn, n) {
		return
	}

	if err := beeep.Notify(n.title, n.body, notifyIcon()); err != nil {
		a.logf("notify new mail failed: %v", err)
	}
}

// notifyFreedesktop posts through org.freedesktop.Notifications, registering a
// default action so clicking the notification body opens its thread (beeep, the
// last-resort fallback, cannot carry one). Reports whether it was accepted.
func (a *App) notifyFreedesktop(conn *dbus.Conn, n notification) bool {
	// Only offer the click action when there is something to open; a server
	// that renders actions as buttons would otherwise show a dead one.
	var actions []string
	if n.account != "" && n.threadID != "" {
		actions = []string{fdoNotificationDefaultAction, "Open"}
	}
	hints := map[string]dbus.Variant{
		// Attributes the notification to the installed desktop entry, which is
		// what the shell keys its per-app icon and notification settings off.
		"desktop-entry": dbus.MakeVariant(linuxDesktopEntry()),
	}

	call := conn.Object(fdoNotificationName, fdoNotificationPath).Call(
		fdoNotificationNotify,
		0,
		"Meron",
		uint32(0), // 0 replaces nothing; each arrival gets its own notification.
		notifyIcon(),
		n.title,
		n.body,
		actions,
		hints,
		int32(-1), // Let the server pick the timeout.
	)
	if call.Err != nil {
		a.logf("notify new mail failed: %v", call.Err)
		return false
	}
	if len(actions) == 0 {
		return true
	}

	var id uint32
	if err := call.Store(&id); err != nil {
		// Delivered, but without an id there is nothing to match a later click
		// against; the notification simply won't be clickable.
		return true
	}
	notificationMu.Lock()
	fdoNotificationTargets[id] = fdoNotificationTarget{
		target:           notificationTarget{Account: n.account, ThreadID: n.threadID},
		responseSequence: call.ResponseSequence,
	}
	notificationMu.Unlock()
	return true
}

// linuxDesktopEntry is the basename of this build's installed .desktop file.
// Snap installs its own under <snap>_<app>; everything else ships meron.desktop.
func linuxDesktopEntry() string {
	if id := os.Getenv("FLATPAK_ID"); id != "" {
		return id
	}
	if name := os.Getenv("SNAP_NAME"); name != "" {
		return name + "_meron"
	}
	return "meron"
}

// takeFdoNotificationTarget resolves an ActionInvoked signal to the thread it
// should open, consuming the entry: the click is the notification's last event
// (the server closes it right after), and ids are reused over time.
func takeFdoNotificationTarget(body []any, sequence dbus.Sequence) (notificationTarget, bool) {
	if len(body) < 2 {
		return notificationTarget{}, false
	}
	id, ok := body[0].(uint32)
	if !ok {
		return notificationTarget{}, false
	}
	action, ok := body[1].(string)
	if !ok || action != fdoNotificationDefaultAction {
		return notificationTarget{}, false
	}
	notificationMu.Lock()
	entry, known := fdoNotificationTargets[id]
	if known && sequence > entry.responseSequence {
		delete(fdoNotificationTargets, id)
	} else {
		known = false
	}
	notificationMu.Unlock()
	return entry.target, known
}

func closeFdoNotificationTarget(body []any, sequence dbus.Sequence) {
	if len(body) == 0 {
		return
	}
	id, ok := body[0].(uint32)
	if !ok {
		return
	}
	notificationMu.Lock()
	if entry, known := fdoNotificationTargets[id]; known && sequence > entry.responseSequence {
		delete(fdoNotificationTargets, id)
	}
	notificationMu.Unlock()
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
