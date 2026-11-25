//go:build integration

package main

import (
	"encoding/base64"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// TestIntegrationMailFlow subtests: decoding real-world MIME, and the attachment cache.

func (f *mailFlow) encodedSubjectAndQuotedPrintableBodyDecode(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// Real mail arrives encoded — an RFC 2047 subject and a quoted-printable
	// body — and meron's own send path never emits either, so only a raw
	// append exercises the decode on the way in.
	want := "Méron héllo " + nonce
	encoded := "=?UTF-8?B?" + base64.StdEncoding.EncodeToString([]byte(want)) + "?="
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + encoded,
		fmt.Sprintf("Message-ID: <itest-encoded-%s@mail.test>", nonce),
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
}

func (f *mailFlow) multipartAlternativeKeepsBothTextAndHtml(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
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
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + altSubject,
		fmt.Sprintf("Message-ID: <itest-alt-%s@mail.test>", nonce),
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
}

func (f *mailFlow) nestedMultipartKeepsInlineAndDownloadableAttachments(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "bob", "bob@mail.test")
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
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + nestedSubject,
		fmt.Sprintf("Message-ID: <itest-nested-mime-%s@mail.test>", nonce),
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
}

func (f *mailFlow) messageWithoutAMessageIdStillThreads(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// Threading keys off Message-ID, but mail without one exists and must
	// still be readable: it needs a synthesized thread_key, or the row is
	// unreachable from the thread view even though the list shows it.
	bareSubject := "Meron integration no message id " + nonce
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
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
}

func (f *mailFlow) emptyJunk(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// emptyFolder is role-gated to Trash *and* Junk (the Trash half and the
	// INBOX refusal are covered above), and the role has to resolve for a
	// folder the account never had special-use metadata for.
	// The mail server pre-creates Junk; folders.list serves the cache and refreshes in
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
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "Junk", rawMessage([]string{
		"From: Spammer <spam@example.net>",
		"To: bob@mail.test",
		"Subject: " + junkSubject,
		fmt.Sprintf("Message-ID: <itest-junk-%s@mail.test>", nonce),
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
	if uids := imapSearchSubject(t, server.imapPort, "bob@mail.test", testPassword, "Junk", junkSubject); len(uids) != 0 {
		t.Fatalf("emptying Junk left %v on the server", uids)
	}
}

func (f *mailFlow) clearingTheAttachmentCacheRefetchesTheBytes(t *testing.T) {
	sidecar, nonce := f.sidecar, f.nonce
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
}
