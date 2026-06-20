//go:build integration

package main

// End-to-end tests over the real stack: sidecar → SMTP submission → maddy →
// IMAP fetch → SQLite store. See maddy_harness_test.go for the setup.

import (
	"encoding/base64"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// pollFolder refreshes an account folder until predicate matches a message row
// (messages.recent triggers a background IMAP sync and serves the store cache).
func pollFolder(t *testing.T, sidecar *Sidecar, account, folder string, predicate func(map[string]any) bool) map[string]any {
	t.Helper()
	deadline := time.Now().Add(60 * time.Second)
	for time.Now().Before(deadline) {
		result := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": account,
			"folder":  folder,
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
	t.Fatalf("message did not arrive in %s %s within deadline", account, folder)
	return nil
}

func pollInbox(t *testing.T, sidecar *Sidecar, account string, predicate func(map[string]any) bool) map[string]any {
	t.Helper()
	return pollFolder(t, sidecar, account, "INBOX", predicate)
}

func assertNoMessageInFolder(t *testing.T, sidecar *Sidecar, account, folder string, predicate func(map[string]any) bool) {
	t.Helper()
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		result := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": account,
			"folder":  folder,
			"refresh": true,
			"limit":   50,
		})
		rows, _ := result["messages"].([]any)
		found := false
		for _, row := range rows {
			message, ok := row.(map[string]any)
			if ok && predicate(message) {
				found = true
				break
			}
		}
		if !found {
			return
		}
		time.Sleep(500 * time.Millisecond)
	}
	t.Fatalf("message still present in %s %s after deadline", account, folder)
}

func str(message map[string]any, key string) string {
	value, _ := message[key].(string)
	return value
}

func num(message map[string]any, key string) uint32 {
	switch value := message[key].(type) {
	case float64:
		return uint32(value)
	case int:
		return uint32(value)
	case uint32:
		return value
	default:
		return 0
	}
}

func boolValue(message map[string]any, key string) bool {
	value, _ := message[key].(bool)
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
		result = callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": "ITestFolder"})
		if !foldersContain(result, "ITestFolder") {
			t.Fatalf("folders.create for bob did not return ITestFolder: %v", result)
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

	t.Run("move and flags", func(t *testing.T) {
		moveSubject := "Meron integration move " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    moveSubject,
			"body":       "move me to the integration folder",
			"message_id": fmt.Sprintf("itest-move-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send move fixture: %v", err)
		}

		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == moveSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("delivered move fixture has no uid: %v", message)
		}

		callMap(t, sidecar, "messages.markRead", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"seen":    true,
		})
		callMap(t, sidecar, "messages.markStarred", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"starred": true,
		})

		updated := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == moveSubject && boolValue(m, "seen") && boolValue(m, "starred")
		})
		if num(updated, "uid") != uid {
			t.Fatalf("flagged message uid = %d, want %d", num(updated, "uid"), uid)
		}

		result := callMap(t, sidecar, "messages.move", map[string]any{
			"account":       "bob",
			"folder":        "INBOX",
			"target_folder": "ITestFolder",
			"uid":           uid,
		})
		if moved := num(result, "moved"); moved != 1 {
			t.Fatalf("messages.move moved = %d, want 1: %v", moved, result)
		}

		assertNoMessageInFolder(t, sidecar, "bob", "INBOX", func(m map[string]any) bool {
			return str(m, "subject") == moveSubject
		})
		moved := pollFolder(t, sidecar, "bob", "ITestFolder", func(m map[string]any) bool {
			return str(m, "subject") == moveSubject
		})
		if !boolValue(moved, "seen") || !boolValue(moved, "starred") {
			t.Fatalf("moved message lost flags: %v", moved)
		}
	})

	t.Run("search and starred items", func(t *testing.T) {
		searchSubject := "Meron integration search " + nonce
		searchBody := "unique-search-token-" + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    searchSubject,
			"body":       searchBody,
			"message_id": fmt.Sprintf("itest-search-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send search fixture: %v", err)
		}

		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == searchSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("delivered search fixture has no uid: %v", message)
		}

		search := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"query":   searchBody,
			"refresh": true,
			"limit":   50,
		})
		if !messagesContainSubject(search, searchSubject) {
			t.Fatalf("messages.recent query did not return %q: %v", searchSubject, search)
		}

		callMap(t, sidecar, "messages.markStarred", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"starred": true,
		})
		starred := callMap(t, sidecar, "starred.items", map[string]any{"limit": 50})
		if !starredMailContainsSubject(starred, searchSubject) {
			t.Fatalf("starred.items did not include %q: %v", searchSubject, starred)
		}
	})

	t.Run("draft lifecycle", func(t *testing.T) {
		draftID := fmt.Sprintf("itest-draft-%s@maddy.test", nonce)
		draftSubject := "Meron integration draft " + nonce
		if _, err := sidecar.Call("save_draft", map[string]any{
			"account":  "alice",
			"to":       "bob@maddy.test",
			"subject":  draftSubject,
			"body":     "draft body",
			"draft_id": draftID,
		}); err != nil {
			t.Fatalf("save_draft: %v", err)
		}

		draft := pollFolder(t, sidecar, "alice", "Drafts", func(m map[string]any) bool {
			return str(m, "subject") == draftSubject
		})
		if str(draft, "thread_key") == "" {
			t.Fatalf("draft has empty thread_key: %v", draft)
		}

		if _, err := sidecar.Call("discard_draft", map[string]any{
			"account":  "alice",
			"draft_id": draftID,
		}); err != nil {
			t.Fatalf("discard_draft: %v", err)
		}
		assertNoMessageInFolder(t, sidecar, "alice", "Drafts", func(m map[string]any) bool {
			return str(m, "subject") == draftSubject
		})
	})

	t.Run("attachments", func(t *testing.T) {
		attachmentSubject := "Meron integration attachment " + nonce
		attachmentBody := "attachment body " + nonce
		attachmentBytes := []byte("hello attachment " + nonce)
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    attachmentSubject,
			"body":       attachmentBody,
			"message_id": fmt.Sprintf("itest-attachment-%s@maddy.test", nonce),
			"attachments": []map[string]any{{
				"filename":  "itest-note.txt",
				"mime":      "text/plain",
				"data":      base64.StdEncoding.EncodeToString(attachmentBytes),
				"inline_id": "",
			}},
		}); err != nil {
			t.Fatalf("send attachment fixture: %v", err)
		}

		header := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == attachmentSubject
		})
		threadKey := str(header, "thread_key")
		if threadKey == "" {
			t.Fatalf("attachment fixture has empty thread_key: %v", header)
		}

		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": threadKey,
		})
		message := firstThreadMessage(t, thread)
		attachments := attachmentRows(t, message)
		if len(attachments) != 1 {
			t.Fatalf("attachments len = %d, want 1: %v", len(attachments), message)
		}
		attachment := attachments[0]
		if str(attachment, "filename") != "itest-note.txt" {
			t.Fatalf("attachment filename = %q", str(attachment, "filename"))
		}
		if str(attachment, "mime") != "text/plain" {
			t.Fatalf("attachment mime = %q", str(attachment, "mime"))
		}
		if size := num(attachment, "size"); size != uint32(len(attachmentBytes)) {
			t.Fatalf("attachment size = %d, want %d", size, len(attachmentBytes))
		}
		key := str(attachment, "key")
		if key == "" {
			t.Fatalf("attachment key is empty: %v", attachment)
		}
		got, err := os.ReadFile(filepath.Join(mediaDir(), key))
		if err != nil {
			t.Fatalf("read cached attachment %q: %v", key, err)
		}
		if string(got) != string(attachmentBytes) {
			t.Fatalf("cached attachment bytes = %q, want %q", got, attachmentBytes)
		}
	})

	t.Run("delete moves to trash", func(t *testing.T) {
		// Delete is destructive and folder-aware: a non-draft inbox message must
		// land in Trash (not expunge), so a stray UID never silently vanishes.
		deleteSubject := "Meron integration delete " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    deleteSubject,
			"body":       "delete me",
			"message_id": fmt.Sprintf("itest-delete-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send delete fixture: %v", err)
		}
		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == deleteSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("delete fixture has no uid: %v", message)
		}

		result := callMap(t, sidecar, "messages.delete", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
		})
		if trash, _ := result["trash"].(string); trash == "" {
			t.Fatalf("messages.delete did not report a trash folder: %v", result)
		}

		assertNoMessageInFolder(t, sidecar, "bob", "INBOX", func(m map[string]any) bool {
			return str(m, "subject") == deleteSubject
		})
		pollFolder(t, sidecar, "bob", "Trash", func(m map[string]any) bool {
			return str(m, "subject") == deleteSubject
		})
	})

	t.Run("copy keeps original", func(t *testing.T) {
		// Copy must duplicate, not move: the source UID stays put while a copy
		// appears in the target folder.
		copySubject := "Meron integration copy " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    copySubject,
			"body":       "copy me",
			"message_id": fmt.Sprintf("itest-copy-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send copy fixture: %v", err)
		}
		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == copySubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("copy fixture has no uid: %v", message)
		}

		result := callMap(t, sidecar, "messages.copy", map[string]any{
			"account":        "bob",
			"folder":         "INBOX",
			"target_account": "bob",
			"target_folder":  "ITestFolder",
			"uid":            uid,
		})
		if copied := num(result, "copied"); copied != 1 {
			t.Fatalf("messages.copy copied = %d, want 1: %v", copied, result)
		}

		pollFolder(t, sidecar, "bob", "ITestFolder", func(m map[string]any) bool {
			return str(m, "subject") == copySubject
		})
		// The original must still be in INBOX.
		original := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"refresh": true,
			"limit":   50,
		})
		if !messagesContainSubject(original, copySubject) {
			t.Fatalf("copy removed the original from INBOX: %v", original)
		}
	})

	t.Run("mark all read", func(t *testing.T) {
		readSubjectA := "Meron integration markall A " + nonce
		readSubjectB := "Meron integration markall B " + nonce
		for i, subj := range []string{readSubjectA, readSubjectB} {
			if _, err := sidecar.Call("send", map[string]any{
				"account":    "alice",
				"to":         "bob@maddy.test",
				"subject":    subj,
				"body":       "unread fixture",
				"message_id": fmt.Sprintf("itest-markall-%d-%s@maddy.test", i, nonce),
			}); err != nil {
				t.Fatalf("send markall fixture %d: %v", i, err)
			}
		}
		// Wait for both to land while still unseen.
		for _, subj := range []string{readSubjectA, readSubjectB} {
			pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
				return str(m, "subject") == subj && !boolValue(m, "seen")
			})
		}

		callMap(t, sidecar, "messages.markAllRead", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
		})

		for _, subj := range []string{readSubjectA, readSubjectB} {
			pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
				return str(m, "subject") == subj && boolValue(m, "seen")
			})
		}
	})

	t.Run("contacts suggest from history", func(t *testing.T) {
		// bob's mailbox holds messages from alice, so a prefix query against
		// bob's history must surface alice's address.
		result := callMap(t, sidecar, "contacts.suggest", map[string]any{
			"account": "bob",
			"query":   "alice",
			"limit":   8,
		})
		contacts, _ := result["contacts"].([]any)
		found := false
		for _, item := range contacts {
			contact, ok := item.(map[string]any)
			if ok && strings.Contains(str(contact, "addr"), "alice@maddy.test") {
				found = true
				break
			}
		}
		if !found {
			t.Fatalf("contacts.suggest(%q) did not surface alice: %v", "alice", result)
		}
	})

	t.Run("archive folder resolves", func(t *testing.T) {
		// folders.archive resolves the account's special-use Archive folder; the
		// move-to-archive action depends on this lookup succeeding.
		result := callMap(t, sidecar, "folders.archive", map[string]any{"account": "bob"})
		if folder := str(result, "folder"); folder == "" {
			t.Fatalf("folders.archive returned no folder: %v", result)
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

func messagesContainSubject(result map[string]any, subject string) bool {
	rows, _ := result["messages"].([]any)
	for _, row := range rows {
		message, ok := row.(map[string]any)
		if ok && str(message, "subject") == subject {
			return true
		}
	}
	return false
}

func starredMailContainsSubject(result map[string]any, subject string) bool {
	rows, _ := result["mail"].([]any)
	for _, row := range rows {
		message, ok := row.(map[string]any)
		if ok && str(message, "subject") == subject {
			return true
		}
	}
	return false
}

func firstThreadMessage(t *testing.T, result map[string]any) map[string]any {
	t.Helper()
	rows, _ := result["messages"].([]any)
	if len(rows) == 0 {
		t.Fatalf("thread has no messages: %v", result)
	}
	row, ok := rows[0].(map[string]any)
	if !ok {
		t.Fatalf("thread row has type %T", rows[0])
	}
	message, ok := row["message"].(map[string]any)
	if !ok {
		t.Fatalf("thread row message has type %T: %v", row["message"], row)
	}
	return message
}

func attachmentRows(t *testing.T, message map[string]any) []map[string]any {
	t.Helper()
	raw, _ := message["attachments"].([]any)
	out := make([]map[string]any, 0, len(raw))
	for _, item := range raw {
		attachment, ok := item.(map[string]any)
		if !ok {
			t.Fatalf("attachment has type %T", item)
		}
		out = append(out, attachment)
	}
	return out
}
