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
	})
}
