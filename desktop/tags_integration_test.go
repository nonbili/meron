//go:build integration

package main

import (
	"fmt"
	"slices"
	"strings"
	"testing"
	"time"
)

func serverKeywords(t *testing.T, server *mailServer, user, subject string) string {
	t.Helper()
	return runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "fetch", "-u", user, "flags", "mailbox", "INBOX", "subject", subject)
}

func changeServerKeyword(t *testing.T, server *mailServer, user, subject, change, keyword string) {
	t.Helper()
	runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "flags", change, "-u", user, keyword, "mailbox", "INBOX", "subject", subject)
}

func cardTags(card map[string]any) []string {
	tags := []string{}
	items, _ := card["tags"].([]any)
	for _, item := range items {
		tags = append(tags, fmt.Sprint(item))
	}
	return tags
}

// pollCards waits for a thread card, the shape the app lists and tags live on.
func pollCards(t *testing.T, sidecar *Sidecar, predicate func(map[string]any) bool) {
	t.Helper()
	deadline := time.Now().Add(30 * time.Second)
	for time.Now().Before(deadline) {
		result := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "acct", "folder": "INBOX", "refresh": true, "limit": 50, "group": true,
		})
		rows, _ := result["threads"].([]any)
		for _, row := range rows {
			if card, ok := row.(map[string]any); ok && predicate(card) {
				return
			}
		}
		time.Sleep(500 * time.Millisecond)
	}
	t.Fatal("no matching card within deadline")
}

// Tags against a real IMAP server: what Meron writes is what another client
// reads, and what another client changes comes back.
func TestIntegrationTags(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	const user = "alice@mail.test"
	connectAccount(t, sidecar, server, "acct", user)
	deliver := func(subject string) uint32 {
		imapAppend(t, server.imapPort, user, testPassword, "INBOX", rawMessage([]string{
			"From: sender@mail.test", "To: " + user, "Subject: " + subject,
			fmt.Sprintf("Message-ID: <%s@mail.test>", strings.ReplaceAll(subject, " ", "-")),
			"Content-Type: text/plain; charset=utf-8",
		}, "Body"))
		return mcpAssertServerCopies(t, server, user, "INBOX", subject, 1)[0]
	}
	hasTags := func(subject string, tags ...string) func(map[string]any) bool {
		return func(card map[string]any) bool {
			return str(card, "subject") == subject && slices.Equal(cardTags(card), tags)
		}
	}

	t.Run("a local tag reaches the server, non-ASCII included", func(t *testing.T) {
		subject := "Tag written here"
		uid := deliver(subject)
		pollInbox(t, sidecar, "acct", func(card map[string]any) bool { return str(card, "subject") == subject })
		callMap(t, sidecar, "messages.setTags", map[string]any{
			"account": "acct", "folder": "INBOX", "uids": []uint32{uid}, "add": []string{"Todo", "重要"},
		})
		pollCards(t, sidecar, hasTags(subject, "todo", "重要"))
		deadline := time.Now().Add(20 * time.Second)
		for {
			flags := serverKeywords(t, server, user, subject)
			if strings.Contains(flags, "todo") && strings.Contains(flags, "meron-x-e9878de8a681") {
				break
			}
			if time.Now().After(deadline) {
				t.Fatalf("keywords never reached the server: %s", flags)
			}
			time.Sleep(200 * time.Millisecond)
		}
		found := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": "acct", "folder": "INBOX", "query": "tag:todo", "refresh": true, "limit": 50, "group": true,
		})
		if threads, _ := found["threads"].([]any); len(threads) != 1 {
			t.Fatalf("tag search: %v", found)
		}
	})

	t.Run("a keyword from another client comes and goes", func(t *testing.T) {
		subject := "Tag written elsewhere"
		deliver(subject)
		pollInbox(t, sidecar, "acct", func(card map[string]any) bool { return str(card, "subject") == subject })
		changeServerKeyword(t, server, user, subject, "add", "waiting")
		pollCards(t, sidecar, hasTags(subject, "waiting"))
		changeServerKeyword(t, server, user, subject, "remove", "waiting")
		pollCards(t, sidecar, hasTags(subject))
	})

	t.Run("renaming and deleting a tag reach the server", func(t *testing.T) {
		subject := "Tag renamed everywhere"
		uid := deliver(subject)
		pollInbox(t, sidecar, "acct", func(card map[string]any) bool { return str(card, "subject") == subject })
		callMap(t, sidecar, "messages.setTags", map[string]any{
			"account": "acct", "folder": "INBOX", "uids": []uint32{uid}, "add": []string{"draftname"},
		})
		keywordsSettle := func(want func(string) bool) {
			t.Helper()
			deadline := time.Now().Add(20 * time.Second)
			for {
				flags := serverKeywords(t, server, user, subject)
				if want(flags) {
					return
				}
				if time.Now().After(deadline) {
					t.Fatalf("keywords never settled: %s", flags)
				}
				time.Sleep(200 * time.Millisecond)
			}
		}
		keywordsSettle(func(flags string) bool { return strings.Contains(flags, "draftname") })

		callMap(t, sidecar, "tags.replace", map[string]any{"tag": "draftname", "to": "Final Name"})
		pollCards(t, sidecar, hasTags(subject, "final-name"))
		keywordsSettle(func(flags string) bool {
			return strings.Contains(flags, "final-name") && !strings.Contains(flags, "draftname")
		})

		callMap(t, sidecar, "tags.replace", map[string]any{"tag": "final-name"})
		pollCards(t, sidecar, hasTags(subject))
		keywordsSettle(func(flags string) bool { return !strings.Contains(flags, "final-name") })
	})

	t.Run("a rule tags new mail and leaves mail already here", func(t *testing.T) {
		old := "Invoice from before"
		deliver(old)
		pollInbox(t, sidecar, "acct", func(card map[string]any) bool { return str(card, "subject") == old })
		callMap(t, sidecar, "rules.set", map[string]any{
			"rules": []map[string]any{{"query": "subject:invoice", "tags": []string{"finance"}}},
		})
		fresh := "Invoice just in"
		deliver(fresh)
		pollCards(t, sidecar, hasTags(fresh, "finance"))
		pollCards(t, sidecar, hasTags(old))
	})

	t.Run("rules read, star and move new mail on the server", func(t *testing.T) {
		callMap(t, sidecar, "rules.set", map[string]any{
			"rules": []map[string]any{
				{"query": "subject:keepme", "mark_read": true, "star": true},
				{"query": "subject:binme", "move_to": "trash"},
			},
		})

		kept := "Keepme please"
		deliver(kept)
		pollInbox(t, sidecar, "acct", func(message map[string]any) bool {
			return str(message, "subject") == kept && message["seen"] == true && message["starred"] == true
		})
		settled := func(flags string) bool {
			return strings.Contains(flags, "\\Seen") && strings.Contains(flags, "\\Flagged")
		}
		deadline := time.Now().Add(10 * time.Second)
		for !settled(serverKeywords(t, server, user, kept)) {
			if time.Now().After(deadline) {
				t.Fatalf("server flags: %s", serverKeywords(t, server, user, kept))
			}
			time.Sleep(200 * time.Millisecond)
		}

		binned := "Binme now"
		bystander := "Stays right here"
		deliver(binned)
		deliver(bystander)
		pollFolder(t, sidecar, "acct", "Trash", func(message map[string]any) bool { return str(message, "subject") == binned })
		assertNoMessageInFolder(t, sidecar, "acct", "INBOX", func(message map[string]any) bool { return str(message, "subject") == binned })
		pollInbox(t, sidecar, "acct", func(message map[string]any) bool { return str(message, "subject") == bystander })

		callMap(t, sidecar, "folders.create", map[string]any{"account": "acct", "name": "Receipts"})
		callMap(t, sidecar, "folders.list", map[string]any{"account": "acct", "refresh": true})
		callMap(t, sidecar, "rules.set", map[string]any{
			"rules": []map[string]any{{"query": "subject:fileme", "move_to": "folder", "folder": "Receipts"}},
		})
		filed := "Fileme away"
		deliver(filed)
		pollFolder(t, sidecar, "acct", "Receipts", func(message map[string]any) bool { return str(message, "subject") == filed })
		assertNoMessageInFolder(t, sidecar, "acct", "INBOX", func(message map[string]any) bool { return str(message, "subject") == filed })

		callMap(t, sidecar, "rules.set", map[string]any{
			"rules": []map[string]any{
				{"query": "subject:copyme", "copy_account": "acct", "copy_folder": "Receipts"},
				{"query": "subject:notmine", "star": true, "accounts": []string{"someone-else"}},
			},
		})
		copied := "Copyme over"
		foreign := "Notmine at all"
		deliver(copied)
		deliver(foreign)
		pollFolder(t, sidecar, "acct", "Receipts", func(message map[string]any) bool { return str(message, "subject") == copied })
		pollInbox(t, sidecar, "acct", func(message map[string]any) bool { return str(message, "subject") == copied })
		pollInbox(t, sidecar, "acct", func(message map[string]any) bool {
			return str(message, "subject") == foreign && message["starred"] != true
		})

		callMap(t, sidecar, "rules.set", map[string]any{
			"rules": []map[string]any{{"move_to": "folder", "folder": "Receipts"}},
		})
		anything := "Anything at all"
		deliver(anything)
		pollFolder(t, sidecar, "acct", "Receipts", func(message map[string]any) bool { return str(message, "subject") == anything })

		// Delivered with no rule to take it, so only running one by hand can.
		callMap(t, sidecar, "rules.set", map[string]any{"rules": []map[string]any{}})
		late := "Bystander already here"
		deliver(late)
		pollInbox(t, sidecar, "acct", func(message map[string]any) bool { return str(message, "subject") == late })
		saved := callMap(t, sidecar, "rules.set", map[string]any{
			"rules": []map[string]any{{"query": "subject:bystander", "star": true}},
		})
		id := str(saved["rules"].([]any)[0].(map[string]any), "id")
		if matches := callMap(t, sidecar, "rules.matches", map[string]any{"id": id}); matches["count"] != float64(1) {
			t.Fatalf("matches: %v", matches)
		}
		if run := callMap(t, sidecar, "rules.run", map[string]any{"id": id}); run["count"] != float64(1) {
			t.Fatalf("run: %v", run)
		}
		pollInbox(t, sidecar, "acct", func(message map[string]any) bool {
			return str(message, "subject") == late && message["starred"] == true
		})
	})
}

// A rule across two real accounts: a copy reaches the other account and the mail
// stays, a move reaches it and the original goes to Trash, and repeating a run by
// hand doesn't copy again.
func TestIntegrationRulesAcrossAccounts(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	const alice, bob = "alice@mail.test", "bob@mail.test"
	connectAccount(t, sidecar, server, "a", alice)
	connectAccount(t, sidecar, server, "b", bob)
	deliver := func(subject string) {
		imapAppend(t, server.imapPort, alice, testPassword, "INBOX", rawMessage([]string{
			"From: sender@mail.test", "To: " + alice, "Subject: " + subject,
			"Message-ID: <" + strings.ReplaceAll(subject, " ", ".") + "@mail.test>",
		}, "body\r\n"))
	}
	subjectIs := func(subject string) func(map[string]any) bool {
		return func(message map[string]any) bool { return str(message, "subject") == subject }
	}
	// Mail only counts as new once the folder has been synced with something in it.
	deliver("seed")
	pollInbox(t, sidecar, "a", subjectIs("seed"))
	callMap(t, sidecar, "folders.create", map[string]any{"account": "b", "name": "Saved"})
	callMap(t, sidecar, "folders.list", map[string]any{"account": "b", "refresh": true})
	callMap(t, sidecar, "folders.list", map[string]any{"account": "a", "refresh": true})

	callMap(t, sidecar, "rules.set", map[string]any{
		"rules": []map[string]any{
			{"query": "subject:copyacross", "accounts": []string{"a"}, "copy_account": "b", "copy_folder": "Saved"},
			{"query": "subject:moveacross", "accounts": []string{"a"}, "move_to": "folder", "folder": "Saved", "folder_account": "b"},
		},
	})
	copied, moved := "copyacross one", "moveacross two"
	deliver(copied)
	deliver(moved)
	pollFolder(t, sidecar, "b", "Saved", subjectIs(copied))
	pollFolder(t, sidecar, "b", "Saved", subjectIs(moved))
	pollInbox(t, sidecar, "a", subjectIs(copied))
	pollFolder(t, sidecar, "a", "Trash", subjectIs(moved))
	assertNoMessageInFolder(t, sidecar, "a", "INBOX", subjectIs(moved))

	// Nothing here to catch it as new, so only a run by hand copies it, once.
	callMap(t, sidecar, "rules.set", map[string]any{"rules": []map[string]any{}})
	existing := "existing three"
	deliver(existing)
	pollInbox(t, sidecar, "a", subjectIs(existing))
	saved := callMap(t, sidecar, "rules.set", map[string]any{
		"rules": []map[string]any{{"query": "subject:existing", "copy_account": "b", "copy_folder": "Saved"}},
	})
	id := str(saved["rules"].([]any)[0].(map[string]any), "id")
	for range 2 {
		if run := callMap(t, sidecar, "rules.run", map[string]any{"id": id}); run["count"] != float64(1) {
			// The second run sees the first still queued, or done.
			t.Logf("run: %v", run)
		}
		time.Sleep(time.Second)
	}
	pollFolder(t, sidecar, "b", "Saved", subjectIs(existing))
	time.Sleep(2 * time.Second)
	found := runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "search", "-u", bob, "mailbox", "Saved", "header", "subject", existing)
	if lines := strings.Count(strings.TrimSpace(found), "\n") + 1; lines != 1 {
		t.Fatalf("copies in Saved: %d\n%s", lines, found)
	}

	// A different message under the same Message-ID isn't a copy of this one.
	imapAppend(t, server.imapPort, alice, testPassword, "INBOX", rawMessage([]string{
		"From: sender@mail.test", "To: " + alice, "Subject: resent four", "Message-ID: <resent@mail.test>",
	}, "the first, shorter\r\n"))
	imapAppend(t, server.imapPort, bob, testPassword, "Saved", rawMessage([]string{
		"From: sender@mail.test", "To: " + alice, "Subject: resent four", "Message-ID: <resent@mail.test>",
	}, "x\r\n"))
	pollInbox(t, sidecar, "a", subjectIs("resent four"))
	saved = callMap(t, sidecar, "rules.set", map[string]any{
		"rules": []map[string]any{{"query": "subject:resent", "copy_account": "b", "copy_folder": "Saved"}},
	})
	callMap(t, sidecar, "rules.run", map[string]any{"id": str(saved["rules"].([]any)[0].(map[string]any), "id")})
	deadline := time.Now().Add(15 * time.Second)
	for {
		found = runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "search", "-u", bob, "mailbox", "Saved", "header", "subject", "resent four")
		if strings.Count(strings.TrimSpace(found), "\n") == 1 {
			break
		}
		if time.Now().After(deadline) {
			t.Fatalf("a resent message wasn't copied beside the earlier one:\n%s", found)
		}
		time.Sleep(300 * time.Millisecond)
	}

	// An identical copy already there still gets what a rule did to the original.
	flagged := "flagged five"
	deliver(flagged)
	pollInbox(t, sidecar, "a", subjectIs(flagged))
	saved = callMap(t, sidecar, "rules.set", map[string]any{
		"rules": []map[string]any{{"query": "subject:flagged", "copy_account": "b", "copy_folder": "Saved"}},
	})
	callMap(t, sidecar, "rules.run", map[string]any{"id": str(saved["rules"].([]any)[0].(map[string]any), "id")})
	pollFolder(t, sidecar, "b", "Saved", subjectIs(flagged))
	saved = callMap(t, sidecar, "rules.set", map[string]any{
		"rules": []map[string]any{{"query": "subject:flagged", "star": true, "copy_account": "b", "copy_folder": "Saved"}},
	})
	callMap(t, sidecar, "rules.run", map[string]any{"id": str(saved["rules"].([]any)[0].(map[string]any), "id")})
	deadline = time.Now().Add(15 * time.Second)
	for !strings.Contains(runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "fetch", "-u", bob, "flags", "mailbox", "Saved", "header", "subject", flagged), "\\Flagged") {
		if time.Now().After(deadline) {
			t.Fatal("the star never reached the copy already in Saved")
		}
		time.Sleep(300 * time.Millisecond)
	}

	// A cross-account move still lands if an earlier copy has since been deleted.
	runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "expunge", "-u", bob, "mailbox", "Saved", "header", "subject", existing)
	saved = callMap(t, sidecar, "rules.set", map[string]any{
		"rules": []map[string]any{{"query": "subject:existing", "move_to": "folder", "folder": "Saved", "folder_account": "b"}},
	})
	id = str(saved["rules"].([]any)[0].(map[string]any), "id")
	callMap(t, sidecar, "rules.run", map[string]any{"id": id})
	pollFolder(t, sidecar, "a", "Trash", subjectIs(existing))
	found = runCmd(t, dockerBin(t), "exec", server.container, "doveadm", "search", "-u", bob, "mailbox", "Saved", "header", "subject", existing)
	if strings.TrimSpace(found) == "" {
		t.Fatal("the original went to Trash but no copy reached Saved")
	}

	status := callMap(t, sidecar, "rules.status", map[string]any{})
	if log, _ := status["log"].([]any); len(log) == 0 {
		t.Fatal("no log of what rules did")
	}
}
