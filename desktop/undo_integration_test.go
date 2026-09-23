//go:build integration

package main

import (
	"fmt"
	"slices"
	"strconv"
	"strings"
	"testing"
	"time"
)

// Undo of a move, archive, or move to Trash, driven through the same App
// handlers the frontend calls. Each undo moves back the `target_uids` the
// action reported, so every case checks those against the server — over an
// independent IMAP connection, not the core cache — and then that the undo
// put back exactly those messages.
func TestIntegrationUndo(t *testing.T) {
	server := startMaddy(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "bob", "bob@maddy.test")
	app := &App{sidecar: sidecar}
	const user = "bob@maddy.test"
	nonce := fmt.Sprintf("%d", time.Now().UnixNano())

	// deliver appends a message to bob's INBOX and waits for its cached row.
	// An empty messageID leaves the Message-ID header off entirely.
	deliver := func(t *testing.T, subject, messageID string, extra ...string) map[string]any {
		t.Helper()
		headers := []string{
			"From: Carol <carol@example.net>",
			"To: " + user,
			"Subject: " + subject,
			"Date: " + time.Now().Format(time.RFC1123Z),
		}
		if messageID != "" {
			headers = append(headers, "Message-ID: <"+messageID+">")
		}
		headers = append(headers, extra...)
		imapAppend(t, server.imapPort, user, testPassword, "INBOX", rawMessage(headers, "body of "+subject))
		return pollInbox(t, sidecar, "bob", func(m map[string]any) bool { return str(m, "subject") == subject })
	}
	threadID := func(t *testing.T, row map[string]any) string {
		t.Helper()
		key := str(row, "thread_key")
		if key == "" {
			t.Fatalf("row has no thread_key: %v", row)
		}
		// The core spells a uid-derived key as the bare UID.
		if uid, ok := strings.CutPrefix(key, "uid:"); ok {
			return "bob#INBOX#" + uid
		}
		return formatImapThreadID("bob", "INBOX", key)
	}
	call := func(t *testing.T, name string, handler func(map[string]any) (any, error), payload map[string]any) map[string]any {
		t.Helper()
		res, err := handler(payload)
		if err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		out, ok := res.(map[string]any)
		if !ok {
			t.Fatalf("%s: expected object result, got %T", name, res)
		}
		return out
	}
	// undo mirrors the frontend's undo toast: move the reported copies back.
	undo := func(t *testing.T, res map[string]any, from, to string) map[string]any {
		t.Helper()
		ids := make([]any, 0)
		for _, uid := range targetUIDs(t, res) {
			ids = append(ids, strconv.FormatUint(uint64(uid), 10))
		}
		movedThreadID := str(res, "thread_id")
		if movedThreadID == "" || !strings.Contains(movedThreadID, "#"+from+"#") {
			t.Fatalf("result thread_id %q does not point into %s: %v", movedThreadID, from, res)
		}
		return call(t, "undo", app.mailMove, map[string]any{
			"thread_id":        movedThreadID,
			"target_folder_id": to,
			"message_ids":      ids,
		})
	}
	onServer := func(t *testing.T, folder, messageID string) []uint32 {
		t.Helper()
		return imapSearchMessageID(t, server.imapPort, user, testPassword, folder, messageID)
	}

	t.Run("trash then undo restores the message", func(t *testing.T) {
		messageID := "undo-trash-" + nonce + "@maddy.test"
		row := deliver(t, "Undo trash "+nonce, messageID)

		res := call(t, "mail.delete", app.mailDelete, map[string]any{"thread_id": threadID(t, row)})
		trashed := targetUIDs(t, res)
		if got := onServer(t, "Trash", messageID); len(trashed) != 1 || !slices.Equal(got, trashed) {
			t.Fatalf("target_uids = %v, server Trash copies = %v", trashed, got)
		}

		if moved := num(undo(t, res, "Trash", "INBOX"), "moved"); moved != 1 {
			t.Fatalf("undo moved %d, want 1", moved)
		}
		if got := onServer(t, "INBOX", messageID); len(got) != 1 {
			t.Fatalf("INBOX copies after undo = %v, want 1", got)
		}
		if got := onServer(t, "Trash", messageID); len(got) != 0 {
			t.Fatalf("Trash still holds %v after undo", got)
		}
	})

	t.Run("a thread moves out of trash by key straight after the delete", func(t *testing.T) {
		// Moving a conversation out of Trash by its thread key resolves the
		// cached Trash rows, which the delete must have filled — nothing else
		// refreshes Trash between the two.
		messageID := "undo-trash-key-" + nonce + "@maddy.test"
		row := deliver(t, "Undo trash by key "+nonce, messageID)

		res := call(t, "mail.delete", app.mailDelete, map[string]any{"thread_id": threadID(t, row)})
		back := call(t, "mail.move", app.mailMove, map[string]any{
			"thread_id":        str(res, "thread_id"),
			"target_folder_id": "INBOX",
		})
		if moved := num(back, "moved"); moved != 1 {
			t.Fatalf("thread-key move moved %d, want 1: %v", moved, back)
		}
		if got := onServer(t, "INBOX", messageID); len(got) != 1 {
			t.Fatalf("INBOX copies after the move = %v, want 1", got)
		}
	})

	t.Run("undo leaves older conversation mail in trash", func(t *testing.T) {
		rootID := "undo-root-" + nonce + "@maddy.test"
		replyID := "undo-reply-" + nonce + "@maddy.test"
		subject := "Undo conversation " + nonce
		root := deliver(t, subject, rootID)
		reply := deliver(t, "Re: "+subject, replyID,
			"In-Reply-To: <"+rootID+">", "References: <"+rootID+">")
		if str(root, "thread_key") != str(reply, "thread_key") {
			t.Fatalf("reply did not thread with root: %q vs %q", str(root, "thread_key"), str(reply, "thread_key"))
		}
		thread := threadID(t, root)

		// Trash the root on its own first, then the reply: Trash now holds the
		// whole conversation, and only the reply's deletion is being undone.
		call(t, "mail.delete", app.mailDelete, map[string]any{
			"thread_id":   thread,
			"message_ids": []any{strconv.FormatUint(uint64(num(root, "uid")), 10)},
		})
		res := call(t, "mail.delete", app.mailDelete, map[string]any{
			"thread_id":   thread,
			"message_ids": []any{strconv.FormatUint(uint64(num(reply, "uid")), 10)},
		})
		if got := onServer(t, "Trash", replyID); !slices.Equal(targetUIDs(t, res), got) {
			t.Fatalf("target_uids = %v, server Trash reply copies = %v", targetUIDs(t, res), got)
		}

		if moved := num(undo(t, res, "Trash", "INBOX"), "moved"); moved != 1 {
			t.Fatalf("undo moved %d, want 1", moved)
		}
		if got := onServer(t, "INBOX", replyID); len(got) != 1 {
			t.Fatalf("reply not back in INBOX: %v", got)
		}
		if got := onServer(t, "Trash", rootID); len(got) != 1 {
			t.Fatalf("undo took the earlier-trashed root out of Trash: %v", got)
		}
		if got := onServer(t, "INBOX", rootID); len(got) != 0 {
			t.Fatalf("undo restored the earlier-trashed root to INBOX: %v", got)
		}
	})

	t.Run("message without a message-id undoes by uid", func(t *testing.T) {
		// Its uid: thread key names a UID the move changes, so only the
		// reported copy can bring it back.
		subject := "Undo no message id " + nonce
		row := deliver(t, subject, "")
		if !strings.HasPrefix(str(row, "thread_key"), "uid:") {
			t.Fatalf("expected a uid-derived thread key, got %q", str(row, "thread_key"))
		}

		res := call(t, "mail.delete", app.mailDelete, map[string]any{"thread_id": threadID(t, row)})
		if got := imapSearchSubject(t, server.imapPort, user, testPassword, "Trash", subject); !slices.Equal(targetUIDs(t, res), got) {
			t.Fatalf("target_uids = %v, server Trash copies = %v", targetUIDs(t, res), got)
		}
		if moved := num(undo(t, res, "Trash", "INBOX"), "moved"); moved != 1 {
			t.Fatalf("undo moved %d, want 1", moved)
		}
		if got := imapSearchSubject(t, server.imapPort, user, testPassword, "INBOX", subject); len(got) != 1 {
			t.Fatalf("INBOX copies after undo = %v, want 1", got)
		}
	})

	t.Run("move then undo", func(t *testing.T) {
		callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": "ITestUndo"})
		messageID := "undo-move-" + nonce + "@maddy.test"
		row := deliver(t, "Undo move "+nonce, messageID)

		res := call(t, "mail.move", app.mailMove, map[string]any{
			"thread_id":        threadID(t, row),
			"target_folder_id": "ITestUndo",
		})
		if got := onServer(t, "ITestUndo", messageID); len(got) != 1 || !slices.Equal(targetUIDs(t, res), got) {
			t.Fatalf("target_uids = %v, server copies = %v", targetUIDs(t, res), got)
		}
		// mail.move reports no thread_id of its own; the frontend derives it.
		res["thread_id"] = strings.Replace(threadID(t, row), "#INBOX#", "#ITestUndo#", 1)
		if moved := num(undo(t, res, "ITestUndo", "INBOX"), "moved"); moved != 1 {
			t.Fatalf("undo moved %d, want 1", moved)
		}
		if got := onServer(t, "INBOX", messageID); len(got) != 1 {
			t.Fatalf("INBOX copies after undo = %v, want 1", got)
		}
	})

	t.Run("a move past the recent window reports every copy", func(t *testing.T) {
		// More copies than the post-move refresh window reads: all of them
		// must still be found, or Undo would silently leave some behind.
		const total = 60
		subject := "Undo bulk " + nonce
		for i := range total {
			imapAppend(t, server.imapPort, user, testPassword, "INBOX", rawMessage([]string{
				"From: Carol <carol@example.net>",
				"To: " + user,
				fmt.Sprintf("Subject: %s #%d", subject, i),
				fmt.Sprintf("Message-ID: <undo-bulk-%d-%s@maddy.test>", i, nonce),
			}, "bulk"))
		}
		callMap(t, sidecar, "messages.recent", map[string]any{"account": "bob", "folder": "INBOX", "refresh": true, "limit": 200})
		inbox := imapSearchSubject(t, server.imapPort, user, testPassword, "INBOX", subject)
		if len(inbox) != total {
			t.Fatalf("INBOX holds %d bulk messages, want %d", len(inbox), total)
		}
		ids := make([]any, 0, total)
		for _, uid := range inbox {
			ids = append(ids, strconv.FormatUint(uint64(uid), 10))
		}
		callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": "ITestUndoBulk"})

		res := call(t, "mail.move", app.mailMove, map[string]any{
			"thread_id":        fmt.Sprintf("bob#INBOX#%d", inbox[0]),
			"target_folder_id": "ITestUndoBulk",
			"message_ids":      ids,
		})
		moved := imapSearchSubject(t, server.imapPort, user, testPassword, "ITestUndoBulk", subject)
		slices.Sort(moved)
		if got := targetUIDs(t, res); len(got) != total || !slices.Equal(got, moved) {
			t.Fatalf("target_uids has %d of %d copies: %v, server %v", len(got), total, got, moved)
		}
		res["thread_id"] = fmt.Sprintf("bob#ITestUndoBulk#%d", moved[0])
		if n := num(undo(t, res, "ITestUndoBulk", "INBOX"), "moved"); n != total {
			t.Fatalf("undo moved %d, want %d", n, total)
		}
		if got := imapSearchSubject(t, server.imapPort, user, testPassword, "ITestUndoBulk", subject); len(got) != 0 {
			t.Fatalf("undo left %d copies behind", len(got))
		}
	})

	t.Run("archive then undo", func(t *testing.T) {
		messageID := "undo-archive-" + nonce + "@maddy.test"
		row := deliver(t, "Undo archive "+nonce, messageID)

		res := call(t, "mail.archive", app.mailArchive, map[string]any{"thread_id": threadID(t, row)})
		archive := str(res, "folder")
		if archive == "" {
			t.Fatalf("mail.archive reported no folder: %v", res)
		}
		if got := onServer(t, archive, messageID); len(got) != 1 || !slices.Equal(targetUIDs(t, res), got) {
			t.Fatalf("target_uids = %v, server %s copies = %v", targetUIDs(t, res), archive, got)
		}
		if moved := num(undo(t, res, archive, "INBOX"), "moved"); moved != 1 {
			t.Fatalf("undo moved %d, want 1", moved)
		}
		if got := onServer(t, "INBOX", messageID); len(got) != 1 {
			t.Fatalf("INBOX copies after undo = %v, want 1", got)
		}
	})
}

// targetUIDs reads the `target_uids` a move, archive, or delete reported,
// sorted to compare with a server SEARCH.
func targetUIDs(t *testing.T, res map[string]any) []uint32 {
	t.Helper()
	raw, ok := res["target_uids"].([]any)
	if !ok {
		t.Fatalf("result has no target_uids: %v", res)
	}
	uids := make([]uint32, 0, len(raw))
	for _, item := range raw {
		value, ok := item.(float64)
		if !ok || value <= 0 {
			t.Fatalf("bad target_uids entry %v in %v", item, res)
		}
		uids = append(uids, uint32(value))
	}
	slices.Sort(uids)
	return uids
}

func imapSearchMessageID(t *testing.T, port int, user, password, folder, messageID string) []uint32 {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	uids := parseIMAPSearch(client.do("UID SEARCH HEADER Message-ID %q", "<"+messageID+">"))
	slices.Sort(uids)
	return uids
}
