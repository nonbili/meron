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

type nativeLabels struct {
	trayShow, trayHide, trayHideTooltip, trayQuit         string
	newMessage, newMessageCount, noSubject, unknownSender string
}

var (
	nativeLabelsMu sync.RWMutex
	currentLabels  = nativeLabels{
		trayShow: "Show Meron", trayHide: "Hide to Tray",
		trayHideTooltip: "Hide Meron to the system tray", trayQuit: "Quit Meron",
		newMessage:      "New message",
		newMessageCount: "{count, plural, one {1 new message} other {{count} new messages}}",
		noSubject:       "(no subject)", unknownSender: "unknown sender",
	}
)

func (a *App) currentNativeLabels() nativeLabels {
	nativeLabelsMu.RLock()
	defer nativeLabelsMu.RUnlock()
	return currentLabels
}

func (a *App) setNativeLabels(payload map[string]any) (any, error) {
	nativeLabelsMu.Lock()
	set := func(field *string, key string) {
		if value, ok := payload[key].(string); ok && value != "" {
			*field = value
		}
	}
	set(&currentLabels.trayShow, "trayShow")
	set(&currentLabels.trayHide, "trayHide")
	set(&currentLabels.trayHideTooltip, "trayHideTooltip")
	set(&currentLabels.trayQuit, "trayQuit")
	set(&currentLabels.newMessage, "newMessage")
	set(&currentLabels.newMessageCount, "newMessageCount")
	set(&currentLabels.noSubject, "noSubject")
	set(&currentLabels.unknownSender, "unknownSender")
	labels := currentLabels
	// Released before trayMenuMu is taken: trayReady holds trayMenuMu while it
	// reads the labels, so holding both here in the other order would deadlock.
	nativeLabelsMu.Unlock()
	trayMenuMu.Lock()
	defer trayMenuMu.Unlock()
	if trayShowItem != nil {
		trayShowItem.SetTitle(labels.trayShow)
		trayShowItem.SetTooltip(labels.trayShow)
	}
	if trayHideItem != nil {
		trayHideItem.SetTitle(labels.trayHide)
		trayHideItem.SetTooltip(labels.trayHideTooltip)
	}
	if trayQuitItem != nil {
		trayQuitItem.SetTitle(labels.trayQuit)
		trayQuitItem.SetTooltip(labels.trayQuit)
	}
	return map[string]any{"ok": true}, nil
}

func formatNativePlural(template string, count int) string {
	selected := template
	if strings.HasPrefix(template, "{count, plural,") {
		branches := map[string]string{}
		for _, category := range []string{"one", "other"} {
			marker := category + " {"
			start := strings.Index(template, marker)
			if start < 0 {
				continue
			}
			start += len(marker)
			depth, end := 1, start
			for end < len(template) && depth > 0 {
				switch template[end] {
				case '{':
					depth++
				case '}':
					depth--
				}
				end++
			}
			if depth == 0 {
				branches[category] = template[start : end-1]
			}
		}
		if count == 1 {
			selected = branches["one"]
		} else {
			selected = branches["other"]
		}
	}
	return strings.ReplaceAll(selected, "{count}", fmt.Sprint(count))
}

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
// it is needed and returns that path. Windows notifications take an icon file
// path rather than raw bytes; an empty string falls back to no icon.
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
	case "core.fatal":
		a.recordCoreError(detail)
		return
	case "mail.syncError", "error":
		a.logSidecarError(name, detail)
		return
	case "mail.newMessages":
		a.notifyNewMail(detail)
	}
}

// logSidecarError copies engine-side sync/generic failures into meron.log. The
// UI shows only a generic "unable to connect" banner, so without this the real
// cause (TLS, auth, timeout) reached the frontend and was thrown away, leaving
// nothing on disk to diagnose a failing account from.
func (a *App) logSidecarError(name string, detail any) {
	message, account := "", ""
	if m, ok := detail.(map[string]any); ok {
		message, _ = m["message"].(string)
		account, _ = m["account"].(string)
	}
	if message == "" {
		message = fmt.Sprint(detail)
	}
	if account != "" {
		a.logf("%s: account=%s %s", name, account, message)
		return
	}
	a.logf("%s: %s", name, message)
}

// recordCoreError logs the core's own fatal-condition report and keeps it for
// engineUnavailable. The event already reaches the frontend; dropping it here
// meant a core that could not open its store failed silently, leaving only
// per-call timeouts in the log.
func (a *App) recordCoreError(detail any) {
	message := ""
	if m, ok := detail.(map[string]any); ok {
		message, _ = m["message"].(string)
	}
	if message == "" {
		message = fmt.Sprint(detail)
	}
	a.logf("core error: %s", message)
	a.setCoreError(message)
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
	a.showMainWindow()
	wailsRuntime.EventsEmit(a.ctx, "notification-clicked", map[string]string{
		"account":   account,
		"threadId":  threadID,
		"threadKey": threadID,
	})
}

func (a *App) notifyNewMail(detail any) {
	labels := a.currentNativeLabels()
	count := 1
	var account, accountName, folder, from, subject, preview, threadKey string
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
		preview, _ = m["preview"].(string)
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
		title = firstNonEmpty(from, accountName, labels.newMessage)
		body = firstNonEmpty(subject, labels.noSubject)
		// The body snippet, when the sidecar managed to fetch it in time, so
		// the notification shows the mail itself and not just its subject.
		if preview != "" {
			body = fmt.Sprintf("%s — %s", body, preview)
		}
		if accountName != "" && title != accountName {
			title = fmt.Sprintf("%s - %s", title, accountName)
		}
	} else {
		title = formatNativePlural(labels.newMessageCount, count)
		if accountName != "" {
			title = fmt.Sprintf("%s - %s", title, accountName)
		}
		if from != "" || subject != "" {
			body = firstNonEmpty(from, labels.unknownSender)
			if subject != "" {
				body = fmt.Sprintf("%s — %s", body, subject)
			}
		}
	}

	n := notification{
		title:    title,
		body:     body,
		account:  account,
		threadID: notificationThreadID(account, folder, threadKey),
	}

	// Off the sidecar read loop: the platform notify call can block briefly and
	// must not stall event processing.
	go a.deliverNotification(n)
}

func notificationThreadID(account, folder, threadKey string) string {
	if account == "" || threadKey == "" {
		return ""
	}
	if isRSSAccountID(account) {
		return fmt.Sprintf("%s#rss#%s", account, threadKey)
	}
	if folder == "" {
		folder = "INBOX"
	}
	// The sidecar already emits the branch-aware card key (subject branching
	// lives in the core), so this id lines up with the thread-list card the
	// user clicks. formatImapThreadID canonicalizes the folder casing
	// (inbox → INBOX) for the same reason.
	return formatImapThreadID(account, folder, threadKey)
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}
