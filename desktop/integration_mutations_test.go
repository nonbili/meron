//go:build integration

package main

import (
	"fmt"
	"strings"
	"testing"
	"time"
)

// TestIntegrationMailFlow subtests: deleting, copying, archiving and reconciling changes made elsewhere.

func (f *mailFlow) deleteMovesToTrash(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	// Delete is destructive and folder-aware: a non-draft inbox message must
	// land in Trash (not expunge), so a stray UID never silently vanishes.
	deleteSubject := "Meron integration delete " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    deleteSubject,
		"body":       "delete me",
		"message_id": fmt.Sprintf("itest-delete-%s@mail.test", nonce),
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
}

func (f *mailFlow) emptyTrash(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	// Emptying is folder-wide and permanent — it clears the server folder
	// itself, not just the UIDs the client cached — and role-gated, so a
	// request for INBOX must be refused before anything is touched.
	emptySubject := "Meron integration empty " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    emptySubject,
		"body":       "empty me",
		"message_id": fmt.Sprintf("itest-empty-%s@mail.test", nonce),
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
}

func (f *mailFlow) copyKeepsOriginal(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	// Copy must duplicate, not move: the source UID stays put while a copy
	// appears in the target folder.
	copySubject := "Meron integration copy " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    copySubject,
		"body":       "copy me",
		"message_id": fmt.Sprintf("itest-copy-%s@mail.test", nonce),
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
}

func (f *mailFlow) markAllRead(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	readSubjectA := "Meron integration markall A " + nonce
	readSubjectB := "Meron integration markall B " + nonce
	for i, subj := range []string{readSubjectA, readSubjectB} {
		if _, err := sidecar.Call("send", map[string]any{
			"account":    "alice",
			"to":         "bob@mail.test",
			"subject":    subj,
			"body":       "unread fixture",
			"message_id": fmt.Sprintf("itest-markall-%d-%s@mail.test", i, nonce),
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
}

func (f *mailFlow) contactsSuggestFromHistory(t *testing.T) {
	sidecar := f.sidecar
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
		if ok && strings.Contains(str(contact, "addr"), "alice@mail.test") {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("contacts.suggest(%q) did not surface alice: %v", "alice", result)
	}
}

func (f *mailFlow) archiveFromTheArchiveFolderTakesTheInboxCopies(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
	// A conversation listed in the Archive folder can still have Inbox
	// copies (a reply arrived after it was archived). Archiving it there
	// moves those copies, not the Archive folder onto itself.
	archiveSubject := "Meron integration archive " + nonce
	rootID := fmt.Sprintf("itest-archive-%s@mail.test", nonce)
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    archiveSubject,
		"body":       "archive me",
		"message_id": rootID,
	}); err != nil {
		t.Fatalf("send archive fixture: %v", err)
	}
	original := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
		return str(m, "subject") == archiveSubject
	})
	threadKey := str(original, "thread_key")

	// The reply names the Archive folder the account resolved to.
	result := callMap(t, sidecar, "messages.archive", map[string]any{
		"account": "bob", "folder": "INBOX", "thread_key": threadKey,
	})
	archive := str(result, "folder")
	if moved := num(result, "moved"); moved != 1 || archive == "" || archive == "INBOX" {
		t.Fatalf("messages.archive from INBOX = %v, want 1 moved into the Archive folder", result)
	}
	pollFolder(t, sidecar, "bob", archive, func(m map[string]any) bool {
		return str(m, "subject") == archiveSubject
	})

	if _, err := sidecar.Call("send", map[string]any{
		"account":     "alice",
		"to":          "bob@mail.test",
		"subject":     "Re: " + archiveSubject,
		"body":        "a reply after archiving",
		"in_reply_to": rootID,
		"references":  rootID,
	}); err != nil {
		t.Fatalf("send archive reply: %v", err)
	}
	reply := pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
		return str(m, "subject") == "Re: "+archiveSubject
	})
	if str(reply, "thread_key") != threadKey {
		t.Fatalf("reply thread_key %q != archived thread_key %q", str(reply, "thread_key"), threadKey)
	}

	archived := formatImapThreadID("bob", archive, threadKey)
	result = callMap(t, sidecar, "messages.archive", map[string]any{
		"account": "bob", "folder": archive, "thread_key": threadKey,
	})
	if num(result, "moved") != 1 || str(result, "source_folder") != "INBOX" ||
		str(result, "thread_id") != archived || boolValue(result, "already_archived") {
		t.Fatalf("messages.archive from %s = %v, want the Inbox reply moved, thread still %s", archive, result, archived)
	}
	assertNoMessageInFolder(t, sidecar, "bob", "INBOX", func(m map[string]any) bool {
		return str(m, "subject") == "Re: "+archiveSubject
	})

	result = callMap(t, sidecar, "messages.archive", map[string]any{
		"account": "bob", "folder": archive, "thread_key": threadKey,
	})
	if num(result, "moved") != 0 || !boolValue(result, "already_archived") {
		t.Fatalf("archiving again = %v, want already_archived", result)
	}
}

func (f *mailFlow) groupedThreadCards(t *testing.T) {
	sidecar := f.sidecar
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
}

func (f *mailFlow) externalExpungePrunesTheLocalRow(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// A message another client deletes for good must not linger as a ghost
	// row: the sync compares the server's UID set against the cache and
	// drops what is no longer there.
	expungeSubject := "Meron integration expunge " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    expungeSubject,
		"body":       "delete me from another client",
		"message_id": fmt.Sprintf("itest-expunge-%s@mail.test", nonce),
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

	imapExpunge(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", uid)
	assertNoMessageInFolder(t, sidecar, "bob", "INBOX", func(m map[string]any) bool {
		return str(m, "subject") == expungeSubject
	})
}

func (f *mailFlow) externalFlagChangesReconcileLocally(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "alice", "alice@mail.test")
	connectAccount(t, sidecar, server, "bob", "bob@mail.test")
	nonce := fmt.Sprintf("%d", time.Now().UnixNano())

	// The reverse direction of the server-truth checks above: another client
	// can read or star a message, and the next sync must replace the cached
	// flags and unread count with what IMAP reports.
	flagSubject := "Meron integration external flags " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    flagSubject,
		"body":       "change my flags from another client",
		"message_id": fmt.Sprintf("itest-external-flags-%s@mail.test", nonce),
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

	imapSetFlags(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", uid, true, true)
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

	imapSetFlags(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", uid, false, false)
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
}

func (f *mailFlow) uidvalidityChangeClearsTheCachedFolder(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
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
		imapAppend(t, server.imapPort, "bob@mail.test", testPassword, folder, rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: bob@mail.test",
			"Subject: " + subj,
			fmt.Sprintf("Message-ID: <itest-uidv-%d-%s@mail.test>", i, nonce),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "before the mailbox was recreated"))
	}
	for _, subj := range []string{firstSubject, secondSubject} {
		pollFolder(t, sidecar, "bob", folder, func(m map[string]any) bool {
			return str(m, "subject") == subj
		})
	}
	before := imapUIDValidity(t, server.imapPort, "bob@mail.test", testPassword, folder)

	imapRecreateFolder(t, server.imapPort, "bob@mail.test", testPassword, folder)
	if after := imapUIDValidity(t, server.imapPort, "bob@mail.test", testPassword, folder); after == before {
		t.Skipf("mail server reused UIDVALIDITY %d for the recreated folder; nothing to reconcile", after)
	}

	freshSubject := "Meron integration uidv fresh " + nonce
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, folder, rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + freshSubject,
		fmt.Sprintf("Message-ID: <itest-uidv-fresh-%s@mail.test>", nonce),
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
}
