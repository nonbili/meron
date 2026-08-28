//go:build integration

package main

// End-to-end tests over the real stack: sidecar → SMTP submission → maddy →
// IMAP fetch → SQLite store. See maddy_harness_test.go for the setup.

import (
	"bufio"
	"encoding/base64"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
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

func TestIMAPResponseParsing(t *testing.T) {
	t.Run("tagged status is case insensitive", func(t *testing.T) {
		if !imapTaggedOK("t4 ok completed", "t4") {
			t.Fatal("lowercase tagged OK was rejected")
		}
		if imapTaggedOK("t4 NO failed", "t4") {
			t.Fatal("tagged NO was accepted")
		}
		if imapTaggedOK("t40 OK completed", "t4") {
			t.Fatal("different tag was accepted")
		}
	})

	t.Run("uidvalidity tolerates response text and casing", func(t *testing.T) {
		got, ok := parseIMAPUIDValidity([]string{
			"* FLAGS (\\Seen)",
			"* ok [uidvalidity 4294967295] UIDs valid",
		})
		if !ok || got != ^uint32(0) {
			t.Fatalf("UIDVALIDITY = %d, %v; want %d, true", got, ok, ^uint32(0))
		}
		for _, lines := range [][]string{
			{"* OK mailbox selected"},
			{"* OK [UIDVALIDITY 0] invalid"},
			{"* OK [UIDVALIDITY nope] invalid"},
		} {
			if got, ok := parseIMAPUIDValidity(lines); ok || got != 0 {
				t.Fatalf("invalid UIDVALIDITY parsed as %d, %v from %v", got, ok, lines)
			}
		}
	})

	t.Run("flags tolerate reordered fetch fields", func(t *testing.T) {
		lines := []string{
			"* 3 FETCH (FLAGS (\\Seen custom) RFC822.SIZE 42 uid 77)",
			"* 4 FETCH (UID 78 FLAGS (\\Flagged))",
		}
		if got := parseIMAPFlags(lines, 77); got != `\Seen custom` {
			t.Fatalf("flags for UID 77 = %q", got)
		}
		if got := parseIMAPFlags(lines, 78); got != `\Flagged` {
			t.Fatalf("flags for UID 78 = %q", got)
		}
		if got := parseIMAPFlags(lines, 79); got != "" {
			t.Fatalf("flags for absent UID = %q", got)
		}
	})

	t.Run("search handles empty and malformed fields", func(t *testing.T) {
		got := parseIMAPSearch([]string{
			"* SEARCH 2 nope 3x 0 17",
			"* VANISHED 99",
		})
		if fmt.Sprint(got) != "[2 17]" {
			t.Fatalf("SEARCH UIDs = %v, want [2 17]", got)
		}
		if got := parseIMAPSearch([]string{"* SEARCH"}); len(got) != 0 {
			t.Fatalf("empty SEARCH = %v", got)
		}
	})
}

func TestIntegrationMailFlow(t *testing.T) {
	server := startMaddy(t)
	sidecar, events := startSidecar(t)

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

		// Deleting on the server: an ordinary folder goes with its cached mail,
		// while a special-use folder is refused before any IMAP work.
		result = callMap(t, sidecar, "folders.create", map[string]any{"account": "alice", "name": "ITestDeleteMe"})
		if !foldersContain(result, "ITestDeleteMe") {
			t.Fatalf("folders.create did not return ITestDeleteMe: %v", result)
		}
		result = callMap(t, sidecar, "folders.delete", map[string]any{"account": "alice", "folder": "ITestDeleteMe"})
		if foldersContain(result, "ITestDeleteMe") {
			t.Fatalf("folders.delete left ITestDeleteMe in the folder list: %v", result)
		}
		if _, err := sidecar.Call("folders.delete", map[string]any{"account": "alice", "folder": "INBOX"}); err == nil {
			t.Fatal("folders.delete accepted INBOX, want refusal")
		}

		// A real hierarchy is removed leaf-first, and the response names every
		// mailbox that disappeared so clients can clear nested views.
		delimiter := folderDelimiter(result, "INBOX")
		if delimiter == "" {
			t.Fatal("maddy reported no hierarchy delimiter for INBOX")
		}
		parent := "ITestDeleteTree"
		child := parent + delimiter + "Child"
		callMap(t, sidecar, "folders.create", map[string]any{"account": "alice", "name": parent})
		callMap(t, sidecar, "folders.create", map[string]any{"account": "alice", "name": child})
		deadline = time.Now().Add(30 * time.Second)
		for {
			result = callMap(t, sidecar, "folders.list", map[string]any{"account": "alice"})
			if foldersContain(result, parent) &&
				foldersContain(result, child) &&
				folderDelimiter(result, parent) == delimiter {
				break
			}
			if time.Now().After(deadline) {
				t.Fatalf("nested folders never refreshed with delimiter %q: %v", delimiter, result)
			}
			time.Sleep(300 * time.Millisecond)
		}
		result = callMap(t, sidecar, "folders.delete", map[string]any{"account": "alice", "folder": parent})
		if foldersContain(result, parent) || foldersContain(result, child) {
			t.Fatalf("folders.delete left part of the subtree cached: %v", result)
		}
		removed, _ := result["removed"].([]any)
		if len(removed) != 2 || removed[0] != child || removed[1] != parent {
			t.Fatalf("removed = %#v, want [%q %q]", removed, child, parent)
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
		// The rows above came out of the same store the flag write updated, so
		// check the server too: a STORE that never left the client would look
		// identical locally.
		if flags := imapFlags(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", uid); !strings.Contains(flags, `\Seen`) || !strings.Contains(flags, `\Flagged`) {
			t.Fatalf("server flags for INBOX uid %d = %q, want \\Seen and \\Flagged", uid, flags)
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
		// A MOVE re-APPENDs at the destination, so the flags have to survive the
		// copy on the server, under the new UID.
		movedUID := num(moved, "uid")
		if movedUID == 0 {
			t.Fatalf("moved message has no uid: %v", moved)
		}
		if flags := imapFlags(t, server.imapPort, "bob@maddy.test", testPassword, "ITestFolder", movedUID); !strings.Contains(flags, `\Seen`) || !strings.Contains(flags, `\Flagged`) {
			t.Fatalf("server flags for ITestFolder uid %d = %q, want \\Seen and \\Flagged", movedUID, flags)
		}
	})

	t.Run("failed imap write does not falsely mark read locally", func(t *testing.T) {
		// Regression test: messages.markRead/markStarred/markAllRead must not
		// commit the local "seen"/"starred" state when the IMAP STORE itself
		// fails (dropped connection, offline, etc) — otherwise the local store
		// and every unread badge built on it silently drift from server truth.
		flagFailSubject := "Meron integration flagfail " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    flagFailSubject,
			"body":       "should stay unread after a failed write",
			"message_id": fmt.Sprintf("itest-flagfail-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send flagfail fixture: %v", err)
		}
		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == flagFailSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("delivered flagfail fixture has no uid: %v", message)
		}

		docker := dockerBin(t)
		network := runCmd(t, docker, "inspect", "-f", "{{range $net,$v := .NetworkSettings.Networks}}{{$net}}{{end}}", server.container)
		if network == "" {
			t.Skip("could not determine container network; skipping simulated IMAP outage")
		}

		runCmd(t, docker, "network", "disconnect", network, server.container)
		reconnected := false
		reconnect := func() {
			if reconnected {
				return
			}
			reconnected = true
			if out, err := exec.Command(docker, "network", "connect", network, server.container).CombinedOutput(); err != nil {
				t.Logf("docker network connect %s %s: %v\n%s", network, server.container, err, out)
			}
			waitForIMAPGreeting(t, server.imapPort, docker, server.container)
			// with_write_session doesn't retry a stale pooled connection (only
			// reads do), so both accounts can be left holding a dead socket from
			// the outage above. Pause/resume forces the engine to drop pooled
			// sessions, so later subtests in this shared run don't inherit them.
			for _, account := range []string{"alice", "bob"} {
				callMap(t, sidecar, "account.setPaused", map[string]any{"account": account, "enabled": true})
				callMap(t, sidecar, "account.setPaused", map[string]any{"account": account, "enabled": false})
			}
		}
		defer reconnect()

		if _, err := sidecar.Call("messages.markRead", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"seen":    true,
		}); err == nil {
			t.Fatalf("messages.markRead succeeded despite a severed IMAP connection")
		}

		reconnect()

		stillUnread := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == flagFailSubject
		})
		if boolValue(stillUnread, "seen") {
			t.Fatalf("message was marked seen locally despite the IMAP write having failed: %v", stillUnread)
		}

		// A retry once connectivity is restored should succeed normally.
		callMap(t, sidecar, "messages.markRead", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"seen":    true,
		})
		updated := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == flagFailSubject && boolValue(m, "seen")
		})
		if num(updated, "uid") != uid {
			t.Fatalf("flagged message uid = %d, want %d", num(updated, "uid"), uid)
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

	t.Run("quick reply draft lifecycle", func(t *testing.T) {
		// The quick reply saves a *reply* draft: it must thread into the
		// conversation it answers (that is what the frontend hydrates the box
		// from), and discard_draft with a thread_key must scrub it from the
		// thread so a cleared reply cannot resurface on the next thread open.
		quickSubject := "Meron integration quick reply " + nonce
		quickMessageID := fmt.Sprintf("itest-qr-%s@maddy.test", nonce)
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    quickSubject,
			"body":       "please reply inline",
			"message_id": quickMessageID,
		}); err != nil {
			t.Fatalf("send quick reply fixture: %v", err)
		}
		original := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == quickSubject
		})
		threadKey := str(original, "thread_key")
		if threadKey == "" {
			t.Fatalf("quick reply fixture has empty thread_key: %v", original)
		}

		// Same Message-ID shape the frontend mints (newDraftMessageId): the
		// store-side thread cleanup only targets meron-draft-*@meron ids.
		draftID := fmt.Sprintf("meron-draft-%s-itest@meron", nonce)
		draftSubject := "Re: " + quickSubject
		if _, err := sidecar.Call("save_draft", map[string]any{
			"account":     "bob",
			"to":          "alice@maddy.test",
			"subject":     draftSubject,
			"body":        "quick reply draft body",
			"in_reply_to": quickMessageID,
			"references":  quickMessageID,
			"draft_id":    draftID,
		}); err != nil {
			t.Fatalf("save_draft: %v", err)
		}

		draft := pollFolder(t, sidecar, "bob", "Drafts", func(m map[string]any) bool {
			return str(m, "subject") == draftSubject
		})
		if got := str(draft, "thread_key"); got != threadKey {
			t.Fatalf("reply draft thread_key = %q, want %q (draft would not hydrate into its thread)", got, threadKey)
		}
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": threadKey,
		})
		if !threadContainsSubject(thread, draftSubject) {
			t.Fatalf("messages.thread does not include the reply draft: %v", thread)
		}

		if _, err := sidecar.Call("discard_draft", map[string]any{
			"account":    "bob",
			"draft_id":   draftID,
			"thread_key": threadKey,
		}); err != nil {
			t.Fatalf("discard_draft: %v", err)
		}
		assertNoMessageInFolder(t, sidecar, "bob", "Drafts", func(m map[string]any) bool {
			return str(m, "subject") == draftSubject
		})
		thread = callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": threadKey,
		})
		if threadContainsSubject(thread, draftSubject) {
			t.Fatalf("discarded reply draft still in messages.thread: %v", thread)
		}
		if !threadContainsSubject(thread, quickSubject) {
			t.Fatalf("original message missing from thread after draft discard: %v", thread)
		}
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

	t.Run("empty trash", func(t *testing.T) {
		// Emptying is folder-wide and permanent — it clears the server folder
		// itself, not just the UIDs the client cached — and role-gated, so a
		// request for INBOX must be refused before anything is touched.
		emptySubject := "Meron integration empty " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    emptySubject,
			"body":       "empty me",
			"message_id": fmt.Sprintf("itest-empty-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send empty fixture: %v", err)
		}
		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == emptySubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("empty fixture has no uid: %v", message)
		}
		callMap(t, sidecar, "messages.delete", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
		})
		pollFolder(t, sidecar, "bob", "Trash", func(m map[string]any) bool {
			return str(m, "subject") == emptySubject
		})

		if _, err := sidecar.Call("messages.emptyFolder", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
		}); err == nil {
			t.Fatal("messages.emptyFolder accepted INBOX, want refusal")
		}

		callMap(t, sidecar, "messages.emptyFolder", map[string]any{
			"account": "bob",
			"folder":  "Trash",
		})
		assertNoMessageInFolder(t, sidecar, "bob", "Trash", func(map[string]any) bool { return true })
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

	t.Run("grouped thread cards", func(t *testing.T) {
		// group:true opts into core-side thread grouping: the response carries
		// ready thread cards (thread_id, unread counts) instead of raw message
		// rows.
		res := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"refresh": false,
			"limit":   50,
			"group":   true,
		})
		threads, _ := res["threads"].([]any)
		if len(threads) == 0 {
			t.Fatalf("grouped messages.recent returned no threads: %v", res)
		}
		card, ok := threads[0].(map[string]any)
		if !ok || str(card, "thread_id") == "" {
			t.Fatalf("thread card missing thread_id: %v", threads[0])
		}
		if _, hasMessages := res["messages"]; hasMessages {
			t.Fatalf("grouped response should not also carry raw messages: %v", res)
		}
	})

	t.Run("external expunge prunes the local row", func(t *testing.T) {
		// A message another client deletes for good must not linger as a ghost
		// row: the sync compares the server's UID set against the cache and
		// drops what is no longer there.
		expungeSubject := "Meron integration expunge " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    expungeSubject,
			"body":       "delete me from another client",
			"message_id": fmt.Sprintf("itest-expunge-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send expunge fixture: %v", err)
		}
		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == expungeSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("expunge fixture has no uid: %v", message)
		}

		imapExpunge(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", uid)
		assertNoMessageInFolder(t, sidecar, "bob", "INBOX", func(m map[string]any) bool {
			return str(m, "subject") == expungeSubject
		})
	})

	t.Run("external flag changes reconcile locally", func(t *testing.T) {
		server := startMaddy(t)
		sidecar, _ := startSidecar(t)
		connectAccount(t, sidecar, server, "alice", "alice@maddy.test")
		connectAccount(t, sidecar, server, "bob", "bob@maddy.test")
		nonce := fmt.Sprintf("%d", time.Now().UnixNano())

		// The reverse direction of the server-truth checks above: another client
		// can read or star a message, and the next sync must replace the cached
		// flags and unread count with what IMAP reports.
		flagSubject := "Meron integration external flags " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    flagSubject,
			"body":       "change my flags from another client",
			"message_id": fmt.Sprintf("itest-external-flags-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send external-flags fixture: %v", err)
		}
		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == flagSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("external-flags fixture has no uid: %v", message)
		}
		if boolValue(message, "seen") || boolValue(message, "starred") {
			t.Fatalf("external-flags fixture did not start unread and unstarred: %v", message)
		}
		page := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"refresh": false,
			"limit":   50,
		})
		unreadBefore, _ := page["folder_unread"].(float64)
		if unreadBefore != 1 {
			t.Fatalf("folder_unread before external flag change = %v, want 1: %v", unreadBefore, page)
		}

		imapSetFlags(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", uid, true, true)
		updated := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == flagSubject && boolValue(m, "seen") && boolValue(m, "starred")
		})
		if num(updated, "uid") != uid {
			t.Fatalf("externally flagged message uid = %d, want %d", num(updated, "uid"), uid)
		}
		page = callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"refresh": false,
			"limit":   50,
		})
		if unread, _ := page["folder_unread"].(float64); unread != 0 {
			t.Fatalf("folder_unread after external \\Seen = %v, want 0: %v", unread, page)
		}

		imapSetFlags(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", uid, false, false)
		updated = pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == flagSubject && !boolValue(m, "seen") && !boolValue(m, "starred")
		})
		if num(updated, "uid") != uid {
			t.Fatalf("externally unflagged message uid = %d, want %d", num(updated, "uid"), uid)
		}
		page = callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"refresh": false,
			"limit":   50,
		})
		if unread, _ := page["folder_unread"].(float64); unread != 1 {
			t.Fatalf("folder_unread after removing external \\Seen = %v, want 1: %v", unread, page)
		}
	})

	t.Run("uidvalidity change clears the cached folder", func(t *testing.T) {
		// A recreated mailbox hands out a new UIDVALIDITY and starts UIDs over
		// from 1, so cached rows describe messages that no longer exist under
		// those UIDs. The sync must drop the folder's rows wholesale instead of
		// merging two generations — the UID-set prune is deliberately skipped
		// when validity changed, so nothing else would clean them up.
		folder := "ITestUidv"
		if result := callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": folder}); !foldersContain(result, folder) {
			t.Fatalf("folders.create did not return %s: %v", folder, result)
		}
		firstSubject := "Meron integration uidv first " + nonce
		secondSubject := "Meron integration uidv second " + nonce
		for i, subj := range []string{firstSubject, secondSubject} {
			imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, folder, rawMessage([]string{
				"From: Carol <carol@example.net>",
				"To: bob@maddy.test",
				"Subject: " + subj,
				fmt.Sprintf("Message-ID: <itest-uidv-%d-%s@maddy.test>", i, nonce),
				"Date: " + time.Now().Format(time.RFC1123Z),
			}, "before the mailbox was recreated"))
		}
		for _, subj := range []string{firstSubject, secondSubject} {
			pollFolder(t, sidecar, "bob", folder, func(m map[string]any) bool {
				return str(m, "subject") == subj
			})
		}
		before := imapUIDValidity(t, server.imapPort, "bob@maddy.test", testPassword, folder)

		imapRecreateFolder(t, server.imapPort, "bob@maddy.test", testPassword, folder)
		if after := imapUIDValidity(t, server.imapPort, "bob@maddy.test", testPassword, folder); after == before {
			t.Skipf("maddy reused UIDVALIDITY %d for the recreated folder; nothing to reconcile", after)
		}

		freshSubject := "Meron integration uidv fresh " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, folder, rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + freshSubject,
			fmt.Sprintf("Message-ID: <itest-uidv-fresh-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "after the mailbox was recreated"))

		pollFolder(t, sidecar, "bob", folder, func(m map[string]any) bool {
			return str(m, "subject") == freshSubject
		})
		// The new message reuses UID 1, so an upsert alone would hide the first
		// stale row. The second one can only be gone if the folder was cleared.
		for _, subj := range []string{firstSubject, secondSubject} {
			assertNoMessageInFolder(t, sidecar, "bob", folder, func(m map[string]any) bool {
				return str(m, "subject") == subj
			})
		}
	})

	t.Run("idle pushes folder updates while watched", func(t *testing.T) {
		// IMAP IDLE is per selected mailbox, so kanban starts a watch per visible
		// non-INBOX column. A message another client appends has to reach the
		// store from the push alone — no refresh call — and stop reaching it once
		// the watch is dropped.
		folder := "ITestIdle"
		if result := callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": folder}); !foldersContain(result, folder) {
			t.Fatalf("folders.create did not return %s: %v", folder, result)
		}
		synced := func() int {
			return events.count(func(event sidecarEvent) bool {
				return event.name == "mail.synced" && str(event.detail, "account") == "bob" &&
					strings.EqualFold(str(event.detail, "folder"), folder)
			})
		}
		callMap(t, sidecar, "watch.start", map[string]any{"account": "bob", "folder": folder})

		baseline := synced()
		pushedSubject := "Meron integration idle pushed " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, folder, rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + pushedSubject,
			fmt.Sprintf("Message-ID: <itest-idle-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "pushed over IDLE"))

		deadline := time.Now().Add(60 * time.Second)
		for synced() == baseline {
			if time.Now().After(deadline) {
				t.Fatalf("IDLE never pushed a mail.synced for %s", folder)
			}
			time.Sleep(200 * time.Millisecond)
		}
		// refresh:false serves the store cache only, so a hit proves the push
		// itself wrote the row.
		cached := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  folder,
			"refresh": false,
			"limit":   50,
		})
		if !messagesContainSubject(cached, pushedSubject) {
			t.Fatalf("IDLE push did not land %q in the store: %v", pushedSubject, cached)
		}

		stopped := func() int {
			return events.count(func(event sidecarEvent) bool {
				return event.name == "watch.stopped" && str(event.detail, "account") == "bob" &&
					strings.EqualFold(str(event.detail, "folder"), folder)
			})
		}
		stoppedBaseline := stopped()
		if result := callMap(t, sidecar, "watch.stop", map[string]any{"account": "bob", "folder": folder}); !boolValue(result, "stopped") {
			t.Fatalf("watch.stop did not stop the watcher: %v", result)
		}
		deadline = time.Now().Add(10 * time.Second)
		for stopped() == stoppedBaseline {
			if time.Now().After(deadline) {
				t.Fatalf("watch.stop never completed for %s", folder)
			}
			time.Sleep(100 * time.Millisecond)
		}
		baseline = synced()
		quietSubject := "Meron integration idle quiet " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, folder, rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + quietSubject,
			fmt.Sprintf("Message-ID: <itest-idle-quiet-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "appended after the watch stopped"))
		if synced() != baseline {
			t.Fatalf("a stopped watch kept syncing %s", folder)
		}
		cached = callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  folder,
			"refresh": false,
			"limit":   50,
		})
		if messagesContainSubject(cached, quietSubject) {
			t.Fatalf("a stopped watch still cached %q: %v", quietSubject, cached)
		}

		// Starting the watch again catches up the message that arrived while it
		// was stopped, proving the negative assertion above was not merely an
		// append that had not reached the server yet.
		callMap(t, sidecar, "watch.start", map[string]any{"account": "bob", "folder": folder})
		deadline = time.Now().Add(30 * time.Second)
		for synced() == baseline {
			if time.Now().After(deadline) {
				t.Fatalf("restarted watch never caught up %q", quietSubject)
			}
			time.Sleep(100 * time.Millisecond)
		}
		cached = callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  folder,
			"refresh": false,
			"limit":   50,
		})
		if !messagesContainSubject(cached, quietSubject) {
			t.Fatalf("restarted watch did not cache %q: %v", quietSubject, cached)
		}
		// Best-effort teardown only. The stop behavior was asserted above; once
		// the restarted watcher has caught up, waiting for a second lifecycle
		// event adds no coverage and can race the watcher entering IDLE.
		callMap(t, sidecar, "watch.stop", map[string]any{"account": "bob", "folder": folder})
	})

	t.Run("unified inbox merges and writes across accounts", func(t *testing.T) {
		// The unified view fans messages.recent out over every included account
		// and merges the pages by date; markAllReadUnified fans the write out the
		// same way. Both must cover *both* accounts and report no per-account
		// failure — a partial merge looks like a working inbox with mail missing.
		unifiedSubject := "Meron integration unified " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "bob",
			"to":         "alice@maddy.test",
			"subject":    unifiedSubject,
			"body":       "so alice has inbox mail too",
			"message_id": fmt.Sprintf("itest-unified-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send unified fixture: %v", err)
		}
		pollInbox(t, sidecar, "alice", func(m map[string]any) bool {
			return str(m, "subject") == unifiedSubject
		})
		pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == subject
		})

		// refresh:false: both stores were just primed, and the merge is the thing
		// under test, not another round of IMAP sync.
		merged := callMap(t, sidecar, "messages.unifiedRecent", map[string]any{
			"limit":   50,
			"refresh": false,
		})
		if failures, _ := merged["failures"].([]any); len(failures) > 0 {
			t.Fatalf("unifiedRecent reported account failures: %v", failures)
		}
		threads, _ := merged["threads"].([]any)
		if len(threads) == 0 {
			t.Fatalf("unifiedRecent returned no threads: %v", merged)
		}
		seen := map[string]bool{}
		lastDate := int64(-1)
		for _, item := range threads {
			card, ok := item.(map[string]any)
			if !ok {
				t.Fatalf("unified thread has type %T", item)
			}
			seen[str(card, "account_id")] = true
			date, _ := card["date"].(float64)
			if lastDate >= 0 && int64(date) > lastDate {
				t.Fatalf("unified page is not sorted newest first: %v", threads)
			}
			lastDate = int64(date)
		}
		for _, account := range []string{"alice", "bob"} {
			if !seen[account] {
				t.Fatalf("unified page has no cards from %s: %v", account, seen)
			}
		}

		result := callMap(t, sidecar, "messages.markAllReadUnified", map[string]any{"folder": "INBOX"})
		if failures, _ := result["failures"].([]any); len(failures) > 0 {
			t.Fatalf("markAllReadUnified reported account failures: %v", failures)
		}
		deadline := time.Now().Add(30 * time.Second)
		for {
			merged = callMap(t, sidecar, "messages.unifiedRecent", map[string]any{
				"limit":   50,
				"refresh": false,
			})
			unread, _ := merged["folder_unread"].(float64)
			if unread == 0 {
				break
			}
			if time.Now().After(deadline) {
				t.Fatalf("unified folder_unread stayed at %v after markAllReadUnified: %v", unread, merged)
			}
			time.Sleep(500 * time.Millisecond)
		}
	})

	t.Run("unified mark-all-read reports a partial failure", func(t *testing.T) {
		server := startMaddy(t)
		sidecar, _ := startSidecar(t)
		connectAccount(t, sidecar, server, "alice", "alice@maddy.test")
		connectAccount(t, sidecar, server, "bob", "bob@maddy.test")
		nonce := fmt.Sprintf("%d", time.Now().UnixNano())

		// Give both healthy accounts a known unread row, and give a third account
		// a cached row before replacing its credentials with a bad password.
		// The fan-out must continue past that failure and identify it precisely.
		aliceSubject := "Meron integration unified partial alice " + nonce
		bobSubject := "Meron integration unified partial bob " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "bob",
			"to":         "alice@maddy.test",
			"subject":    aliceSubject,
			"body":       "healthy alice unread",
			"message_id": fmt.Sprintf("itest-unified-partial-alice-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send alice partial-failure fixture: %v", err)
		}
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    bobSubject,
			"body":       "healthy bob unread",
			"message_id": fmt.Sprintf("itest-unified-partial-bob-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send bob partial-failure fixture: %v", err)
		}
		aliceMessage := pollInbox(t, sidecar, "alice", func(m map[string]any) bool {
			return str(m, "subject") == aliceSubject
		})
		bobMessage := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == bobSubject
		})

		broken := "itest-unified-broken"
		connectAccount(t, sidecar, server, broken, "bob@maddy.test")
		t.Cleanup(func() {
			if _, err := sidecar.Call("account.remove", map[string]any{"account": broken}); err != nil {
				t.Logf("remove partial-failure account: %v", err)
			}
		})
		pollInbox(t, sidecar, broken, func(m map[string]any) bool {
			return str(m, "subject") == bobSubject
		})
		callMap(t, sidecar, "account.connect", map[string]any{
			"account":   broken,
			"email":     "bob@maddy.test",
			"host":      "127.0.0.1",
			"port":      server.imapPort,
			"tls":       false,
			"smtp_host": "127.0.0.1",
			"smtp_port": server.smtpPort,
			"smtp_tls":  false,
			"user":      "bob@maddy.test",
			"password":  "definitely-not-" + testPassword,
			"validate":  false,
		})
		// Drop any authenticated connection retained from the initial sync.
		callMap(t, sidecar, "account.setPaused", map[string]any{"account": broken, "enabled": true})

		result := callMap(t, sidecar, "messages.markAllReadUnified", map[string]any{"folder": "INBOX"})
		if boolValue(result, "ok") {
			t.Fatalf("markAllReadUnified reported success despite a broken account: %v", result)
		}
		failures, _ := result["failures"].([]any)
		if len(failures) != 1 {
			t.Fatalf("markAllReadUnified failures len = %d, want 1: %v", len(failures), result)
		}
		failure, ok := failures[0].(map[string]any)
		if !ok || str(failure, "account_id") != broken {
			t.Fatalf("markAllReadUnified failure = %v, want account %s", failures[0], broken)
		}
		folderUnreads, _ := result["folder_unreads"].(map[string]any)
		for _, account := range []string{"alice", "bob"} {
			if _, ok := folderUnreads[account]; !ok {
				t.Fatalf("healthy account %s missing from markAllReadUnified result: %v", account, result)
			}
		}
		for _, fixture := range []struct {
			user string
			uid  uint32
		}{
			{"alice@maddy.test", num(aliceMessage, "uid")},
			{"bob@maddy.test", num(bobMessage, "uid")},
		} {
			if flags := imapFlags(t, server.imapPort, fixture.user, testPassword, "INBOX", fixture.uid); !strings.Contains(flags, `\Seen`) {
				t.Fatalf("healthy account %s uid %d was not marked read on the server: %q", fixture.user, fixture.uid, flags)
			}
		}
	})

	t.Run("unified cursors advance each account independently", func(t *testing.T) {
		server := startMaddy(t)
		sidecar, _ := startSidecar(t)
		connectAccount(t, sidecar, server, "alice", "alice@maddy.test")
		connectAccount(t, sidecar, server, "bob", "bob@maddy.test")
		nonce := fmt.Sprintf("%d", time.Now().UnixNano())

		aliceSubject := "Meron integration unified cursor alice " + nonce
		bobSubject := "Meron integration unified cursor bob " + nonce
		fixtures := []struct {
			account, to, subject, id string
		}{{"bob", "alice@maddy.test", aliceSubject, "alice"}}
		for i := 0; i < 5; i++ {
			fixtures = append(fixtures, struct {
				account, to, subject, id string
			}{
				"alice",
				"bob@maddy.test",
				fmt.Sprintf("%s %d", bobSubject, i),
				fmt.Sprintf("bob-%d", i),
			})
		}
		for _, fixture := range fixtures {
			if _, err := sidecar.Call("send", map[string]any{
				"account":    fixture.account,
				"to":         fixture.to,
				"subject":    fixture.subject,
				"body":       "unified cursor fixture",
				"message_id": fmt.Sprintf("itest-unified-cursor-%s-%s@maddy.test", fixture.id, nonce),
			}); err != nil {
				t.Fatalf("send unified cursor fixture for %s: %v", fixture.id, err)
			}
		}
		pollInbox(t, sidecar, "alice", func(m map[string]any) bool {
			return str(m, "subject") == aliceSubject
		})
		pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == bobSubject+" 4"
		})

		seenIDs := map[string]bool{}
		seenSubjects := map[string]bool{}
		cursor := ""
		sawBobOnlyContinuation := false
		for pageNumber := 0; ; pageNumber++ {
			if pageNumber > 50 {
				t.Fatal("unified cursor did not terminate within 50 pages")
			}
			params := map[string]any{
				"limit":   3,
				"refresh": false,
			}
			if cursor != "" {
				params["before_cursor"] = cursor
			}
			page := callMap(t, sidecar, "messages.unifiedRecent", params)
			if failures, _ := page["failures"].([]any); len(failures) != 0 {
				t.Fatalf("unified cursor page reported failures: %v", page)
			}
			threads, _ := page["threads"].([]any)
			accountsOnPage := map[string]bool{}
			lastDate := int64(-1)
			for _, item := range threads {
				card, ok := item.(map[string]any)
				if !ok {
					t.Fatalf("unified cursor card has type %T", item)
				}
				id := str(card, "id")
				if id == "" {
					t.Fatalf("unified cursor card has no id: %v", card)
				}
				if seenIDs[id] {
					t.Fatalf("unified cursor repeated card %q: %v", id, card)
				}
				seenIDs[id] = true
				accountsOnPage[str(card, "account_id")] = true
				seenSubjects[str(card, "subject")] = true
				date, _ := card["date"].(float64)
				if lastDate >= 0 && int64(date) > lastDate {
					t.Fatalf("unified cursor page is not sorted newest first: %v", threads)
				}
				lastDate = int64(date)
			}
			if pageNumber > 0 && accountsOnPage["bob"] && !accountsOnPage["alice"] {
				sawBobOnlyContinuation = true
			}
			next, _ := page["next_cursor"].(string)
			if next == "" {
				break
			}
			if next == cursor {
				t.Fatalf("unified cursor did not advance: %q", next)
			}
			cursor = next
		}
		for _, subject := range []string{aliceSubject, bobSubject + " 4"} {
			if !seenSubjects[subject] {
				t.Fatalf("unified cursor traversal missed %q", subject)
			}
		}
		if !sawBobOnlyContinuation {
			t.Fatal("unified cursor never continued bob after alice was exhausted")
		}
	})

	t.Run("encoded subject and quoted-printable body decode", func(t *testing.T) {
		// Real mail arrives encoded — an RFC 2047 subject and a quoted-printable
		// body — and meron's own send path never emits either, so only a raw
		// append exercises the decode on the way in.
		want := "Méron héllo " + nonce
		encoded := "=?UTF-8?B?" + base64.StdEncoding.EncodeToString([]byte(want)) + "?="
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + encoded,
			fmt.Sprintf("Message-ID: <itest-encoded-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
			"MIME-Version: 1.0",
			"Content-Type: text/plain; charset=utf-8",
			"Content-Transfer-Encoding: quoted-printable",
		}, "caf=C3=A9 pi=C3=B1a "+nonce))

		row := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == want
		})
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": str(row, "thread_key"),
		})
		message := firstThreadMessage(t, thread)
		if body := str(message, "body"); !strings.Contains(body, "café piña") {
			t.Fatalf("quoted-printable body = %q, want it to contain %q", body, "café piña")
		}
	})

	t.Run("multipart alternative keeps both text and html", func(t *testing.T) {
		// The thread read serves text and HTML separately (the HTML view is a
		// per-account preference), so a multipart/alternative message must land
		// with both halves, not whichever part the walk saw last.
		altSubject := "Meron integration alternative " + nonce
		boundary := "itest-boundary-" + nonce
		body := strings.Join([]string{
			"--" + boundary,
			"Content-Type: text/plain; charset=utf-8",
			"",
			"plain part " + nonce,
			"--" + boundary,
			"Content-Type: text/html; charset=utf-8",
			"",
			"<p>html part " + nonce + "</p>",
			"--" + boundary + "--",
		}, "\n")
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + altSubject,
			fmt.Sprintf("Message-ID: <itest-alt-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
			"MIME-Version: 1.0",
			`Content-Type: multipart/alternative; boundary="` + boundary + `"`,
		}, body))

		row := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == altSubject
		})
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": str(row, "thread_key"),
		})
		message := firstThreadMessage(t, thread)
		if text := str(message, "body"); !strings.Contains(text, "plain part "+nonce) {
			t.Fatalf("body = %q, want the text/plain part", text)
		}
		if html := str(message, "body_html"); !strings.Contains(html, "html part "+nonce) {
			t.Fatalf("body_html = %q, want the text/html part", html)
		}
	})

	t.Run("nested multipart keeps inline and downloadable attachments", func(t *testing.T) {
		server := startMaddy(t)
		sidecar, _ := startSidecar(t)
		connectAccount(t, sidecar, server, "bob", "bob@maddy.test")
		nonce := fmt.Sprintf("%d", time.Now().UnixNano())

		nestedSubject := "Meron integration nested MIME " + nonce
		outer := "itest-outer-" + nonce
		related := "itest-related-" + nonce
		alternative := "itest-nested-alt-" + nonce
		contentID := "itest-logo-" + nonce
		attachmentBytes := []byte("nested attachment " + nonce)
		body := strings.Join([]string{
			"--" + outer,
			`Content-Type: multipart/related; boundary="` + related + `"`,
			"",
			"--" + related,
			`Content-Type: multipart/alternative; boundary="` + alternative + `"`,
			"",
			"--" + alternative,
			"Content-Type: text/plain; charset=utf-8",
			"",
			"nested plain " + nonce,
			"--" + alternative,
			"Content-Type: text/html; charset=utf-8",
			"",
			`<p>nested html <img src="cid:` + contentID + `"></p>`,
			"--" + alternative + "--",
			"--" + related,
			"Content-Type: image/png",
			"Content-Transfer-Encoding: base64",
			"Content-ID: <" + contentID + ">",
			`Content-Disposition: inline; filename="logo.png"`,
			"",
			"AQIDBA==",
			"--" + related + "--",
			"--" + outer,
			"Content-Type: application/octet-stream",
			"Content-Transfer-Encoding: base64",
			`Content-Disposition: attachment; filename="=?UTF-8?B?Y2Fmw6kudHh0?="`,
			"",
			base64.StdEncoding.EncodeToString(attachmentBytes),
			"--" + outer + "--",
		}, "\n")
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + nestedSubject,
			fmt.Sprintf("Message-ID: <itest-nested-mime-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
			"MIME-Version: 1.0",
			`Content-Type: multipart/mixed; boundary="` + outer + `"`,
		}, body))

		row := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == nestedSubject
		})
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": str(row, "thread_key"),
		})
		message := firstThreadMessage(t, thread)
		if text := str(message, "body"); !strings.Contains(text, "nested plain "+nonce) {
			t.Fatalf("nested MIME body = %q, want text/plain part", text)
		}
		html := str(message, "body_html")
		if !strings.Contains(html, "nested html") || strings.Contains(html, "cid:"+contentID) || !strings.Contains(html, "/media/") {
			t.Fatalf("nested MIME HTML did not rewrite inline CID: %q", html)
		}
		attachments := attachmentRows(t, message)
		if len(attachments) != 2 {
			t.Fatalf("nested MIME attachments len = %d, want 2: %v", len(attachments), message)
		}
		byName := map[string]map[string]any{}
		for _, attachment := range attachments {
			byName[str(attachment, "filename")] = attachment
		}
		for _, expected := range []struct {
			name string
			data []byte
		}{
			{"logo.png", []byte{1, 2, 3, 4}},
			{"café.txt", attachmentBytes},
		} {
			attachment, ok := byName[expected.name]
			if !ok {
				t.Fatalf("nested MIME attachment %q missing: %v", expected.name, attachments)
			}
			key := str(attachment, "key")
			got, err := os.ReadFile(filepath.Join(mediaDir(), key))
			if err != nil {
				t.Fatalf("read nested MIME attachment %q: %v", expected.name, err)
			}
			if string(got) != string(expected.data) {
				t.Fatalf("nested MIME attachment %q bytes = %q, want %q", expected.name, got, expected.data)
			}
		}
	})

	t.Run("message without a message-id still threads", func(t *testing.T) {
		// Threading keys off Message-ID, but mail without one exists and must
		// still be readable: it needs a synthesized thread_key, or the row is
		// unreachable from the thread view even though the list shows it.
		bareSubject := "Meron integration no message id " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + bareSubject,
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "no Message-ID header at all"))

		row := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == bareSubject
		})
		threadKey := str(row, "thread_key")
		if threadKey == "" {
			t.Fatalf("message without a Message-ID got no thread_key: %v", row)
		}
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": threadKey,
		})
		if !threadContainsSubject(thread, bareSubject) {
			t.Fatalf("messages.thread cannot reach the id-less message: %v", thread)
		}
	})

	t.Run("empty junk", func(t *testing.T) {
		// emptyFolder is role-gated to Trash *and* Junk (the Trash half and the
		// INBOX refusal are covered above), and the role has to resolve for a
		// folder the account never had special-use metadata for.
		// maddy pre-creates Junk; folders.list serves the cache and refreshes in
		// the background, so poll until the role-bearing row is there to empty.
		deadline := time.Now().Add(30 * time.Second)
		for {
			result := callMap(t, sidecar, "folders.list", map[string]any{"account": "bob"})
			if foldersContain(result, "Junk") {
				break
			}
			if time.Now().After(deadline) {
				t.Fatalf("Junk never appeared in folders.list: %v", result)
			}
			time.Sleep(300 * time.Millisecond)
		}
		junkSubject := "Meron integration junk " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "Junk", rawMessage([]string{
			"From: Spammer <spam@example.net>",
			"To: bob@maddy.test",
			"Subject: " + junkSubject,
			fmt.Sprintf("Message-ID: <itest-junk-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "junk body"))
		pollFolder(t, sidecar, "bob", "Junk", func(m map[string]any) bool {
			return str(m, "subject") == junkSubject
		})

		callMap(t, sidecar, "messages.emptyFolder", map[string]any{
			"account": "bob",
			"folder":  "Junk",
		})
		assertNoMessageInFolder(t, sidecar, "bob", "Junk", func(map[string]any) bool { return true })
		if uids := imapSearchSubject(t, server.imapPort, "bob@maddy.test", testPassword, "Junk", junkSubject); len(uids) != 0 {
			t.Fatalf("emptying Junk left %v on the server", uids)
		}
	})

	t.Run("clearing the attachment cache refetches the bytes", func(t *testing.T) {
		// The cache is cleared behind the store's back (a plain rmdir from the
		// bridge), so cached rows keep pointing at files that are gone. A thread
		// read has to notice the missing file and refetch, not serve an
		// attachment whose bytes no longer exist.
		app := &App{}
		usage, err := app.storageUsage()
		if err != nil {
			t.Fatalf("storageUsage: %v", err)
		}
		if bytes, _ := usage.(map[string]any)["cacheBytes"].(int64); bytes == 0 {
			t.Fatalf("no cached attachment bytes to clear: %v", usage)
		}

		cleared, err := app.storageClearCache()
		if err != nil {
			t.Fatalf("storageClearCache: %v", err)
		}
		if bytes, _ := cleared.(map[string]any)["cacheBytes"].(int64); bytes != 0 {
			t.Fatalf("cacheBytes after clear = %d, want 0", bytes)
		}
		if db, _ := cleared.(map[string]any)["dbBytes"].(int64); db == 0 {
			t.Fatalf("clearing the cache emptied the database too: %v", cleared)
		}

		attachmentSubject := "Meron integration attachment " + nonce
		row := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == attachmentSubject
		})
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "INBOX",
			"thread_key": str(row, "thread_key"),
		})
		message := firstThreadMessage(t, thread)
		attachments := attachmentRows(t, message)
		if len(attachments) != 1 {
			t.Fatalf("attachments len = %d, want 1: %v", len(attachments), message)
		}
		key := str(attachments[0], "key")
		if key == "" {
			t.Fatalf("refetched attachment has no key: %v", attachments[0])
		}
		got, err := os.ReadFile(filepath.Join(mediaDir(), key))
		if err != nil {
			t.Fatalf("attachment was not refetched after the cache was cleared: %v", err)
		}
		if !strings.Contains(string(got), "hello attachment "+nonce) {
			t.Fatalf("refetched attachment bytes = %q", got)
		}
	})

	t.Run("cold sync from an empty profile sees server state", func(t *testing.T) {
		// Everything above reads back through the same store that performed the
		// write, so a write that never left the client would still pass. A second
		// sidecar on a fresh profile has no cached rows at all: whatever it
		// reports came from maddy.
		cold, _ := startSidecar(t)
		connectAccount(t, cold, server, "bob", "bob@maddy.test")

		moveSubject := "Meron integration move " + nonce
		row := pollFolder(t, cold, "bob", "ITestFolder", func(m map[string]any) bool {
			return str(m, "subject") == moveSubject
		})
		if !boolValue(row, "seen") || !boolValue(row, "starred") {
			t.Fatalf("cold sync does not see the flags a warm store reported: %v", row)
		}
		// Bodies come from IMAP too — nothing local can stand in for them.
		thread := callMap(t, cold, "messages.thread", map[string]any{
			"account":    "bob",
			"folder":     "ITestFolder",
			"thread_key": str(row, "thread_key"),
		})
		message := firstThreadMessage(t, thread)
		if boolValue(message, "body_missing") {
			t.Fatalf("cold sync could not fetch the message body: %v", message)
		}
		if body := str(message, "body"); !strings.Contains(body, "move me to the integration folder") {
			t.Fatalf("cold-synced body = %q", body)
		}
	})

	t.Run("sent copy honors the account override", func(t *testing.T) {
		// A generic IMAP server does not file sent mail itself, so meron APPENDs
		// the copy — exactly once, since a duplicate upload would show the
		// message twice in Sent. The override exists for providers that do file
		// their own, and must suppress the upload entirely.
		defaultSubject := "Meron integration sentcopy default " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    defaultSubject,
			"body":       "should be filed in Sent by us",
			"message_id": fmt.Sprintf("itest-sentcopy-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send sentcopy fixture: %v", err)
		}
		pollFolder(t, sidecar, "alice", "Sent", func(m map[string]any) bool {
			return str(m, "subject") == defaultSubject
		})
		if uids := imapSearchSubject(t, server.imapPort, "alice@maddy.test", testPassword, "Sent", defaultSubject); len(uids) != 1 {
			t.Fatalf("Sent holds %d copies of the message, want 1: %v", len(uids), uids)
		}

		callMap(t, sidecar, "account.setSaveSentCopy", map[string]any{"account": "alice", "value": false})
		// Back to the provider default, so nothing after this inherits it.
		defer callMap(t, sidecar, "account.setSaveSentCopy", map[string]any{"account": "alice", "value": nil})

		suppressedSubject := "Meron integration sentcopy off " + nonce
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@maddy.test",
			"subject":    suppressedSubject,
			"body":       "should not be filed in Sent",
			"message_id": fmt.Sprintf("itest-sentcopy-off-%s@maddy.test", nonce),
		}); err != nil {
			t.Fatalf("send suppressed sentcopy fixture: %v", err)
		}
		// Delivery is what proves the send itself went through.
		pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == suppressedSubject
		})
		if uids := imapSearchSubject(t, server.imapPort, "alice@maddy.test", testPassword, "Sent", suppressedSubject); len(uids) != 0 {
			t.Fatalf("override off still uploaded a Sent copy: %v", uids)
		}
	})

	t.Run("connect rejects bad credentials", func(t *testing.T) {
		// Validation is what stands between a typo'd password and an account that
		// looks connected but can never sync, so the account must not be stored.
		id := "itest-badpass"
		_, err := sidecar.Call("account.connect", map[string]any{
			"account":   id,
			"email":     "alice@maddy.test",
			"host":      "127.0.0.1",
			"port":      server.imapPort,
			"tls":       false,
			"smtp_host": "127.0.0.1",
			"smtp_port": server.smtpPort,
			"smtp_tls":  false,
			"user":      "alice@maddy.test",
			"password":  "definitely-not-" + testPassword,
			"validate":  true,
		})
		if err == nil {
			t.Fatal("account.connect accepted a wrong password")
		}
		accounts, _ := callMap(t, sidecar, "account.list", map[string]any{})["accounts"].([]any)
		for _, item := range accounts {
			account, ok := item.(map[string]any)
			if ok && str(account, "id") == id {
				t.Fatalf("rejected account was stored anyway: %v", account)
			}
		}
	})

	t.Run("sync recovers after a server restart", func(t *testing.T) {
		server := startMaddy(t)
		sidecar, events := startSidecar(t)
		connectAccount(t, sidecar, server, "alice", "alice@maddy.test")
		connectAccount(t, sidecar, server, "bob", "bob@maddy.test")
		nonce := fmt.Sprintf("%d", time.Now().UnixNano())

		// A server that goes away must not need a client restart: reads retry a
		// stale pooled connection, and an active IDLE watcher must reconnect and
		// catch up without a client restart or an explicit refresh.
		folder := "ITestIdleRestart"
		if result := callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": folder}); !foldersContain(result, folder) {
			t.Fatalf("folders.create did not return %s: %v", folder, result)
		}
		synced := func() int {
			return events.count(func(event sidecarEvent) bool {
				return event.name == "mail.synced" && str(event.detail, "account") == "bob" &&
					strings.EqualFold(str(event.detail, "folder"), folder)
			})
		}
		callMap(t, sidecar, "watch.start", map[string]any{"account": "bob", "folder": folder})
		deadline := time.Now().Add(30 * time.Second)
		for synced() == 0 {
			if time.Now().After(deadline) {
				t.Fatalf("IDLE watch for %s never completed its initial catch-up", folder)
			}
			time.Sleep(200 * time.Millisecond)
		}
		t.Cleanup(func() {
			_, _ = sidecar.Call("watch.stop", map[string]any{"account": "bob", "folder": folder})
		})
		baseline := synced()

		restartMaddy(t, server)

		idleRestartSubject := "Meron integration idle restart " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, folder, rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + idleRestartSubject,
			fmt.Sprintf("Message-ID: <itest-idle-restart-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "caught up after the IDLE connection was severed"))
		deadline = time.Now().Add(60 * time.Second)
		for synced() == baseline {
			if time.Now().After(deadline) {
				t.Fatalf("IDLE watch for %s never recovered after restart", folder)
			}
			time.Sleep(200 * time.Millisecond)
		}
		cached := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "bob",
			"folder":  folder,
			"refresh": false,
			"limit":   50,
		})
		if !messagesContainSubject(cached, idleRestartSubject) {
			t.Fatalf("reconnected IDLE watch did not cache %q: %v", idleRestartSubject, cached)
		}

		restartSubject := "Meron integration restart " + nonce
		// Appended rather than sent: SMTP submission is a write path, and writes
		// deliberately do not retry a pooled connection.
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + restartSubject,
			fmt.Sprintf("Message-ID: <itest-restart-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "delivered after the restart"))

		// The refresh itself can fail while the reconnect is in flight, so retry
		// the call, not just the assertion.
		deadline = time.Now().Add(90 * time.Second)
		for {
			result, err := sidecar.Call("messages.recent", map[string]any{
				"account": "bob",
				"folder":  "INBOX",
				"refresh": true,
				"limit":   50,
			})
			if err == nil {
				if page, ok := result.(map[string]any); ok && messagesContainSubject(page, restartSubject) {
					break
				}
			}
			if time.Now().After(deadline) {
				t.Fatalf("sync never recovered after the server restart (last error: %v)", err)
			}
			time.Sleep(1 * time.Second)
		}

		// Reads recovered on their own; write sessions do not retry, so drop the
		// pooled sockets both accounts may still hold from before the restart
		// rather than leaving them for later subtests.
		for _, account := range []string{"alice", "bob"} {
			callMap(t, sidecar, "account.setPaused", map[string]any{"account": account, "enabled": true})
			callMap(t, sidecar, "account.setPaused", map[string]any{"account": account, "enabled": false})
		}
	})

	t.Run("flag writes recover from a stale pooled connection", func(t *testing.T) {
		// Regression test: a pooled session the server had silently dropped
		// failed messages.markRead outright ("SELECT: Broken pipe"), so the
		// flag never reached the server and the next sync pulled the message
		// back as unread. The SELECT now runs as a retryable preflight, ahead
		// of the STORE that must not be replayed.
		server := startMaddy(t)
		sidecar, _ := startSidecar(t)
		// The account talks to maddy through a proxy the test can cut, so the
		// pooled socket can be killed without taking the server down with it.
		proxy := startIMAPProxy(t, server.imapPort)
		proxied := *server
		proxied.imapPort = proxy.port()
		connectAccount(t, sidecar, &proxied, "bob", "bob@maddy.test")
		nonce := fmt.Sprintf("%d", time.Now().UnixNano())

		staleSubject := "Meron integration stale pool " + nonce
		imapAppend(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@maddy.test",
			"Subject: " + staleSubject,
			fmt.Sprintf("Message-ID: <itest-stale-pool-%s@maddy.test>", nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "flagged over a pooled connection the server dropped"))

		message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
			return str(m, "subject") == staleSubject
		})
		uid := num(message, "uid")
		if uid == 0 {
			t.Fatalf("delivered stale-pool fixture has no uid: %v", message)
		}

		// The sync above leaves a warm session in the pool; severing it is
		// invisible to the client until its next command, so a flag write can
		// start out holding a dead socket. Stop the IDLE watcher first, and let its connection go: a reconnecting
		// watcher would otherwise be the one to pick the dead session out of the
		// pool, leaving the flag write a healthy connection and no bug to catch.
		if _, err := sidecar.Call("watch.stop", map[string]any{"account": "bob", "folder": "INBOX"}); err != nil {
			t.Logf("watch.stop: %v", err)
		}
		time.Sleep(500 * time.Millisecond)

		// Both writes below start out holding a severed session: the first one
		// leaves a healthy connection behind, so sever again before the second.
		proxy.sever()
		callMap(t, sidecar, "messages.markRead", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"seen":    true,
		})
		proxy.sever()
		callMap(t, sidecar, "messages.markStarred", map[string]any{
			"account": "bob",
			"folder":  "INBOX",
			"uid":     uid,
			"starred": true,
		})

		// Server truth, not the local store: a STORE that never left the
		// client would look identical in messages.recent.
		if flags := imapFlags(t, server.imapPort, "bob@maddy.test", testPassword, "INBOX", uid); !strings.Contains(flags, `\Seen`) || !strings.Contains(flags, `\Flagged`) {
			t.Fatalf("server flags for INBOX uid %d = %q, want \\Seen and \\Flagged", uid, flags)
		}
	})

	// Last on purpose: no later subtest sends from alice, so nothing but the
	// piggyback under test can refresh her Sent folder.
	t.Run("sent from another client surfaces via inbox sync", func(t *testing.T) {
		// From is a send-as alias meron knows nothing about: the message must
		// still classify as outgoing purely from its Sent-folder provenance.
		externalSubject := "Meron integration external sent " + nonce
		raw := fmt.Sprintf(
			"From: Alice Alias <alice-alias@example.net>\r\nTo: bob@maddy.test\r\nSubject: %s\r\n"+
				"Message-ID: <itest-external-%s@maddy.test>\r\nDate: %s\r\n\r\n"+
				"sent from another client\r\n",
			externalSubject, nonce, time.Now().Format(time.RFC1123Z))
		imapAppend(t, server.imapPort, "alice@maddy.test", testPassword, "Sent", []byte(raw))

		// The copy must reach the local store without Sent ever being opened:
		// an INBOX refresh piggybacks a Sent envelope sync (messages.recent
		// with refresh=false serves the store cache only).
		var row map[string]any
		deadline := time.Now().Add(60 * time.Second)
		for row == nil {
			callMap(t, sidecar, "messages.recent", map[string]any{
				"account": "alice",
				"folder":  "INBOX",
				"refresh": true,
				"limit":   50,
			})
			cached := callMap(t, sidecar, "messages.recent", map[string]any{
				"account": "alice",
				"folder":  "Sent",
				"refresh": false,
				"limit":   50,
			})
			rows, _ := cached["messages"].([]any)
			for _, item := range rows {
				message, ok := item.(map[string]any)
				if ok && str(message, "subject") == externalSubject {
					row = message
					break
				}
			}
			if row == nil && time.Now().After(deadline) {
				t.Fatalf("externally appended Sent message never surfaced via inbox piggyback: %v", cached)
			}
			if row == nil {
				time.Sleep(500 * time.Millisecond)
			}
		}

		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account":    "alice",
			"folder":     "Sent",
			"thread_key": str(row, "thread_key"),
			"limit":      50,
		})
		entries, _ := thread["messages"].([]any)
		for _, item := range entries {
			message, ok := item.(map[string]any)
			if !ok {
				continue
			}
			if str(message, "subject") != externalSubject {
				continue
			}
			if !boolValue(message, "outgoing") {
				t.Fatalf("alias-sent Sent copy not classified outgoing: %v", message)
			}
			return
		}
		t.Fatalf("appended message missing from thread read: %v", thread)
	})
}

// imapClient is a bare-bones IMAP client standing in for another mail client
// (webmail, a phone) touching the mailbox behind meron's back — and for reading
// server truth directly. Assertions that only consult the sidecar cannot tell a
// write that reached the server from one that stopped at the local store.
type imapClient struct {
	t      *testing.T
	conn   net.Conn
	reader *bufio.Reader
	tag    int
}

func dialIMAP(t *testing.T, port int, user, password string) *imapClient {
	t.Helper()
	conn, err := net.DialTimeout("tcp", fmt.Sprintf("127.0.0.1:%d", port), 5*time.Second)
	if err != nil {
		t.Fatalf("dial imap: %v", err)
	}
	conn.SetDeadline(time.Now().Add(30 * time.Second))
	client := &imapClient{t: t, conn: conn, reader: bufio.NewReader(conn)}
	client.readUntil("* OK")
	client.do("LOGIN %q %q", user, password)
	return client
}

func (c *imapClient) close() {
	_, _ = io.WriteString(c.conn, "zz LOGOUT\r\n")
	_ = c.conn.Close()
}

func (c *imapClient) write(data []byte) {
	c.t.Helper()
	for len(data) > 0 {
		n, err := c.conn.Write(data)
		if err != nil {
			c.t.Fatalf("imap write: %v", err)
		}
		if n == 0 {
			c.t.Fatal("imap write made no progress")
		}
		data = data[n:]
	}
}

// readUntil reads past untagged/unsolicited lines until one starts with prefix.
func (c *imapClient) readUntil(prefix string) string {
	c.t.Helper()
	for {
		line, err := c.reader.ReadString('\n')
		if err != nil {
			c.t.Fatalf("imap read waiting for %q: %v", prefix, err)
		}
		if strings.HasPrefix(line, prefix) {
			return line
		}
	}
}

// do runs one command, fails the test unless the server answers OK, and returns
// the untagged lines the command produced.
func (c *imapClient) do(format string, args ...any) []string {
	c.t.Helper()
	c.tag++
	tag := fmt.Sprintf("t%d", c.tag)
	command := fmt.Sprintf(format, args...)
	c.write([]byte(fmt.Sprintf("%s %s\r\n", tag, command)))
	var untagged []string
	for {
		line, err := c.reader.ReadString('\n')
		if err != nil {
			c.t.Fatalf("imap %s: %v", command, err)
		}
		line = strings.TrimRight(line, "\r\n")
		if strings.HasPrefix(line, tag+" ") {
			if !imapTaggedOK(line, tag) {
				c.t.Fatalf("imap %s: %s", command, line)
			}
			return untagged
		}
		untagged = append(untagged, line)
	}
}

// selectFolder selects a mailbox and returns its UIDVALIDITY.
func (c *imapClient) selectFolder(folder string) uint32 {
	c.t.Helper()
	lines := c.do("SELECT %q", folder)
	validity, ok := parseIMAPUIDValidity(lines)
	if !ok {
		c.t.Fatalf("SELECT %q returned no valid UIDVALIDITY: %v", folder, lines)
	}
	return validity
}

// imapAppend adds a message to a folder over raw IMAP, standing in for another
// mail client (e.g. webmail) writing to the mailbox behind meron's back.
func imapAppend(t *testing.T, port int, user, password, folder string, message []byte) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.tag++
	tag := fmt.Sprintf("t%d", client.tag)
	client.write([]byte(fmt.Sprintf("%s APPEND %q {%d}\r\n", tag, folder, len(message))))
	client.readUntil("+")
	client.write(message)
	client.write([]byte("\r\n"))
	if line := client.readUntil(tag + " "); !imapTaggedOK(strings.TrimRight(line, "\r\n"), tag) {
		t.Fatalf("imap append failed: %s", line)
	}
}

// imapFlags reads a message's flags straight from the server, e.g.
// `\Seen \Flagged`. Returns "" when the UID is gone.
func imapFlags(t *testing.T, port int, user, password, folder string, uid uint32) string {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	return parseIMAPFlags(client.do("UID FETCH %d (FLAGS)", uid), uid)
}

func imapTaggedOK(line, tag string) bool {
	fields := strings.Fields(line)
	return len(fields) >= 2 && fields[0] == tag && strings.EqualFold(fields[1], "OK")
}

func parseIMAPUIDValidity(lines []string) (uint32, bool) {
	for _, line := range lines {
		upper := strings.ToUpper(line)
		marker := strings.Index(upper, "[UIDVALIDITY ")
		if marker < 0 {
			continue
		}
		value := line[marker+len("[UIDVALIDITY "):]
		if end := strings.IndexByte(value, ']'); end >= 0 {
			value = value[:end]
		}
		parsed, err := strconv.ParseUint(strings.TrimSpace(value), 10, 32)
		if err == nil && parsed > 0 {
			return uint32(parsed), true
		}
	}
	return 0, false
}

func parseIMAPFlags(lines []string, uid uint32) string {
	for _, line := range lines {
		upper := strings.ToUpper(line)
		marker := strings.Index(upper, "UID ")
		if marker < 0 {
			continue
		}
		var got uint32
		if _, err := fmt.Sscanf(upper[marker:], "UID %d", &got); err != nil || got != uid {
			continue
		}
		start := strings.Index(upper, "FLAGS (")
		if start < 0 {
			continue
		}
		rest := line[start+len("FLAGS ("):]
		if end := strings.Index(rest, ")"); end >= 0 {
			return rest[:end]
		}
	}
	return ""
}

// imapSetFlags changes the two user-visible flags as another mail client would.
func imapSetFlags(t *testing.T, port int, user, password, folder string, uid uint32, seen, starred bool) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	for _, change := range []struct {
		flag    string
		enabled bool
	}{
		{`\Seen`, seen},
		{`\Flagged`, starred},
	} {
		operation := "-FLAGS"
		if change.enabled {
			operation = "+FLAGS"
		}
		client.do("UID STORE %d %s (%s)", uid, operation, change.flag)
	}
}

// imapExpunge deletes a message the way another client would: flag it \Deleted
// and expunge it out of the mailbox.
func imapExpunge(t *testing.T, port int, user, password, folder string, uid uint32) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	client.do(`UID STORE %d +FLAGS (\Deleted)`, uid)
	client.do("EXPUNGE")
}

// imapUIDValidity reports a mailbox's current UIDVALIDITY.
func imapUIDValidity(t *testing.T, port int, user, password, folder string) uint32 {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	return client.selectFolder(folder)
}

// imapRecreateFolder deletes and recreates a mailbox, which is what makes a
// server hand out a fresh UIDVALIDITY and start UIDs over from 1.
func imapRecreateFolder(t *testing.T, port int, user, password, folder string) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.do("DELETE %q", folder)
	client.do("CREATE %q", folder)
}

// imapSearchSubject returns the UIDs in a folder whose Subject header matches,
// server side — how many copies of a sent message actually exist, regardless of
// what the local store cached.
func imapSearchSubject(t *testing.T, port int, user, password, folder, subject string) []uint32 {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	return parseIMAPSearch(client.do("UID SEARCH HEADER SUBJECT %q", subject))
}

func parseIMAPSearch(lines []string) []uint32 {
	var uids []uint32
	for _, line := range lines {
		fields := strings.Fields(line)
		if len(fields) < 2 || fields[0] != "*" || !strings.EqualFold(fields[1], "SEARCH") {
			continue
		}
		for _, field := range fields[2:] {
			uid, err := strconv.ParseUint(field, 10, 32)
			if err == nil && uid > 0 {
				uids = append(uids, uint32(uid))
			}
		}
	}
	return uids
}

// rawMessage builds an RFC 5322 message for imapAppend. Headers are passed
// verbatim so a test can deliver encodings and header shapes meron's own send
// path never emits.
func rawMessage(headers []string, body string) []byte {
	joined := strings.Join(headers, "\r\n")
	body = strings.ReplaceAll(body, "\n", "\r\n")
	return []byte(joined + "\r\n\r\n" + body + "\r\n")
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

func folderDelimiter(result map[string]any, name string) string {
	folders, _ := result["folders"].([]any)
	for _, item := range folders {
		folder, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if value, _ := folder["name"].(string); strings.EqualFold(value, name) {
			delimiter, _ := folder["delimiter"].(string)
			return delimiter
		}
	}
	return ""
}

func threadLength(result map[string]any) int {
	messages, _ := result["messages"].([]any)
	return len(messages)
}

func threadContainsSubject(result map[string]any, subject string) bool {
	return messagesContainSubject(result, subject)
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
	rows, _ := result["items"].([]any)
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
	message, ok := rows[0].(map[string]any)
	if !ok {
		t.Fatalf("thread row has type %T", rows[0])
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
