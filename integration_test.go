//go:build integration

package main

// End-to-end tests over the real stack: sidecar → SMTP submission → maddy →
// IMAP fetch → SQLite store. See maddy_harness_test.go for the setup.

import (
	"fmt"
	"strings"
	"testing"
	"time"
)

// pollInbox refreshes an account's INBOX until predicate matches a message row
// (messages.recent triggers a background IMAP sync and serves the store cache).
func pollInbox(t *testing.T, sidecar *Sidecar, account string, predicate func(map[string]any) bool) map[string]any {
	t.Helper()
	deadline := time.Now().Add(60 * time.Second)
	for time.Now().Before(deadline) {
		result := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": account,
			"folder":  "INBOX",
			"refresh": true,
			"limit":   50,
		})
		rows, _ := result["messages"].([]any)
		for _, row := range rows {
			message, ok := row.(map[string]any)
			if ok && predicate(message) {
				return message
			}
		}
		time.Sleep(500 * time.Millisecond)
	}
	t.Fatalf("message did not arrive in %s INBOX within deadline", account)
	return nil
}

func str(message map[string]any, key string) string {
	value, _ := message[key].(string)
	return value
}

func TestIntegrationMailFlow(t *testing.T) {
	server := startMaddy(t)
	sidecar := startSidecar(t)

	connectAccount(t, sidecar, server, "alice", "alice@maddy.test")
	connectAccount(t, sidecar, server, "bob", "bob@maddy.test")

	t.Run("folders", func(t *testing.T) {
		// folders.list serves the store cache and refreshes in the background,
		// so poll until the IMAP folder sync lands.
		deadline := time.Now().Add(30 * time.Second)
		for {
			result := callMap(t, sidecar, "folders.list", map[string]any{"account": "alice"})
			if foldersContain(result, "INBOX") {
				break
			}
			if time.Now().After(deadline) {
				t.Fatalf("INBOX never appeared in folders.list: %v", result)
			}
			time.Sleep(300 * time.Millisecond)
		}

		// Not "Archive"/"Sent"/etc — maddy pre-creates the special folders.
		result := callMap(t, sidecar, "folders.create", map[string]any{"account": "alice", "name": "ITestFolder"})
		if !foldersContain(result, "ITestFolder") {
			t.Fatalf("folders.create did not return ITestFolder: %v", result)
		}
	})

	nonce := fmt.Sprintf("%d", time.Now().UnixNano())
	subject := "Meron integration " + nonce
	// Bare id (no angle brackets) — the app convention: the frontend mints bare
	// ids and the backend wraps them when emitting headers.
	messageID := fmt.Sprintf("itest-%s@maddy.test", nonce)

	t.Run("send and receive", func(t *testing.T) {
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    subject,
			"body":       "hello from the integration test",
			"message_id": messageID,
		}); err != nil {
			t.Fatalf("send: %v", err)
		}

		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == subject
		})
		if from := str(message, "from_addr"); from != "alice@maddy.test" {
			t.Errorf("from_addr = %q, want alice@maddy.test", from)
		}
		if str(message, "thread_key") == "" {
			t.Error("delivered message has empty thread_key")
		}
	})

	t.Run("threading", func(t *testing.T) {
		// A reply (References/In-Reply-To pointing at the first message) must
		// land in the same thread as the original in bob's mailbox.
		if _, err := sidecar.Call("send", map[string]any{
			"account":     "alice",
			"to":          "bob@maddy.test",
			"subject":     "Re: " + subject,
			"body":        "follow-up",
			"in_reply_to": messageID,
			"references":  messageID,
		}); err != nil {
			t.Fatalf("send reply: %v", err)
		}

		reply := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == "Re: "+subject
		})
		original := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == subject
		})
		if str(reply, "thread_key") != str(original, "thread_key") {
			t.Errorf("reply thread_key %q != original thread_key %q",
				str(reply, "thread_key"), str(original, "thread_key"))
		}

		result := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": str(original, "thread_key"),
		})
		if n := threadLength(result); n < 2 {
			t.Errorf("messages.thread returned %d messages, want >= 2: %v", n, result)
		}
	})
}

func foldersContain(result map[string]any, name string) bool {
	folders, _ := result["folders"].([]any)
	for _, item := range folders {
		folder, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if value, _ := folder["name"].(string); strings.EqualFold(value, name) {
			return true
		}
	}
	return false
}

func threadLength(result map[string]any) int {
	messages, _ := result["messages"].([]any)
	return len(messages)
}
