package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

var (
	notifyIconOnce sync.Once
	notifyIconPath string
)

type nativeLabels struct {
	calendar, tasks                                       string
	trayShow, trayHide, trayHideTooltip, trayQuit         string
	newMessage, newMessageCount, noSubject, unknownSender string
}

var (
	nativeLabelsMu sync.RWMutex
	currentLabels  = nativeLabels{
		calendar: "Calendar", tasks: "Tasks",
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
	set(&currentLabels.calendar, "calendar")
	set(&currentLabels.tasks, "tasks")
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
	// The arrivals it announces, so it can be withdrawn once all are read;
	// empty when they cannot all be named (see parseStoreMessages).
	storeMessages []storeMessage
}

// storeMessage is a core store row: mail by folder and UID, a feed entry by
// its subscription and entry key — what `messages.stillUnread` looks up.
type storeMessage struct {
	folder    string
	messageID string
}

// trackedNotification is a delivered new-mail notification still on screen,
// as far as we know, with the platform handle that withdraws it.
type trackedNotification struct {
	handle   string
	account  string
	messages []storeMessage
}

// maxTrackedNotifications bounds the list: entries for notifications the user
// dismissed linger until their mail is read, and mail never read would
// otherwise pile up for the life of the process.
const maxTrackedNotifications = 32

var (
	trackedMu             sync.Mutex
	trackedNotifications  []trackedNotification
	notifyReconcileTimer  *time.Timer
	notifyReconcileWindow = 500 * time.Millisecond
)

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
		a.requestNotificationReconcile()
	case "mail.synced":
		// A background or IDLE sync may have brought in mail read, moved or
		// deleted on another device.
		a.requestNotificationReconcile()
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
	// Reminder targets use the same native activation path to focus the app.
	if account == reminderNotificationAccount {
		return
	}
	wailsRuntime.EventsEmit(a.ctx, "notification-clicked", map[string]string{
		"account":   account,
		"threadId":  threadID,
		"threadKey": threadID,
	})
}

func (a *App) notifyNewMail(detail any) {
	labels := a.currentNativeLabels()
	count := 1
	var account, accountName, folder, folderName, from, subject, preview, threadKey string
	var storeMessages []storeMessage
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
		folderName, _ = m["folderName"].(string)
		from, _ = m["from"].(string)
		subject, _ = m["subject"].(string)
		preview, _ = m["preview"].(string)
		threadKey, _ = m["threadKey"].(string)
		storeMessages = parseStoreMessages(m, count)
	}

	if accountName == "" {
		accountName = account
	}
	// Mail outside the inbox (a folder opted in to notifications) names where
	// it landed, since that is not where the user would look for it.
	if folderName != "" {
		accountName = fmt.Sprintf("%s / %s", accountName, folderName)
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
		title:         title,
		body:          body,
		account:       account,
		threadID:      notificationThreadID(account, folder, threadKey),
		storeMessages: storeMessages,
	}

	// Off the sidecar read loop: the platform notify call can block briefly and
	// must not stall event processing.
	go func() {
		if handle := a.deliverNotification(n); handle != "" && len(n.storeMessages) > 0 {
			trackNotification(trackedNotification{handle: handle, account: account, messages: n.storeMessages})
			// The mail may have been read while delivery was under way, after
			// the event's own reconcile found nothing yet to compare.
			a.requestNotificationReconcile()
		}
	}()
}

// parseStoreMessages names the store rows a `mail.newMessages` detail's
// arrivals are: mail by the batch folder and UID, a feed entry by its
// subscription (the entry's thread key) and entry key. Empty unless every
// arrival is listed and identifiable — a notification standing partly for
// unlisted mail must not be withdrawn on the rest.
func parseStoreMessages(detail map[string]any, count int) []storeMessage {
	account, _ := detail["account"].(string)
	folder, _ := detail["folder"].(string)
	entries, _ := detail["messages"].([]any)
	if len(entries) == 0 || len(entries) != count {
		return nil
	}
	messages := make([]storeMessage, 0, len(entries))
	for _, raw := range entries {
		entry, _ := raw.(map[string]any)
		if isRSSAccountID(account) {
			subscription, _ := entry["threadKey"].(string)
			itemKey, _ := entry["itemKey"].(string)
			if subscription == "" || itemKey == "" {
				return nil
			}
			messages = append(messages, storeMessage{folder: subscription, messageID: itemKey})
			continue
		}
		uid, _ := entry["uid"].(float64)
		if uid <= 0 || folder == "" {
			return nil
		}
		messages = append(messages, storeMessage{folder: folder, messageID: fmt.Sprint(int64(uid))})
	}
	return messages
}

func trackNotification(t trackedNotification) {
	trackedMu.Lock()
	defer trackedMu.Unlock()
	trackedNotifications = append(trackedNotifications, t)
	if over := len(trackedNotifications) - maxTrackedNotifications; over > 0 {
		trackedNotifications = trackedNotifications[over:]
	}
}

// forgetNotification drops a notification that left the screen by other
// means: dismissed, clicked, expired, or withdrawn here.
func forgetNotification(handle string) {
	trackedMu.Lock()
	defer trackedMu.Unlock()
	kept := trackedNotifications[:0]
	for _, t := range trackedNotifications {
		if t.handle != handle {
			kept = append(kept, t)
		}
	}
	trackedNotifications = kept
}

// notifyReconcileCommands may leave a message no longer unread, or no longer
// there: a sync that brought in another device's changes, or the user's own
// read, move and delete writes. Matches the mobile clients' lists.
var notifyReconcileCommands = map[string]bool{
	"mail.sync": true, "mail.markRead": true, "mail.markAllRead": true, "mail.archive": true,
	"mail.delete": true, "mail.move": true, "mail.emptyFolder": true, "mail.folderDelete": true,
	"account.remove": true, "rss.removeFeed": true,
}

// requestNotificationReconcile coalesces a burst of syncs and writes into one
// reconcileNotifications pass.
func (a *App) requestNotificationReconcile() {
	trackedMu.Lock()
	defer trackedMu.Unlock()
	if len(trackedNotifications) == 0 {
		return
	}
	if notifyReconcileTimer != nil {
		notifyReconcileTimer.Stop()
	}
	notifyReconcileTimer = time.AfterFunc(notifyReconcileWindow, a.reconcileNotifications)
}

// reconcileNotifications withdraws new-mail notifications whose mail is no
// longer unread, the way Gmail does: read, archived or deleted here, or — once
// a sync has brought the change in — on another device. The notifications on
// screen are compared against the store rather than tracking each action.
func (a *App) reconcileNotifications() {
	trackedMu.Lock()
	tracked := append([]trackedNotification(nil), trackedNotifications...)
	trackedMu.Unlock()
	if len(tracked) == 0 || a.sidecar == nil || !a.sidecar.Started() {
		return
	}
	var messages []map[string]any
	for _, t := range tracked {
		for _, m := range t.messages {
			messages = append(messages, map[string]any{"account_id": t.account, "folder_id": m.folder, "message_id": m.messageID})
		}
	}
	result, err := a.sidecar.Call("messages.stillUnread", map[string]any{"messages": messages})
	if err != nil {
		a.logf("notification reconcile failed: %v", err)
		return
	}
	for _, handle := range staleNotifications(tracked, stillUnreadRows(result)) {
		a.withdrawNotification(handle)
		forgetNotification(handle)
	}
}

type storeRow struct {
	account string
	message storeMessage
}

func stillUnreadRows(result any) map[storeRow]bool {
	rows := map[storeRow]bool{}
	m, _ := result.(map[string]any)
	unread, _ := m["unread"].([]any)
	for _, raw := range unread {
		entry, _ := raw.(map[string]any)
		account, _ := entry["account_id"].(string)
		folder, _ := entry["folder_id"].(string)
		messageID, _ := entry["message_id"].(string)
		rows[storeRow{account: account, message: storeMessage{folder: folder, messageID: messageID}}] = true
	}
	return rows
}

// staleNotifications are those none of whose messages is still unread. One
// notification covers a whole batch, so it stays while any arrival is unread.
func staleNotifications(tracked []trackedNotification, stillUnread map[storeRow]bool) []string {
	var stale []string
	for _, t := range tracked {
		live := false
		for _, m := range t.messages {
			if stillUnread[storeRow{account: t.account, message: m}] {
				live = true
				break
			}
		}
		if !live {
			stale = append(stale, t.handle)
		}
	}
	return stale
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
