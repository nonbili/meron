package main

import (
	"bytes"
	"log"
	"os"
	"path/filepath"
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
