//go:build integration

package main

import (
	"fmt"
	"os"
	"reflect"
	"strings"
	"testing"
	"time"
)

// Exercise timing through the public API and real sync/IMAP writes, without
// advancing store clocks or invoking the scheduler directly.
func TestIntegrationDelayedRules(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	// Follow the replacement process as well as the original harness process.
	t.Cleanup(func() { sidecar.Close() })
	const account, user = "acct", "alice@mail.test"
	connectAccount(t, sidecar, server, account, user)
	subjectIs := func(subject string) func(map[string]any) bool {
		return func(message map[string]any) bool { return str(message, "subject") == subject }
	}
	deliver := func(subject string, date time.Time) map[string]any {
		t.Helper()
		imapAppend(t, server.imapPort, user, testPassword, "INBOX", rawMessage([]string{
			"From: sender@mail.test", "To: " + user, "Subject: " + subject,
			fmt.Sprintf("Message-ID: <%s@mail.test>", strings.ReplaceAll(subject, " ", "-")),
			"Date: " + date.Format(time.RFC1123Z),
		}, "body\r\n"))
		return pollInbox(t, sidecar, account, subjectIs(subject))
	}
	setRule := func(subject, trigger string, seconds int, action map[string]any) string {
		t.Helper()
		rule := map[string]any{"query": "subject:" + subject, "trigger": trigger, "delay_seconds": seconds}
		for key, value := range action {
			rule[key] = value
		}
		saved := callMap(t, sidecar, "rules.set", map[string]any{"rules": []map[string]any{rule}})
		// The editor's seconds/trigger payload must survive the real core, not
		// just a mocked save response.
		listed := callMap(t, sidecar, "rules.list", map[string]any{})
		if !reflect.DeepEqual(saved["rules"], listed["rules"]) {
			t.Fatalf("rules did not round-trip: saved=%v listed=%v", saved, listed)
		}
		got := listed["rules"].([]any)[0].(map[string]any)
		if got["trigger"] != trigger || got["delay_seconds"] != float64(seconds) {
			t.Fatalf("timing did not round-trip: %v", got)
		}
		return str(got, "id")
	}
	markRead := func(message map[string]any, seen bool) {
		t.Helper()
		callMap(t, sidecar, "messages.markRead", map[string]any{
			"account": account, "folder": "INBOX", "uid": num(message, "uid"), "seen": seen,
		})
	}
	// Keep requesting sync throughout a negative assertion: merely sleeping
	// would also pass if the scheduler never ran at all.
	remainsUnstarred := func(subject string, duration time.Duration) {
		t.Helper()
		until := time.Now().Add(duration)
		for time.Now().Before(until) {
			message := pollInbox(t, sidecar, account, subjectIs(subject))
			if boolValue(message, "starred") || strings.Contains(serverKeywords(t, server, user, subject), "\\Flagged") {
				t.Fatalf("%q was starred before it was due", subject)
			}
			time.Sleep(200 * time.Millisecond)
		}
	}
	awaitStar := func(subject string) {
		t.Helper()
		pollInbox(t, sidecar, account, func(message map[string]any) bool {
			return subjectIs(subject)(message) && boolValue(message, "starred")
		})
		if flags := serverKeywords(t, server, user, subject); !strings.Contains(flags, "\\Flagged") {
			t.Fatalf("star never reached IMAP: %s", flags)
		}
	}

	// Establish the initial mailbox snapshot before testing new arrivals.
	deliver("timing seed", time.Now())

	t.Run("arrival waits and survives a sidecar restart", func(t *testing.T) {
		old := "arrival historical"
		deliver(old, time.Now())
		setRule("arrival", "arrival", 6, map[string]any{"star": true})
		fresh := "arrival fresh"
		deliver(fresh, time.Now())
		remainsUnstarred(fresh, time.Second)

		// Reuse the profile, but a new process: persisted clocks and rules must
		// not be reset by its initial envelope/flag refresh.
		sidecar.Close()
		sidecar = NewSidecar("../meron-core/target/debug/meron-core", os.Stderr)
		if err := sidecar.Start(nil); err != nil {
			t.Fatal(err)
		}
		// The harness disables the OS keyring, so credentials are deliberately
		// process-local. Supply them again without replacing the saved profile.
		connectAccount(t, sidecar, server, account, user)
		remainsUnstarred(fresh, time.Second)
		awaitStar(fresh)
		remainsUnstarred(old, time.Second)
	})

	t.Run("local read waits, unread cancels, rereading restarts", func(t *testing.T) {
		const subject = "read-local"
		setRule(subject, "read", 5, map[string]any{"star": true})
		message := deliver(subject, time.Now())
		markRead(message, true)
		remainsUnstarred(subject, time.Second)
		markRead(message, false)
		remainsUnstarred(subject, 6*time.Second)
		markRead(message, true)
		remainsUnstarred(subject, time.Second)
		awaitStar(subject)
	})

	t.Run("a read from another client starts the wait and moves mail", func(t *testing.T) {
		const subject = "read-remote"
		setRule(subject, "read", 5, map[string]any{"move_to": "trash"})
		deliver(subject, time.Now())
		changeServerKeyword(t, server, user, subject, "add", "\\Seen")
		pollInbox(t, sidecar, account, func(message map[string]any) bool {
			return subjectIs(subject)(message) && boolValue(message, "seen")
		})
		until := time.Now().Add(time.Second)
		for time.Now().Before(until) {
			if copies := imapSearchSubject(t, server.imapPort, user, testPassword, "INBOX", subject); len(copies) != 1 {
				t.Fatalf("read rule moved mail before its deadline: %v", copies)
			}
			time.Sleep(200 * time.Millisecond)
		}
		// Poll the source too: timing is evaluated by Inbox sync, not Trash sync.
		deadline := time.Now().Add(30 * time.Second)
		for len(imapSearchSubject(t, server.imapPort, user, testPassword, "INBOX", subject)) != 0 {
			callMap(t, sidecar, "messages.recent", map[string]any{"account": account, "folder": "INBOX", "refresh": true})
			if time.Now().After(deadline) {
				t.Fatal("read rule never moved mail out of INBOX")
			}
			time.Sleep(300 * time.Millisecond)
		}
		mcpAssertServerCopies(t, server, user, "Trash", subject, 1)
		pollFolder(t, sidecar, account, "Trash", subjectIs(subject))
	})

	t.Run("age includes existing mail but not younger mail", func(t *testing.T) {
		old, young := "age-old", "age-young"
		deliver(old, time.Now().Add(-48*time.Hour))
		deliver(young, time.Now())
		setRule("age-", "age", 86400, map[string]any{"star": true})
		awaitStar(old)
		remainsUnstarred(young, time.Second)
	})

	t.Run("manual run ignores trigger and delay", func(t *testing.T) {
		const subject = "manual-delay"
		deliver(subject, time.Now())
		id := setRule(subject, "read", 86400, map[string]any{"star": true})
		remainsUnstarred(subject, time.Second)
		if matches := callMap(t, sidecar, "rules.matches", map[string]any{"id": id}); matches["count"] != float64(1) {
			t.Fatalf("matches: %v", matches)
		}
		if run := callMap(t, sidecar, "rules.run", map[string]any{"id": id}); run["count"] != float64(1) {
			t.Fatalf("run: %v", run)
		}
		awaitStar(subject)
	})

	t.Run("disabling a pending rule prevents its action", func(t *testing.T) {
		const subject = "disabled-delay"
		setRule(subject, "arrival", 5, map[string]any{"star": true})
		deliver(subject, time.Now())
		remainsUnstarred(subject, time.Second)
		listed := callMap(t, sidecar, "rules.list", map[string]any{})
		rule := listed["rules"].([]any)[0].(map[string]any)
		rule["disabled"] = true
		callMap(t, sidecar, "rules.set", map[string]any{"rules": []any{rule}})
		remainsUnstarred(subject, 6*time.Second)
	})

	t.Run("initial download is neither an arrival nor a read event", func(t *testing.T) {
		arrival, read := "initial-arrival", "initial-read"
		deliver(arrival, time.Now())
		deliver(read, time.Now())
		changeServerKeyword(t, server, user, read, "add", "\\Seen")
		// A separate profile sees this mail for the first time with rules
		// already enabled. Neither historical arrival nor already-seen mail
		// should start a clock.
		cold, _ := startSidecar(t)
		callMap(t, cold, "rules.set", map[string]any{"rules": []map[string]any{
			{"query": "subject:" + arrival, "trigger": "arrival", "delay_seconds": 2, "star": true},
			{"query": "subject:" + read, "trigger": "read", "delay_seconds": 2, "star": true},
		}})
		connectAccount(t, cold, server, account, user)
		pollInbox(t, cold, account, subjectIs(arrival))
		pollInbox(t, cold, account, func(message map[string]any) bool {
			return subjectIs(read)(message) && boolValue(message, "seen")
		})
		until := time.Now().Add(4 * time.Second)
		for time.Now().Before(until) {
			for _, subject := range []string{arrival, read} {
				message := pollInbox(t, cold, account, subjectIs(subject))
				if boolValue(message, "starred") || strings.Contains(serverKeywords(t, server, user, subject), "\\Flagged") {
					t.Fatalf("initial download triggered rule for %q", subject)
				}
			}
			time.Sleep(200 * time.Millisecond)
		}
	})
}

func TestIntegrationLocalFolderRules(t *testing.T) {
	server := startMailServer(t)
	sidecar, events := startSidecar(t)
	const account, user = "alice", "alice@mail.test"
	const target = "Receipts"
	connectAccount(t, sidecar, server, account, user)
	callMap(t, sidecar, "account.addLocal", map[string]any{"folder": target})

	subjectIs := func(subject string) func(map[string]any) bool {
		return func(message map[string]any) bool { return str(message, "subject") == subject }
	}
	deliver := func(folder, subject string, hasMessageID bool) {
		t.Helper()
		headers := []string{
			"From: sender@mail.test", "To: " + user, "Subject: " + subject,
			"Date: " + time.Now().Format(time.RFC1123Z),
		}
		if hasMessageID {
			headers = append(headers, "Message-ID: <"+subject+"@mail.test>")
		}
		imapAppend(t, server.imapPort, user, testPassword, folder, rawMessage(headers, "Local rule body\r\n"))
	}
	saveRule := func(subject string, actions map[string]any) string {
		t.Helper()
		actions["query"] = "subject:" + subject
		saved := callMap(t, sidecar, "rules.set", map[string]any{"rules": []map[string]any{actions}})
		return str(saved["rules"].([]any)[0].(map[string]any), "id")
	}

	t.Run("copy from Sent without a Message-ID and rerun without duplicates", func(t *testing.T) {
		const subject = "local-sent"
		targets := callMap(t, sidecar, "rules.targets", map[string]any{})
		local, ok := targets["local"].(map[string]any)
		if !ok || local["id"] != localAccountID || len(local["folders"].([]any)) != 1 {
			t.Fatalf("missing local rule destination: %v", targets)
		}
		deliver("Sent", subject, false)
		pollFolder(t, sidecar, account, "Sent", subjectIs(subject))
		result := callMap(t, sidecar, "rules.create", map[string]any{"rule": map[string]any{
			"query": "subject:" + subject, "sent": true,
			"copy_account": localAccountID, "copy_folder": target,
		}})
		id := str(result["rule"].(map[string]any), "id")
		for i := 1; i <= 2; i++ {
			run := callMap(t, sidecar, "rules.run", map[string]any{"id": id})
			if jsonNumber(run["count"]) != 1 {
				t.Fatalf("expected one sent message to match: %v", run)
			}
			awaitRuleArchiveEvents(t, events, target, i)
		}
		localMail := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": localAccountID, "folder": target, "limit": 20,
		})
		if messages := localMail["messages"].([]any); len(messages) != 1 || !messagesContainSubject(localMail, subject) {
			t.Fatalf("expected one local copy: %v", localMail)
		}
		mcpAssertServerCopies(t, server, user, "Sent", subject, 1)
	})

	t.Run("move new mail without a Message-ID and preserve its flags and tags", func(t *testing.T) {
		const subject = "local-move"
		deliver("INBOX", "local-rule-seed", true)
		pollInbox(t, sidecar, account, subjectIs("local-rule-seed"))
		saveRule(subject, map[string]any{
			"move_to": "folder", "folder_account": localAccountID, "folder": target,
			"star": true, "mark_read": true, "tags": []string{"receipts"},
		})
		deliver("INBOX", subject, false)
		deadline := time.Now().Add(30 * time.Second)
		for {
			callMap(t, sidecar, "messages.recent", map[string]any{"account": account, "folder": "INBOX", "refresh": true})
			if len(imapSearchSubject(t, server.imapPort, user, testPassword, "Trash", subject)) == 1 {
				break
			}
			if time.Now().After(deadline) {
				t.Fatal("the rule never moved the server original to Trash")
			}
			time.Sleep(250 * time.Millisecond)
		}
		mcpAssertServerCopies(t, server, user, "INBOX", subject, 0)
		archived := pollFolder(t, sidecar, localAccountID, target, subjectIs(subject))
		if !boolValue(archived, "seen") || !boolValue(archived, "starred") {
			t.Fatalf("archived flags were lost: %v", archived)
		}
		tagged := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": localAccountID, "folder": target, "query": "tag:receipts", "limit": 20,
		})
		if !messagesContainSubject(tagged, subject) {
			t.Fatalf("archived tags were lost: %v", tagged)
		}
	})

	t.Run("a missing destination leaves the server original in place", func(t *testing.T) {
		const subject = "local-missing"
		callMap(t, sidecar, "folders.create", map[string]any{"account": localAccountID, "name": "Missing"})
		id := saveRule(subject, map[string]any{"move_to": "folder", "folder_account": localAccountID, "folder": "Missing"})
		callMap(t, sidecar, "folders.delete", map[string]any{"account": localAccountID, "folder": "Missing"})
		deliver("INBOX", subject, true)

		deadline := time.Now().Add(30 * time.Second)
		for time.Now().Before(deadline) {
			callMap(t, sidecar, "messages.recent", map[string]any{"account": account, "folder": "INBOX", "refresh": true})
			status := callMap(t, sidecar, "rules.status", map[string]any{})
			log, _ := status["log"].([]any)
			for _, item := range log {
				entry := item.(map[string]any)
				if str(entry, "rule_id") == id && boolValue(entry, "failed") && strings.Contains(str(entry, "detail"), "Folder not found") {
					mcpAssertServerCopies(t, server, user, "INBOX", subject, 1)
					mcpAssertServerCopies(t, server, user, "Trash", subject, 0)
					return
				}
			}
			time.Sleep(250 * time.Millisecond)
		}
		t.Fatal("the missing local destination never appeared as a failed rule")
	})

	t.Run("server rule targets remain available when the archive cannot open", func(t *testing.T) {
		t.Setenv("MERON_ARCHIVE_DB", t.TempDir()) // A directory cannot be opened as SQLite.
		cold, _ := startSidecar(t)
		connectAccount(t, cold, server, account, user)
		if _, err := cold.Call("account.addLocal", map[string]any{"folder": target}); err == nil {
			t.Fatal("expected archive opening to fail")
		}
		targets := callMap(t, cold, "rules.targets", map[string]any{})
		accounts, _ := targets["accounts"].([]any)
		if len(accounts) != 1 || accounts[0].(map[string]any)["id"] != account || targets["local"] != nil {
			t.Fatalf("expected server targets only: %v", targets)
		}
	})
}

func awaitRuleArchiveEvents(t *testing.T, events *eventLog, folder string, want int) {
	t.Helper()
	deadline := time.Now().Add(30 * time.Second)
	for time.Now().Before(deadline) {
		if events.count(func(event sidecarEvent) bool {
			return event.name == "mail.synced" && str(event.detail, "account") == localAccountID &&
				str(event.detail, "folder") == folder
		}) >= want {
			return
		}
		time.Sleep(100 * time.Millisecond)
	}
	t.Fatalf("local rule did not report %d updates to %s", want, folder)
}

// A rule's forward goes out through SMTP with the original attached, is filed
// in Sent and happens before the rule's move. A forward never sets off another:
// bob's rule forwards it straight back, which would otherwise never stop.
func TestIntegrationRuleForward(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	const alice, bob = "alice@mail.test", "bob@mail.test"
	connectAccount(t, sidecar, server, "alice", alice)
	connectAccount(t, sidecar, server, "bob", bob)
	deliver := func(user, account, subject string) {
		t.Helper()
		imapAppend(t, server.imapPort, user, testPassword, "INBOX", rawMessage([]string{
			"From: Carol <carol@mail.test>", "To: " + user, "Subject: " + subject,
			fmt.Sprintf("Message-ID: <%s@mail.test>", strings.ReplaceAll(subject, " ", "-")),
			"Date: " + time.Now().Format(time.RFC1123Z),
		}, "Please pay by Friday.\r\n"))
		pollInbox(t, sidecar, account, func(message map[string]any) bool { return str(message, "subject") == subject })
	}
	// Establish the initial mailbox snapshots before testing new arrivals.
	deliver(alice, "alice", "forward seed")
	deliver(bob, "bob", "forward seed")

	saved := callMap(t, sidecar, "rules.set", map[string]any{"rules": []map[string]any{
		{
			"query":      "subject:invoice",
			"accounts":   []string{"alice"},
			"forward_to": "Bob <bob@mail.test>; BOB@mail.test",
			"move_to":    "trash",
		},
		{"query": "subject:invoice", "accounts": []string{"bob"}, "forward_to": alice},
	}})
	if got := str(saved["rules"].([]any)[0].(map[string]any), "forward_to"); got != bob {
		t.Fatalf("forward_to was not normalized: %q", got)
	}

	const subject = "invoice 42"
	deliver(alice, "alice", subject)
	sync := func() {
		for _, account := range []string{"alice", "bob"} {
			callMap(t, sidecar, "messages.recent", map[string]any{"account": account, "folder": "INBOX", "refresh": true})
		}
	}
	deadline := time.Now().Add(45 * time.Second)
	for len(imapSearchSubject(t, server.imapPort, bob, testPassword, "INBOX", "Fwd: "+subject)) == 0 ||
		len(imapSearchSubject(t, server.imapPort, alice, testPassword, "INBOX", subject)) != 0 {
		if time.Now().After(deadline) {
			t.Fatalf("rule never forwarded and binned: bob inbox %v, alice inbox %v",
				imapSearchSubject(t, server.imapPort, bob, testPassword, "INBOX", subject),
				imapSearchSubject(t, server.imapPort, alice, testPassword, "INBOX", subject))
		}
		sync()
		time.Sleep(300 * time.Millisecond)
	}
	// Give a looping forward every chance to show up.
	until := time.Now().Add(5 * time.Second)
	for time.Now().Before(until) {
		sync()
		time.Sleep(300 * time.Millisecond)
	}
	uids := mcpAssertServerCopies(t, server, bob, "INBOX", "Fwd: "+subject, 1)
	mcpAssertServerCopies(t, server, bob, "Sent", subject, 0)
	mcpAssertServerCopies(t, server, alice, "INBOX", subject, 0)
	mcpAssertServerCopies(t, server, alice, "Trash", subject, 1)
	mcpAssertServerCopies(t, server, alice, "Sent", "Fwd: "+subject, 1)

	raw := string(imapRawMessage(t, server.imapPort, bob, testPassword, "INBOX", uids[0]))
	for _, want := range []string{
		"From: <" + alice + ">",
		"Auto-Submitted: auto-forwarded",
		"---------- Forwarded message ---------",
		"From: Carol <carol@mail.test>",
		"Content-Type: message/rfc822",
		"Message-ID: <invoice-42@mail.test>",
		"Please pay by Friday.",
	} {
		if !strings.Contains(raw, want) {
			t.Fatalf("forward lacks %q:\n%s", want, raw)
		}
	}
}
