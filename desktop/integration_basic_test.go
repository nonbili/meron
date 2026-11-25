//go:build integration

package main

import (
	"encoding/base64"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// TestIntegrationMailFlow subtests: folders, sending, threading, flags, search, drafts and attachments.

func (f *mailFlow) folders(t *testing.T) {
	sidecar := f.sidecar
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

	// Not "Archive"/"Sent"/etc — the mail server pre-creates the special folders.
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
		t.Fatal("mail server reported no hierarchy delimiter for INBOX")
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

	// Renaming moves the subtree on the server and in the folder list, and
	// names every folder that moved. Special-use folders are refused.
	parent = "ITestRenameTree"
	child = parent + delimiter + "Child"
	renamedParent := "ITestRenamed"
	renamedChild := renamedParent + delimiter + "Child"
	callMap(t, sidecar, "folders.create", map[string]any{"account": "alice", "name": parent})
	callMap(t, sidecar, "folders.create", map[string]any{"account": "alice", "name": child})
	deadline = time.Now().Add(30 * time.Second)
	for {
		result = callMap(t, sidecar, "folders.list", map[string]any{"account": "alice"})
		if foldersContain(result, child) && folderDelimiter(result, parent) == delimiter {
			break
		}
		if time.Now().After(deadline) {
			t.Fatalf("nested folders never refreshed with delimiter %q: %v", delimiter, result)
		}
		time.Sleep(300 * time.Millisecond)
	}
	result = callMap(t, sidecar, "folders.rename", map[string]any{"account": "alice", "folder": parent, "name": renamedParent})
	if foldersContain(result, parent) || foldersContain(result, child) ||
		!foldersContain(result, renamedParent) || !foldersContain(result, renamedChild) {
		t.Fatalf("folders.rename did not move the subtree: %v", result)
	}
	if renamed, _ := result["renamed"].([]any); len(renamed) != 2 {
		t.Fatalf("renamed = %#v, want the folder and its child", result["renamed"])
	}
	if _, err := sidecar.Call("folders.rename", map[string]any{"account": "alice", "folder": "INBOX", "name": "Elsewhere"}); err == nil {
		t.Fatal("folders.rename accepted INBOX, want refusal")
	}
	callMap(t, sidecar, "folders.delete", map[string]any{"account": "alice", "folder": renamedParent})
}

func (f *mailFlow) sendAndReceive(t *testing.T) {
	sidecar, subject, messageID := f.sidecar, f.subject, f.messageID
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    subject,
		"body":       "hello from the integration test",
		"message_id": messageID,
	}); err != nil {
		t.Fatalf("send: %v", err)
	}

	message := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
		return str(m, "subject") == subject
	})
	if from := str(message, "from_addr"); from != "alice@mail.test" {
		t.Errorf("from_addr = %q, want alice@mail.test", from)
	}
	if str(message, "thread_key") == "" {
		t.Error("delivered message has empty thread_key")
	}
}

func (f *mailFlow) threading(t *testing.T) {
	sidecar, subject, messageID := f.sidecar, f.subject, f.messageID
	// A reply (References/In-Reply-To pointing at the first message) must
	// land in the same thread as the original in bob's mailbox.
	if _, err := sidecar.Call("send", map[string]any{
		"account":     "alice",
		"to":          "bob@mail.test",
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
}

func (f *mailFlow) moveAndFlags(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	moveSubject := "Meron integration move " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    moveSubject,
		"body":       "move me to the integration folder",
		"message_id": fmt.Sprintf("itest-move-%s@mail.test", nonce),
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
	if flags := imapFlags(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", uid); !strings.Contains(flags, `\Seen`) || !strings.Contains(flags, `\Flagged`) {
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
	if flags := imapFlags(t, server.imapPort, "bob@mail.test", testPassword, "ITestFolder", movedUID); !strings.Contains(flags, `\Seen`) || !strings.Contains(flags, `\Flagged`) {
		t.Fatalf("server flags for ITestFolder uid %d = %q, want \\Seen and \\Flagged", movedUID, flags)
	}
}

func (f *mailFlow) failedImapWriteDoesNotFalselyMarkReadLocally(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// Regression test: messages.markRead/markStarred/markAllRead must not
	// commit the local "seen"/"starred" state when the IMAP STORE itself
	// fails (dropped connection, offline, etc) — otherwise the local store
	// and every unread badge built on it silently drift from server truth.
	flagFailSubject := "Meron integration flagfail " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    flagFailSubject,
		"body":       "should stay unread after a failed write",
		"message_id": fmt.Sprintf("itest-flagfail-%s@mail.test", nonce),
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
}

func (f *mailFlow) searchAndStarredItems(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	searchSubject := "Meron integration search " + nonce
	searchBody := "unique-search-token-" + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    searchSubject,
		"body":       searchBody,
		"message_id": fmt.Sprintf("itest-search-%s@mail.test", nonce),
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
}

func (f *mailFlow) draftLifecycle(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	draftID := fmt.Sprintf("itest-draft-%s@mail.test", nonce)
	draftSubject := "Meron integration draft " + nonce
	if _, err := sidecar.Call("save_draft", map[string]any{
		"account":  "alice",
		"to":       "bob@mail.test",
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
}

func (f *mailFlow) quickReplyDraftLifecycle(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	// The quick reply saves a *reply* draft: it must thread into the
	// conversation it answers (that is what the frontend hydrates the box
	// from), and discard_draft with a thread_key must scrub it from the
	// thread so a cleared reply cannot resurface on the next thread open.
	quickSubject := "Meron integration quick reply " + nonce
	quickMessageID := fmt.Sprintf("itest-qr-%s@mail.test", nonce)
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
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
		"to":          "alice@mail.test",
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
}

func (f *mailFlow) attachments(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	attachmentSubject := "Meron integration attachment " + nonce
	attachmentBody := "attachment body " + nonce
	attachmentBytes := []byte("hello attachment " + nonce)
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    attachmentSubject,
		"body":       attachmentBody,
		"message_id": fmt.Sprintf("itest-attachment-%s@mail.test", nonce),
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
	// BODYSTRUCTURE must index the file before the thread is opened.
	page := callMap(t, sidecar, "messages.recent", map[string]any{
		"account": "bob", "folder": "INBOX", "refresh": false,
		"group": true, "attachments": true, "limit": 50,
	})
	cards, _ := page["threads"].([]any)
	found := false
	for _, row := range cards {
		card, ok := row.(map[string]any)
		if !ok || str(card, "subject") != attachmentSubject {
			continue
		}
		files, _ := card["files"].([]any)
		if len(files) != 1 {
			t.Fatalf("unopened attachment card files = %v, want one file", files)
		}
		file, ok := files[0].(map[string]any)
		if !ok || str(file, "filename") != "itest-note.txt" || str(file, "mime") != "text/plain" {
			t.Fatalf("unopened attachment metadata = %v", files)
		}
		found = true
	}
	if !found {
		t.Fatalf("attachment filter omitted the unopened thread: %v", page)
	}
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
}
