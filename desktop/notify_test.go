package main

import (
	"bytes"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"testing"
)

// The sidecar emits the branch-aware card key (subject branching lives in the
// core); notificationThreadID only canonicalizes the folder and formats the id
// so it matches the list card the user clicks.
func TestNotificationThreadIDPassesCardKeyThrough(t *testing.T) {
	const (
		account   = "acc-1"
		threadKey = "<abc@example.com>#build failed"
	)

	// The mail.newMessages event sends folder:"inbox"; the id must canonicalize to
	// "INBOX" to match the list builder.
	got := notificationThreadID(account, "inbox", threadKey)
	want := formatImapThreadID(account, "INBOX", threadKey)
	if got != want {
		t.Fatalf("notificationThreadID() = %q, want %q", got, want)
	}
}

func TestNotificationThreadIDUIDKeyHasNoSubject(t *testing.T) {
	// uid: keys are already unique; appending a subject would break the match
	// (the list builder skips the subject for uid: keys too).
	got := notificationThreadID("acc-1", "inbox", "uid:42")
	want := formatImapThreadID("acc-1", "INBOX", "uid:42")
	if got != want {
		t.Fatalf("notificationThreadID() = %q, want %q", got, want)
	}
}

func TestNotificationThreadIDGmailThreadIDHasNoSubject(t *testing.T) {
	got := notificationThreadID("acc-1", "inbox", "gmthrid:123")
	want := formatImapThreadID("acc-1", "INBOX", "gmthrid:123")
	if got != want {
		t.Fatalf("notificationThreadID() = %q, want %q", got, want)
	}
}

func TestNotificationThreadIDEmptyWithoutKey(t *testing.T) {
	if got := notificationThreadID("", "INBOX", "k"); got != "" {
		t.Errorf("missing account: got %q, want empty", got)
	}
	if got := notificationThreadID("acc-1", "INBOX", ""); got != "" {
		t.Errorf("missing threadKey: got %q, want empty", got)
	}
}

// formatImapThreadID must fold the inbox folder casing so the same thread gets
// one id no matter which path mints it. The thread-list builder spells the inbox
// "inbox" (the UI folder id, since get_recent_page returns blank per-message
// folders), while the notification/cached-row paths use "INBOX". If these
// diverged, a notification-opened thread's selection never matched its list card
// and quick replies fell back to the header-less card — going out unthreaded.
func TestFormatImapThreadIDFoldsInboxCasing(t *testing.T) {
	const (
		account     = "acc-1"
		compoundKey = "<abc@example.com>#a subject"
	)
	lower := formatImapThreadID(account, "inbox", compoundKey)
	upper := formatImapThreadID(account, "INBOX", compoundKey)
	if lower != upper {
		t.Fatalf("inbox casing not folded: %q != %q", lower, upper)
	}
	// A notification (folder "inbox" from the newMessages event) and the list
	// card (folder "inbox" from threadList's default) must resolve identically.
	notif := notificationThreadID(account, "inbox", compoundKey)
	card := formatImapThreadID(account, "inbox", compoundKey)
	if notif != card {
		t.Fatalf("notification id %q != list card id %q", notif, card)
	}
	// Non-inbox folders keep their exact casing (they're real IMAP names).
	if got := formatImapThreadID(account, "Archive", compoundKey); got == formatImapThreadID(account, "archive", compoundKey) {
		t.Fatalf("non-inbox folder casing should not be folded")
	}
}

func TestNotificationThreadIDRSS(t *testing.T) {
	got := notificationThreadID("rss-feed1", "", "item-key")
	want := "rss-feed1#rss#item-key"
	if got != want {
		t.Fatalf("notificationThreadID() = %q, want %q", got, want)
	}
}

func TestFirstNonEmpty(t *testing.T) {
	if got := firstNonEmpty("", "", "fallback", "later"); got != "fallback" {
		t.Fatalf("firstNonEmpty returned %q", got)
	}
	if got := firstNonEmpty("", ""); got != "" {
		t.Fatalf("firstNonEmpty empty values returned %q", got)
	}
}

func TestFormatNativePlural(t *testing.T) {
	template := "{count, plural, one {Eine neue Nachricht} other {{count} neue Nachrichten}}"
	if got := formatNativePlural(template, 1); got != "Eine neue Nachricht" {
		t.Fatalf("singular = %q", got)
	}
	if got := formatNativePlural(template, 3); got != "3 neue Nachrichten" {
		t.Fatalf("plural = %q", got)
	}
}

func TestCheckProtocolVersionLogsMissingAndMismatch(t *testing.T) {
	var buf bytes.Buffer
	app := &App{logger: log.New(&buf, "", 0)}

	app.checkProtocolVersion(nil)
	if got := buf.String(); !strings.Contains(got, "missing protocol version") {
		t.Fatalf("missing protocol log = %q", got)
	}

	buf.Reset()
	app.checkProtocolVersion(map[string]any{"protocol": float64(expectedProtocolVersion + 1)})
	if got := buf.String(); !strings.Contains(got, "protocol mismatch") {
		t.Fatalf("mismatch protocol log = %q", got)
	}

	buf.Reset()
	app.checkProtocolVersion(map[string]any{"protocol": float64(expectedProtocolVersion)})
	if got := buf.String(); got != "" {
		t.Fatalf("matching protocol logged %q, want empty", got)
	}
}

func TestNotifyIconWritesEmbeddedIconOnce(t *testing.T) {
	oldOnce := notifyIconOnce
	oldPath := notifyIconPath
	notifyIconOnce = sync.Once{}
	notifyIconPath = ""
	t.Cleanup(func() {
		notifyIconOnce = oldOnce
		notifyIconPath = oldPath
	})

	path := notifyIcon()
	if len(appIconPNG) == 0 {
		if path != "" {
			t.Fatalf("notifyIcon path = %q with empty appIconPNG", path)
		}
		return
	}
	if path == "" {
		t.Fatal("notifyIcon returned empty path")
	}
	if filepath.Base(path) != "meron-notify.png" {
		t.Fatalf("notifyIcon path = %q", path)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read notify icon: %v", err)
	}
	if !bytes.Equal(data, appIconPNG) {
		t.Fatalf("notify icon data length = %d, want %d matching bytes", len(data), len(appIconPNG))
	}
	if got := notifyIcon(); got != path {
		t.Fatalf("second notifyIcon() = %q, want %q", got, path)
	}
}

func TestParseStoreMessagesNamesEveryArrival(t *testing.T) {
	mail := parseStoreMessages(map[string]any{
		"account":  "me@example.com",
		"folder":   "inbox",
		"messages": []any{map[string]any{"uid": float64(41)}, map[string]any{"uid": float64(42)}},
	}, 2)
	want := []storeMessage{{folder: "inbox", messageID: "41"}, {folder: "inbox", messageID: "42"}}
	if !reflect.DeepEqual(mail, want) {
		t.Fatalf("mail = %#v, want %#v", mail, want)
	}

	// A feed entry lives under its subscription, not the batch folder.
	feed := parseStoreMessages(map[string]any{
		"account":  "rss-8f2a",
		"folder":   "rss",
		"messages": []any{map[string]any{"uid": float64(0), "itemKey": "key-1", "threadKey": "sub-1"}},
	}, 1)
	if want := []storeMessage{{folder: "sub-1", messageID: "key-1"}}; !reflect.DeepEqual(feed, want) {
		t.Fatalf("feed = %#v, want %#v", feed, want)
	}

	// More arrivals than listed: withdrawing on the listed ones would hide
	// mail still unread, so the notification is left to the user.
	partial := parseStoreMessages(map[string]any{
		"account":  "me@example.com",
		"folder":   "inbox",
		"messages": []any{map[string]any{"uid": float64(41)}},
	}, 30)
	if partial != nil {
		t.Fatalf("partial = %#v, want nil", partial)
	}
}

func TestStaleNotificationsWaitForEveryArrivalToBeRead(t *testing.T) {
	first := storeMessage{folder: "inbox", messageID: "41"}
	second := storeMessage{folder: "inbox", messageID: "42"}
	tracked := []trackedNotification{
		{handle: "batch", account: "me", messages: []storeMessage{first, second}},
		{handle: "single", account: "me", messages: []storeMessage{first}},
		{handle: "elsewhere", account: "other", messages: []storeMessage{second}},
	}
	stillUnread := stillUnreadRows(map[string]any{"unread": []any{
		map[string]any{"account_id": "me", "folder_id": "inbox", "message_id": "42"},
	}})
	// The same UID in another account is a different message.
	if got := staleNotifications(tracked, stillUnread); !reflect.DeepEqual(got, []string{"single", "elsewhere"}) {
		t.Fatalf("stale = %v", got)
	}
}

func TestTrackedNotificationsStayBounded(t *testing.T) {
	trackedMu.Lock()
	saved := trackedNotifications
	trackedNotifications = nil
	trackedMu.Unlock()
	defer func() {
		trackedMu.Lock()
		trackedNotifications = saved
		trackedMu.Unlock()
	}()

	for i := 0; i < maxTrackedNotifications+5; i++ {
		trackNotification(trackedNotification{handle: fmt.Sprint(i)})
	}
	forgetNotification(fmt.Sprint(maxTrackedNotifications + 4))
	trackedMu.Lock()
	defer trackedMu.Unlock()
	if len(trackedNotifications) != maxTrackedNotifications-1 || trackedNotifications[0].handle != "5" {
		t.Fatalf("tracked = %d entries starting at %q", len(trackedNotifications), trackedNotifications[0].handle)
	}
}
