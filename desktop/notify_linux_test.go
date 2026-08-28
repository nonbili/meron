//go:build linux

package main

import (
	"testing"

	"github.com/godbus/dbus/v5"
)

func TestPortalNotificationOptions(t *testing.T) {
	options := portalNotificationOptions(notification{
		title:    "Alice",
		body:     "Build finished",
		account:  "account-1",
		threadID: "account-1#INBOX#message-1",
	})

	for key, want := range map[string]string{
		"title":          "Alice",
		"body":           "Build finished",
		"priority":       "normal",
		"default-action": portalNotificationOpenAction,
	} {
		got, ok := options[key].Value().(string)
		if !ok || got != want {
			t.Errorf("%s = %#v, want %q", key, options[key].Value(), want)
		}
	}
	if signature := options["icon"].Signature().String(); signature != "(sv)" {
		t.Errorf("icon signature = %q, want %q", signature, "(sv)")
	}

	target, ok := portalNotificationTarget([]any{
		"notification-id",
		portalNotificationOpenAction,
		[]dbus.Variant{options["default-action-target"]},
	})
	if !ok {
		t.Fatal("portalNotificationTarget rejected valid target")
	}
	if target.Account != "account-1" || target.ThreadID != "account-1#INBOX#message-1" {
		t.Fatalf("target = %#v", target)
	}
}

func TestPortalNotificationOptionsWithoutTarget(t *testing.T) {
	options := portalNotificationOptions(notification{
		title: "New messages",
		body:  "Three new messages",
	})
	if _, ok := options["default-action"]; ok {
		t.Error("notification without a thread has a default action")
	}
	if _, ok := options["default-action-target"]; ok {
		t.Error("notification without a thread has a default action target")
	}
}

func TestTakeFdoNotificationTarget(t *testing.T) {
	notificationMu.Lock()
	fdoNotificationTargets[7] = fdoNotificationTarget{
		target:           notificationTarget{Account: "account-1", ThreadID: "account-1#INBOX#message-1"},
		responseSequence: 10,
	}
	notificationMu.Unlock()
	t.Cleanup(func() {
		notificationMu.Lock()
		delete(fdoNotificationTargets, 7)
		notificationMu.Unlock()
	})

	target, ok := takeFdoNotificationTarget([]any{uint32(7), fdoNotificationDefaultAction}, 11)
	if !ok {
		t.Fatal("takeFdoNotificationTarget rejected a registered notification")
	}
	if target.Account != "account-1" || target.ThreadID != "account-1#INBOX#message-1" {
		t.Fatalf("target = %#v", target)
	}
	// The entry is consumed: ids are reused, so a later signal carrying the same
	// id must not reopen this thread.
	if _, ok := takeFdoNotificationTarget([]any{uint32(7), fdoNotificationDefaultAction}, 12); ok {
		t.Error("takeFdoNotificationTarget returned a consumed target")
	}
}

func TestTakeFdoNotificationTargetRejectsInvalidSignal(t *testing.T) {
	notificationMu.Lock()
	fdoNotificationTargets[9] = fdoNotificationTarget{
		target:           notificationTarget{Account: "account-1", ThreadID: "thread-1"},
		responseSequence: 10,
	}
	notificationMu.Unlock()
	t.Cleanup(func() {
		notificationMu.Lock()
		delete(fdoNotificationTargets, 9)
		notificationMu.Unlock()
	})

	tests := [][]any{
		nil,
		{uint32(9)},
		{"9", fdoNotificationDefaultAction},
		{uint32(9), "other-action"},
		// Another app's notification, broadcast to every listener.
		{uint32(1234), fdoNotificationDefaultAction},
	}
	for _, body := range tests {
		if target, ok := takeFdoNotificationTarget(body, 11); ok {
			t.Errorf("takeFdoNotificationTarget(%#v) = %#v, want rejection", body, target)
		}
	}
}

func TestFdoNotificationTargetIgnoresSignalsFromReusedID(t *testing.T) {
	notificationMu.Lock()
	fdoNotificationTargets[11] = fdoNotificationTarget{
		target:           notificationTarget{Account: "account-2", ThreadID: "thread-2"},
		responseSequence: 20,
	}
	notificationMu.Unlock()
	t.Cleanup(func() {
		notificationMu.Lock()
		delete(fdoNotificationTargets, 11)
		notificationMu.Unlock()
	})

	// These signals were received before the Notify reply and therefore belong
	// to the previous notification that used id 11, even though they are handled
	// after the replacement target has been registered.
	closeFdoNotificationTarget([]any{uint32(11), uint32(2)}, 19)
	if _, ok := takeFdoNotificationTarget(
		[]any{uint32(11), fdoNotificationDefaultAction},
		19,
	); ok {
		t.Fatal("stale action consumed the replacement notification target")
	}

	target, ok := takeFdoNotificationTarget(
		[]any{uint32(11), fdoNotificationDefaultAction},
		21,
	)
	if !ok {
		t.Fatal("replacement notification target was removed by a stale signal")
	}
	if target.Account != "account-2" || target.ThreadID != "thread-2" {
		t.Fatalf("target = %#v", target)
	}
}

func TestLinuxDesktopEntry(t *testing.T) {
	t.Setenv("FLATPAK_ID", "")
	t.Setenv("SNAP_NAME", "")
	if got := linuxDesktopEntry(); got != "meron" {
		t.Errorf("linuxDesktopEntry() = %q, want %q", got, "meron")
	}

	t.Setenv("SNAP_NAME", "meron")
	if got := linuxDesktopEntry(); got != "meron_meron" {
		t.Errorf("snap linuxDesktopEntry() = %q, want %q", got, "meron_meron")
	}

	t.Setenv("FLATPAK_ID", "jp.nonbili.meron")
	if got := linuxDesktopEntry(); got != "jp.nonbili.meron" {
		t.Errorf("flatpak linuxDesktopEntry() = %q, want %q", got, "jp.nonbili.meron")
	}
}

func TestPortalNotificationTargetRejectsInvalidSignal(t *testing.T) {
	tests := [][]any{
		nil,
		{"id", "wrong-action", []dbus.Variant{dbus.MakeVariant(`{"account":"a","threadId":"t"}`)}},
		{"id", portalNotificationOpenAction, []dbus.Variant{}},
		{"id", portalNotificationOpenAction, []dbus.Variant{dbus.MakeVariant("invalid")}},
		{"id", portalNotificationOpenAction, []dbus.Variant{dbus.MakeVariant(`{"account":"","threadId":"t"}`)}},
	}
	for _, body := range tests {
		if target, ok := portalNotificationTarget(body); ok {
			t.Errorf("portalNotificationTarget(%#v) = %#v, want rejection", body, target)
		}
	}
}
