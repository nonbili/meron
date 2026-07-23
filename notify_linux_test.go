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
