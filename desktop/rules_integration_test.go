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
