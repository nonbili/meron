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

	t.Run("a rule tags new mail and leaves mail already here", func(t *testing.T) {
		old := "Invoice from before"
		deliver(old)
		pollInbox(t, sidecar, "acct", func(card map[string]any) bool { return str(card, "subject") == old })
		callMap(t, sidecar, "tags.setRules", map[string]any{
			"rules": []map[string]any{{"query": "subject:invoice", "tags": []string{"finance"}}},
		})
		fresh := "Invoice just in"
		deliver(fresh)
		pollCards(t, sidecar, hasTags(fresh, "finance"))
		pollCards(t, sidecar, hasTags(old))
	})
}
