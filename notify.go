package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

var (
	notifyIconOnce sync.Once
	notifyIconPath string
)

// notification is the platform-agnostic payload a delivered OS notification
// carries. account/threadID are stashed so that clicking the notification can
// open the originating thread (see openThreadFromNotification).
type notification struct {
	title    string
	body     string
	account  string
	threadID string
}

// notifyIcon writes the embedded app icon to a stable temp path the first time
// it is needed and returns that path. Linux/Windows notifications take an icon
// file path rather than raw bytes; an empty string falls back to no icon.
func notifyIcon() string {
	notifyIconOnce.Do(func() {
		if len(appIconPNG) == 0 {
			return
		}
		path := filepath.Join(os.TempDir(), "meron-notify.png")
		if err := os.WriteFile(path, appIconPNG, 0o644); err != nil {
			return
		}
		notifyIconPath = path
	})
	return notifyIconPath
}

// handleSidecarEvent runs for every event the mail engine pushes. It mirrors the
// event to the frontend already; here we layer on OS-level desktop notifications
// for newly arrived mail.
func (a *App) handleSidecarEvent(name string, detail any) {
	switch name {
	case "ready":
		a.checkProtocolVersion(detail)
		return
	case "mail.newMessages":
		a.notifyNewMail(detail)
	}
}

// expectedProtocolVersion is the stdio protocol version this bridge speaks. It
// must stay in lockstep with PROTOCOL_VERSION in meron-core/src/main.rs.
const expectedProtocolVersion = 1

// checkProtocolVersion reads the protocol version off the sidecar's `ready`
// handshake and warns on absence or mismatch, so a version skew between the Go
// bridge and the Rust sidecar surfaces in the log instead of as silent breakage.
func (a *App) checkProtocolVersion(detail any) {
	m, ok := detail.(map[string]any)
	if !ok {
		a.logf("sidecar ready: missing protocol version (expected %d)", expectedProtocolVersion)
		return
	}
	v, ok := m["protocol"].(float64)
	if !ok {
		a.logf("sidecar ready: missing protocol version (expected %d)", expectedProtocolVersion)
		return
	}
	if int(v) != expectedProtocolVersion {
		a.logf("sidecar protocol mismatch: bridge=%d sidecar=%d", expectedProtocolVersion, int(v))
	}
}

// openThreadFromNotification is the single funnel every platform's click handler
// routes through: focus the window and tell the frontend which thread to open.
// The frontend (useAppEffects) keys off threadId/threadKey; account is passed for
// parity/future use.
func (a *App) openThreadFromNotification(account, threadID string) {
	if a == nil || a.ctx == nil {
		return
	}
	a.logf("notification clicked: account=%s, threadID=%s", account, threadID)
	wailsRuntime.WindowShow(a.ctx)
	wailsRuntime.WindowUnminimise(a.ctx)
	wailsRuntime.EventsEmit(a.ctx, "notification-clicked", map[string]string{
		"account":   account,
		"threadId":  threadID,
		"threadKey": threadID,
	})
}

func (a *App) notifyNewMail(detail any) {
	count := 1
	var account, accountName, folder, from, subject, threadKey string
	if m, ok := detail.(map[string]any); ok {
		// Muted accounts still sync (the UI refreshes), they just don't raise an
		// OS notification. The sidecar resolves the mute pref onto each event.
		if muted, _ := m["muted"].(bool); muted {
			return
		}
		if c, ok := m["count"].(float64); ok && c > 0 {
			count = int(c)
		}
		account, _ = m["account"].(string)
		accountName, _ = m["accountName"].(string)
		folder, _ = m["folder"].(string)
		from, _ = m["from"].(string)
		subject, _ = m["subject"].(string)
		threadKey, _ = m["threadKey"].(string)
	}

	if accountName == "" {
		accountName = account
	}

	// Title carries the most identifying info (sender for a single message,
	// otherwise the count) so the user can triage at a glance from the OS
	// notification list. Body adds the next layer (subject, account).
	var title, body string
	if count == 1 {
		title = firstNonEmpty(from, accountName, "New message")
		body = firstNonEmpty(subject, "(no subject)")
		if accountName != "" && title != accountName {
			title = fmt.Sprintf("%s - %s", title, accountName)
		}
	} else {
		title = fmt.Sprintf("%d new messages", count)
		if accountName != "" {
			title = fmt.Sprintf("%s - %s", title, accountName)
		}
		if from != "" || subject != "" {
			body = firstNonEmpty(from, "unknown sender")
			if subject != "" {
				body = fmt.Sprintf("%s — %s", body, subject)
			}
		}
	}

	n := notification{
		title:    title,
		body:     body,
		account:  account,
		threadID: notificationThreadID(account, folder, threadKey, subject),
	}

	// Off the sidecar read loop: the platform notify call can block briefly and
	// must not stall event processing.
	go a.deliverNotification(n)
}

func notificationThreadID(account, folder, threadKey, subject string) string {
	if account == "" || threadKey == "" {
		return ""
	}
	if isRSSAccountID(account) {
		return fmt.Sprintf("%s#rss#%s", account, threadKey)
	}
	if folder == "" {
		folder = "INBOX"
	}
	// formatImapThreadID canonicalizes the folder casing (inbox → INBOX), so this
	// id lines up with the thread-list card the user clicks regardless of whether
	// the list spelled the folder "inbox" or "INBOX".
	compoundKey := threadKey
	if !strings.HasPrefix(threadKey, "uid:") {
		// Must use the grouping variant (not normalizeThreadSubject): threadRead's
		// subjectFilter is matched against threadGroupingSubject, which strips
		// leading bracket tags ([github], [EXTERNAL], …). Using the display variant
		// here builds an id that never matches such threads on click.
		compoundKey = threadKey + "#" + threadGroupingSubject(subject)
	}
	return formatImapThreadID(account, folder, compoundKey)
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}
