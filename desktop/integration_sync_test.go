//go:build integration

package main

import (
	"fmt"
	"strings"
	"testing"
	"time"
)

// TestIntegrationMailFlow subtests: iDLE, the unified inbox, and recovering sync state.

func (f *mailFlow) idlePushesFolderUpdatesWhileWatched(t *testing.T) {
	server, sidecar, events, nonce := f.server, f.sidecar, f.events, f.nonce
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
	// Finish the startup catch-up before appending, so it cannot satisfy
	// the event and cache assertions for the subsequent IDLE push.
	baseline := synced()
	callMap(t, sidecar, "watch.start", map[string]any{"account": "bob", "folder": folder})
	deadline := time.Now().Add(60 * time.Second)
	for synced() == baseline {
		if time.Now().After(deadline) {
			t.Fatalf("initial watch catch-up did not sync %s (synced %d, baseline %d)", folder, synced(), baseline)
		}
		time.Sleep(100 * time.Millisecond)
	}
	baseline = synced()
	pushedSubject := "Meron integration idle pushed " + nonce
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, folder, rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + pushedSubject,
		fmt.Sprintf("Message-ID: <itest-idle-%s@mail.test>", nonce),
		"Date: " + time.Now().Format(time.RFC1123Z),
	}, "pushed over IDLE"))

	// Wait for the expected cache contents as well as a new sync event;
	// refresh:false ensures the watcher alone brings the message into SQLite.
	waitCached := func(subject string, eventBaseline int, timeout time.Duration) {
		t.Helper()
		deadline := time.Now().Add(timeout)
		for {
			cached := callMap(t, sidecar, "messages.recent", map[string]any{
				"account": "bob",
				"folder":  folder,
				"refresh": false,
				"limit":   50,
			})
			eventCount := synced()
			if eventCount > eventBaseline && messagesContainSubject(cached, subject) {
				return
			}
			if time.Now().After(deadline) {
				t.Fatalf("IDLE watch did not cache %q in %s (synced %d, baseline %d): %v", subject, folder, eventCount, eventBaseline, cached)
			}
			time.Sleep(100 * time.Millisecond)
		}
	}
	waitCached(pushedSubject, baseline, 60*time.Second)

	// Let the initial IDLE responses settle, then stop another folder's
	// watcher. The original connection must stay parked, rather than doing
	// another catch-up sync in response to the shared pause signal.
	otherFolder := "ITestIdleOther"
	callMap(t, sidecar, "folders.create", map[string]any{"account": "bob", "name": otherFolder})
	callMap(t, sidecar, "watch.start", map[string]any{"account": "bob", "folder": otherFolder})
	time.Sleep(time.Second)
	baseline = synced()
	if result := callMap(t, sidecar, "watch.stop", map[string]any{"account": "bob", "folder": otherFolder}); !boolValue(result, "stopped") {
		t.Fatalf("watch.stop did not stop the other watcher: %v", result)
	}
	deadline = time.Now().Add(time.Second)
	for time.Now().Before(deadline) {
		if synced() != baseline {
			t.Fatalf("stopping %s restarted the watcher for %s", otherFolder, folder)
		}
		time.Sleep(50 * time.Millisecond)
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
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, folder, rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + quietSubject,
		fmt.Sprintf("Message-ID: <itest-idle-quiet-%s@mail.test>", nonce),
		"Date: " + time.Now().Format(time.RFC1123Z),
	}, "appended after the watch stopped"))
	if synced() != baseline {
		t.Fatalf("a stopped watch kept syncing %s", folder)
	}
	cached := callMap(t, sidecar, "messages.recent", map[string]any{
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
	waitCached(quietSubject, baseline, 30*time.Second)

	// Best-effort teardown only. The stop behavior was asserted above; once
	// the restarted watcher has caught up, waiting for a second lifecycle
	// event adds no coverage and can race the watcher entering IDLE.
	callMap(t, sidecar, "watch.stop", map[string]any{"account": "bob", "folder": folder})
}

func (f *mailFlow) unifiedInboxMergesAndWritesAcrossAccounts(t *testing.T) {
	sidecar, nonce, subject := f.sidecar, f.nonce, f.subject
	// The unified view fans messages.recent out over every included account
	// and merges the pages by date; markAllReadUnified fans the write out the
	// same way. Both must cover *both* accounts and report no per-account
	// failure — a partial merge looks like a working inbox with mail missing.
	unifiedSubject := "Meron integration unified " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "bob",
		"to":         "alice@mail.test",
		"subject":    unifiedSubject,
		"body":       "so alice has inbox mail too",
		"message_id": fmt.Sprintf("itest-unified-%s@mail.test", nonce),
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
}

func (f *mailFlow) unifiedMarkAllReadReportsAPartialFailure(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "alice", "alice@mail.test")
	connectAccount(t, sidecar, server, "bob", "bob@mail.test")
	nonce := fmt.Sprintf("%d", time.Now().UnixNano())

	// Give both healthy accounts a known unread row, and give a third account
	// a cached row before replacing its credentials with a bad password.
	// The fan-out must continue past that failure and identify it precisely.
	aliceSubject := "Meron integration unified partial alice " + nonce
	bobSubject := "Meron integration unified partial bob " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "bob",
		"to":         "alice@mail.test",
		"subject":    aliceSubject,
		"body":       "healthy alice unread",
		"message_id": fmt.Sprintf("itest-unified-partial-alice-%s@mail.test", nonce),
	}); err != nil {
		t.Fatalf("send alice partial-failure fixture: %v", err)
	}
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    bobSubject,
		"body":       "healthy bob unread",
		"message_id": fmt.Sprintf("itest-unified-partial-bob-%s@mail.test", nonce),
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
	connectAccount(t, sidecar, server, broken, "bob@mail.test")
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
		"email":     "bob@mail.test",
		"host":      "127.0.0.1",
		"port":      server.imapPort,
		"tls":       false,
		"smtp_host": "127.0.0.1",
		"smtp_port": server.smtpPort,
		"smtp_tls":  false,
		"user":      "bob@mail.test",
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
		{"alice@mail.test", num(aliceMessage, "uid")},
		{"bob@mail.test", num(bobMessage, "uid")},
	} {
		if flags := imapFlags(t, server.imapPort, fixture.user, testPassword, "INBOX", fixture.uid); !strings.Contains(flags, `\Seen`) {
			t.Fatalf("healthy account %s uid %d was not marked read on the server: %q", fixture.user, fixture.uid, flags)
		}
	}
}

func (f *mailFlow) unifiedCursorsAdvanceEachAccountIndependently(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "alice", "alice@mail.test")
	connectAccount(t, sidecar, server, "bob", "bob@mail.test")
	nonce := fmt.Sprintf("%d", time.Now().UnixNano())

	aliceSubject := "Meron integration unified cursor alice " + nonce
	bobSubject := "Meron integration unified cursor bob " + nonce
	// Alice must be newer than Bob's rows so she is exhausted on the first
	// page. Wall-clock send dates can put her last when sends cross a second.
	newest := time.Date(2025, time.January, 1, 12, 0, 0, 0, time.UTC)
	fixtures := []struct {
		account, subject, id string
		date                 time.Time
	}{{"alice", aliceSubject, "alice", newest}}
	for i := 0; i < 5; i++ {
		fixtures = append(fixtures, struct {
			account, subject, id string
			date                 time.Time
		}{
			"bob",
			fmt.Sprintf("%s %d", bobSubject, i),
			fmt.Sprintf("bob-%d", i),
			newest.Add(-time.Duration(i+1) * time.Minute),
		})
	}
	for _, fixture := range fixtures {
		user := fixture.account + "@mail.test"
		imapAppend(t, server.imapPort, user, testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@example.net>",
			"To: " + user,
			"Subject: " + fixture.subject,
			fmt.Sprintf("Message-ID: <itest-unified-cursor-%s-%s@mail.test>", fixture.id, nonce),
			"Date: " + fixture.date.Format(time.RFC1123Z),
		}, "unified cursor fixture"))
	}
	for _, fixture := range fixtures {
		pollInbox(t, sidecar, fixture.account, func(m map[string]any) bool {
			return str(m, "subject") == fixture.subject
		})
	}

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
	for _, fixture := range fixtures {
		if !seenSubjects[fixture.subject] {
			t.Fatalf("unified cursor traversal missed %q", fixture.subject)
		}
	}
	if !sawBobOnlyContinuation {
		t.Fatal("unified cursor never continued bob after alice was exhausted")
	}
}

func (f *mailFlow) coldSyncFromAnEmptyProfileSeesServerState(t *testing.T) {
	server, nonce := f.server, f.nonce
	// Everything above reads back through the same store that performed the
	// write, so a write that never left the client would still pass. A second
	// sidecar on a fresh profile has no cached rows at all: whatever it
	// reports came from the mail server.
	cold, _ := startSidecar(t)
	connectAccount(t, cold, server, "bob", "bob@mail.test")

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
}

func (f *mailFlow) sentCopyHonorsTheAccountOverride(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// A generic IMAP server does not file sent mail itself, so meron APPENDs
	// the copy — exactly once, since a duplicate upload would show the
	// message twice in Sent. The override exists for providers that do file
	// their own, and must suppress the upload entirely.
	defaultSubject := "Meron integration sentcopy default " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    defaultSubject,
		"body":       "should be filed in Sent by us",
		"message_id": fmt.Sprintf("itest-sentcopy-%s@mail.test", nonce),
	}); err != nil {
		t.Fatalf("send sentcopy fixture: %v", err)
	}
	pollFolder(t, sidecar, "alice", "Sent", func(m map[string]any) bool {
		return str(m, "subject") == defaultSubject
	})
	if uids := imapSearchSubject(t, server.imapPort, "alice@mail.test", testPassword, "Sent", defaultSubject); len(uids) != 1 {
		t.Fatalf("Sent holds %d copies of the message, want 1: %v", len(uids), uids)
	}

	callMap(t, sidecar, "account.setSaveSentCopy", map[string]any{"account": "alice", "value": false})
	// Back to the provider default, so nothing after this inherits it.
	defer callMap(t, sidecar, "account.setSaveSentCopy", map[string]any{"account": "alice", "value": nil})

	suppressedSubject := "Meron integration sentcopy off " + nonce
	if _, err := sidecar.Call("send", map[string]any{
		"account":    "alice",
		"to":         "bob@mail.test",
		"subject":    suppressedSubject,
		"body":       "should not be filed in Sent",
		"message_id": fmt.Sprintf("itest-sentcopy-off-%s@mail.test", nonce),
	}); err != nil {
		t.Fatalf("send suppressed sentcopy fixture: %v", err)
	}
	// Delivery is what proves the send itself went through.
	pollInbox(t, sidecar, "bob", func(m map[string]any) bool {
		return str(m, "subject") == suppressedSubject
	})
	if uids := imapSearchSubject(t, server.imapPort, "alice@mail.test", testPassword, "Sent", suppressedSubject); len(uids) != 0 {
		t.Fatalf("override off still uploaded a Sent copy: %v", uids)
	}
}

func (f *mailFlow) connectRejectsBadCredentials(t *testing.T) {
	server, sidecar := f.server, f.sidecar
	// Validation is what stands between a typo'd password and an account that
	// looks connected but can never sync, so the account must not be stored.
	id := "itest-badpass"
	_, err := sidecar.Call("account.connect", map[string]any{
		"account":   id,
		"email":     "alice@mail.test",
		"host":      "127.0.0.1",
		"port":      server.imapPort,
		"tls":       false,
		"smtp_host": "127.0.0.1",
		"smtp_port": server.smtpPort,
		"smtp_tls":  false,
		"user":      "alice@mail.test",
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
}

func (f *mailFlow) syncRecoversAfterAServerRestart(t *testing.T) {
	server := startMailServer(t)
	sidecar, events := startSidecar(t)
	connectAccount(t, sidecar, server, "alice", "alice@mail.test")
	connectAccount(t, sidecar, server, "bob", "bob@mail.test")
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

	restartMailServer(t, server)

	idleRestartSubject := "Meron integration idle restart " + nonce
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, folder, rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + idleRestartSubject,
		fmt.Sprintf("Message-ID: <itest-idle-restart-%s@mail.test>", nonce),
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
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + restartSubject,
		fmt.Sprintf("Message-ID: <itest-restart-%s@mail.test>", nonce),
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
}

func (f *mailFlow) flagWritesRecoverFromAStalePooledConnection(t *testing.T) {
	// Regression test: a pooled session the server had silently dropped
	// failed messages.markRead outright ("SELECT: Broken pipe"), so the
	// flag never reached the server and the next sync pulled the message
	// back as unread. The SELECT now runs as a retryable preflight, ahead
	// of the STORE that must not be replayed.
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	// The account talks to the mail server through a proxy the test can cut, so the
	// pooled socket can be killed without taking the server down with it.
	proxy := startIMAPProxy(t, server.imapPort)
	proxied := *server
	proxied.imapPort = proxy.port()
	connectAccount(t, sidecar, &proxied, "bob", "bob@mail.test")
	nonce := fmt.Sprintf("%d", time.Now().UnixNano())

	staleSubject := "Meron integration stale pool " + nonce
	imapAppend(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", rawMessage([]string{
		"From: Carol <carol@example.net>",
		"To: bob@mail.test",
		"Subject: " + staleSubject,
		fmt.Sprintf("Message-ID: <itest-stale-pool-%s@mail.test>", nonce),
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
	if flags := imapFlags(t, server.imapPort, "bob@mail.test", testPassword, "INBOX", uid); !strings.Contains(flags, `\Seen`) || !strings.Contains(flags, `\Flagged`) {
		t.Fatalf("server flags for INBOX uid %d = %q, want \\Seen and \\Flagged", uid, flags)
	}
}

func (f *mailFlow) sentFromAnotherClientSurfacesViaInboxSync(t *testing.T) {
	server, sidecar, nonce := f.server, f.sidecar, f.nonce
	// From is a send-as alias meron knows nothing about: the message must
	// still classify as outgoing purely from its Sent-folder provenance.
	externalSubject := "Meron integration external sent " + nonce
	raw := fmt.Sprintf(
		"From: Alice Alias <alice-alias@example.net>\r\nTo: bob@mail.test\r\nSubject: %s\r\n"+
			"Message-ID: <itest-external-%s@mail.test>\r\nDate: %s\r\n\r\n"+
			"sent from another client\r\n",
		externalSubject, nonce, time.Now().Format(time.RFC1123Z))
	imapAppend(t, server.imapPort, "alice@mail.test", testPassword, "Sent", []byte(raw))

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
}
