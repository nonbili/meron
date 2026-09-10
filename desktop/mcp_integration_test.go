//go:build integration

package main

import (
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// Exercise the public MCP transport against the real core and mail server.
// Mutations are checked over independent IMAP connections, not the core cache.
func TestIntegrationMCP(t *testing.T) {
	server := startMaddy(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "allowed", "alice@maddy.test")
	connectAccount(t, sidecar, server, "excluded", "bob@maddy.test")
	app := &App{sidecar: sidecar}
	s := testMCPService(t, app, true)
	s.config.Clients[0].Send = true
	s.config.Clients[0].Organize = true
	s.config.Clients[0].Delete = true

	// Each subtest gets its own HTTP session and deadline.
	appendMessage := func(t *testing.T, user, folder, subject string) uint32 {
		t.Helper()
		imapAppend(t, server.imapPort, user, testPassword, folder, rawMessage([]string{
			"From: sender@maddy.test", "To: " + user,
			"Subject: " + subject, fmt.Sprintf("Message-ID: <%s@maddy.test>", strings.ReplaceAll(subject, " ", "-")),
			"Content-Type: text/plain; charset=utf-8",
		}, "Body for "+subject))
		return mcpAssertServerCopies(t, server, user, folder, subject, 1)[0]
	}

	t.Run("search read and account isolation", func(t *testing.T) {
		session := connectTestMCP(t, s)
		subject := "MCP visible message"
		uid := appendMessage(t, "alice@maddy.test", "INBOX", subject)
		appendMessage(t, "bob@maddy.test", "INBOX", "MCP private message")
		pollInbox(t, sidecar, "allowed", func(m map[string]any) bool { return str(m, "subject") == subject })
		private := pollInbox(t, sidecar, "excluded", func(m map[string]any) bool { return str(m, "subject") == "MCP private message" })

		accounts := operationResult(t, callMCP(t, session, "list_accounts", map[string]any{}))["accounts"].([]any)
		if len(accounts) != 1 || accounts[0].(map[string]any)["id"] != "allowed" {
			t.Fatalf("account leak: %v", accounts)
		}
		for _, args := range []map[string]any{
			{},
			{"account_id": "allowed", "query": subject, "server_search": true},
		} {
			result := operationResult(t, callMCP(t, session, "search_messages", args))
			threads := result["results"].(map[string]any)["threads"].([]any)
			if len(threads) != 1 || threads[0].(map[string]any)["subject"] != subject {
				t.Fatalf("search = %v", result)
			}
			thread := threads[0].(map[string]any)
			mcpWaitForBody(t, session, str(thread, "thread_id"), "Body for "+subject)
		}
		for _, tc := range []struct {
			tool string
			args map[string]any
		}{
			{"search_messages", map[string]any{"account_id": "excluded"}},
			{"read_thread", map[string]any{"thread_id": formatImapThreadID("excluded", "INBOX", str(private, "thread_key"))}},
			{"delete_permanently", map[string]any{"account_id": "excluded", "folder_id": "INBOX", "uids": []uint32{num(private, "uid")}, "request_id": "forbidden-delete"}},
		} {
			if !callMCP(t, session, tc.tool, tc.args).IsError {
				t.Fatalf("allowed access to excluded account: %s", tc.tool)
			}
		}
		mcpAssertServerCopies(t, server, "bob@maddy.test", "INBOX", "MCP private message", 1)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "INBOX", subject, 1)
		if flags := imapFlags(t, server.imapPort, "alice@maddy.test", testPassword, "INBOX", uid); strings.Contains(flags, `\Seen`) {
			t.Fatalf("MCP read marked message seen: %s", flags)
		}
	})

	t.Run("draft create update and trash", func(t *testing.T) {
		session := connectTestMCP(t, s)
		args := map[string]any{"account_id": "allowed", "to": "bob@maddy.test", "subject": "MCP original draft", "body": "Original draft body"}
		created := operationResult(t, callMCP(t, session, "create_draft", args))
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Drafts", "MCP original draft", 1)
		args["draft_id"] = created["draft_id"]
		args["subject"] = "MCP updated draft"
		args["body"] = "Updated draft body"
		operationResult(t, callMCP(t, session, "update_draft", args))
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Drafts", "MCP original draft", 0)
		uids := mcpAssertServerCopies(t, server, "alice@maddy.test", "Drafts", "MCP updated draft", 1)
		client := dialIMAP(t, server.imapPort, "alice@maddy.test", testPassword)
		client.selectFolder("Drafts")
		body := strings.Join(client.do("UID FETCH %d (BODY.PEEK[TEXT])", uids[0]), "\n")
		client.close()
		if !strings.Contains(body, "Updated draft body") || strings.Contains(body, "Original draft body") {
			t.Fatalf("server draft body = %s", body)
		}
		mcpAssertServerCopies(t, server, "bob@maddy.test", "INBOX", "MCP original draft", 0)
		mcpAssertServerCopies(t, server, "bob@maddy.test", "INBOX", "MCP updated draft", 0)
		operationResult(t, callMCP(t, session, "organize_messages", map[string]any{"account_id": "allowed", "folder_id": "Drafts", "uids": uids, "action": "trash"}))
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Drafts", "MCP updated draft", 0)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Trash", "MCP updated draft", 1)
	})

	t.Run("approved send and retry", func(t *testing.T) {
		session := connectTestMCP(t, s)
		args := map[string]any{"account_id": "allowed", "to": "bob@maddy.test", "subject": "MCP approved send", "body": "Approved body", "request_id": "send-integration"}
		queued := operationResult(t, callMCP(t, session, "send_message", args))
		id := mcpRequireStatus(t, queued, "pending_approval")
		mcpAssertServerCopies(t, server, "bob@maddy.test", "INBOX", "MCP approved send", 0)
		mcpRequireStatus(t, resolveMCP(t, app, id, true), "completed")
		pollInbox(t, sidecar, "excluded", func(m map[string]any) bool { return str(m, "subject") == "MCP approved send" })
		retry := operationResult(t, callMCP(t, session, "send_message", args))
		if mcpRequireStatus(t, retry, "completed") != id {
			t.Fatalf("retry created a new operation: %v", retry)
		}
		mcpRequireStatus(t, resolveMCP(t, app, id, true), "completed")
		mcpRequireStatus(t, operationResult(t, callMCP(t, session, "operation_status", map[string]any{"operation_id": id})), "completed")
		mcpAssertServerCopies(t, server, "bob@maddy.test", "INBOX", "MCP approved send", 1)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Sent", "MCP approved send", 1)
	})

	t.Run("trash moves only selected inbox messages", func(t *testing.T) {
		session := connectTestMCP(t, s)
		uid := appendMessage(t, "alice@maddy.test", "INBOX", "MCP trash target")
		appendMessage(t, "alice@maddy.test", "INBOX", "MCP trash neighbor")
		operationResult(t, callMCP(t, session, "organize_messages", map[string]any{
			"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{uid}, "action": "trash",
		}))
		mcpAssertServerCopies(t, server, "alice@maddy.test", "INBOX", "MCP trash target", 0)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Trash", "MCP trash target", 1)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "INBOX", "MCP trash neighbor", 1)
	})

	t.Run("permanent deletion fails closed without UIDPLUS", func(t *testing.T) {
		session := connectTestMCP(t, s)
		uid := appendMessage(t, "alice@maddy.test", "INBOX", "MCP delete target")
		other := appendMessage(t, "alice@maddy.test", "INBOX", "MCP unrelated deleted flag")
		// Refusing deletion must also preserve messages flagged by another client.
		client := dialIMAP(t, server.imapPort, "alice@maddy.test", testPassword)
		client.selectFolder("INBOX")
		client.do(`UID STORE %d +FLAGS (\Deleted)`, other)
		client.close()
		args := map[string]any{"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{uid}, "request_id": "delete-unsupported"}
		for range 2 {
			mcpRequireUIDPLUSRefusal(t, callMCP(t, session, "delete_permanently", args))
		}
		mcpAssertServerCopies(t, server, "alice@maddy.test", "INBOX", "MCP delete target", 1)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Trash", "MCP delete target", 0)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "INBOX", "MCP unrelated deleted flag", 1)
		if flags := imapFlags(t, server.imapPort, "alice@maddy.test", testPassword, "INBOX", uid); strings.Contains(flags, `\Deleted`) {
			t.Fatalf("refused deletion changed flags: %s", flags)
		}
	})

	t.Run("empty trash fails closed without UIDPLUS", func(t *testing.T) {
		session := connectTestMCP(t, s)
		appendMessage(t, "alice@maddy.test", "Trash", "MCP trash snapshot")
		args := map[string]any{"account_id": "allowed", "folder_id": "Trash", "request_id": "empty-trash"}
		mcpRequireUIDPLUSRefusal(t, callMCP(t, session, "empty_trash", args))
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Trash", "MCP trash snapshot", 1)
		appendMessage(t, "alice@maddy.test", "Trash", "MCP trash new arrival")
		mcpRequireUIDPLUSRefusal(t, callMCP(t, session, "empty_trash", args))
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Trash", "MCP trash snapshot", 1)
		mcpAssertServerCopies(t, server, "alice@maddy.test", "Trash", "MCP trash new arrival", 1)
		pending, err := app.mcpSettings("mcp.pending", nil)
		if err != nil || len(pending.([]any)) != 0 {
			t.Fatalf("unsupported deletion queued approval: %v (%v)", pending, err)
		}
	})
}

// Maddy 0.8 supports MOVE but not UIDPLUS. Keep this explicit: permanent
// deletion must not fall back to an unsafe mailbox-wide EXPUNGE. Successful
// deletion/snapshot execution requires a UIDPLUS-capable integration fixture.
func mcpRequireUIDPLUSRefusal(t *testing.T, result *mcp.CallToolResult) {
	t.Helper()
	if result.IsError {
		for _, content := range result.Content {
			if text, ok := content.(*mcp.TextContent); ok && strings.Contains(text.Text, "requires IMAP UIDPLUS") {
				return
			}
		}
	}
	t.Fatalf("want UIDPLUS refusal, got %+v", result)
}

func mcpRequireStatus(t *testing.T, result map[string]any, status string) string {
	t.Helper()
	id := str(result, "operation_id")
	if result["status"] != status || id == "" {
		t.Fatalf("want operation status %s: %v", status, result)
	}
	return id
}

func mcpAssertServerCopies(t *testing.T, server *maddyServer, user, folder, subject string, want int) []uint32 {
	t.Helper()
	uids := imapSearchSubject(t, server.imapPort, user, testPassword, folder, subject)
	if len(uids) != want {
		t.Fatalf("%s %s subject %q: server UIDs %v, want %d copies", user, folder, subject, uids, want)
	}
	return uids
}

func mcpWaitForBody(t *testing.T, session *mcp.ClientSession, threadID, body string) {
	t.Helper()
	deadline := time.Now().Add(30 * time.Second)
	for {
		result := operationResult(t, callMCP(t, session, "read_thread", map[string]any{"thread_id": threadID}))
		messages, _ := result["messages"].([]any)
		for _, message := range messages {
			if strings.Contains(str(message.(map[string]any), "body"), body) {
				return
			}
		}
		if time.Now().After(deadline) {
			t.Fatalf("body %q never loaded: %v", body, result)
		}
		time.Sleep(200 * time.Millisecond)
	}
}
