//go:build integration

package main

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestIntegrationMCPRules(t *testing.T) {
	sidecar, _ := startSidecar(t)
	app := &App{sidecar: sidecar}
	s := testMCPService(t, app, false)
	s.config.Clients[0].Organize = true
	session := connectTestMCP(t, s)
	list, err := session.ListTools(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, tool := range list.Tools {
		if strings.HasSuffix(tool.Name, "_rule") || tool.Name == "list_rules" || tool.Name == "list_rule_targets" {
			t.Fatalf("rule tool without Manage mail rules: %s", tool.Name)
		}
	}

	s.mu.Lock()
	s.config.Clients[0].Accounts = nil
	s.config.Clients[0].Organize = false
	s.config.Clients[0].ManageRules = true
	s.mu.Unlock()

	call := func(name string, args map[string]any) map[string]any {
		t.Helper()
		return operationResult(t, callMCP(t, session, name, args))
	}
	listRules := func() []any {
		t.Helper()
		return call("list_rules", map[string]any{})["rules"].([]any)
	}
	create := func(rule map[string]any) map[string]any {
		t.Helper()
		return call("create_rule", map[string]any{"rule": rule})["rule"].(map[string]any)
	}
	first := create(map[string]any{
		"conditions": []map[string]any{{"field": "subject", "value": "Invoice"}, {"field": "attachment"}},
		"trigger":    "read", "delay_seconds": 300, "tags": []string{" Receipts "}, "disabled": true,
	})
	id := str(first, "id")
	if id == "" || !reflect.DeepEqual(first["tags"], []any{"receipts"}) || first["disabled"] != true {
		t.Fatalf("created rule = %v", first)
	}
	snapshot := []any{first}
	second := create(map[string]any{"star": true})
	if second["disabled"] != true {
		t.Fatalf("new rule was enabled by default: %v", second)
	}
	if targets := call("list_rule_targets", map[string]any{}); len(targets["accounts"].([]any)) != 0 {
		t.Fatalf("targets of an empty profile = %v", targets)
	}
	age := create(map[string]any{"trigger": "age", "star": true})
	if !callMCP(t, session, "update_rule", map[string]any{
		"rule_id": age["id"], "rule": map[string]any{"disabled": false},
	}).IsError {
		t.Fatal("activated an age rule over MCP")
	}
	call("delete_rule", map[string]any{"rule_id": age["id"]})
	updated := call("update_rule", map[string]any{
		"rule_id": id, "rule": map[string]any{"move_to": "archive"},
	})["rule"].(map[string]any)
	if updated["id"] != id || updated["move_to"] != "archive" || updated["disabled"] != true ||
		!reflect.DeepEqual(updated["tags"], first["tags"]) || !reflect.DeepEqual(updated["conditions"], first["conditions"]) {
		t.Fatalf("updated rule = %v", updated)
	}

	failed := callMCP(t, session, "update_rule", map[string]any{
		"rule_id": id, "rule": map[string]any{"move_to": "", "tags": []string{}},
	})
	if !failed.IsError || !reflect.DeepEqual(listRules(), []any{updated, second}) {
		t.Fatal("invalid edit changed the list or a valid edit changed rule order")
	}
	updated = call("update_rule", map[string]any{
		"rule_id": id, "rule": map[string]any{"disabled": false, "tags": []string{}, "delay_seconds": 0},
	})["rule"].(map[string]any)
	if updated["disabled"] != false || len(updated["tags"].([]any)) != 0 || updated["delay_seconds"] != float64(0) {
		t.Fatalf("explicitly cleared fields = %v", updated)
	}
	if !callMCP(t, session, "create_rule", map[string]any{
		"rule": map[string]any{"star": true, "accounts": []string{"unknown"}},
	}).IsError {
		t.Fatal("created a rule for an unknown account")
	}
	if !callMCP(t, session, "delete_rule", map[string]any{"rule_id": "missing"}).IsError {
		t.Fatal("deleted a missing rule")
	}
	stale := callMap(t, sidecar, "rules.set", map[string]any{"rules": snapshot, "expected_rules": snapshot})
	if stale["conflict"] != true || !reflect.DeepEqual(stale["rules"], listRules()) {
		t.Fatalf("stale desktop save = %v", stale)
	}
	if _, err := app.runRule(map[string]any{"id": id, "expected_rule": first}); err == nil {
		t.Fatal("ran a changed rule using its old definition")
	}
	if _, err := app.ruleMatches(map[string]any{"id": id, "expected_rule": first}); err == nil {
		t.Fatal("counted matches for a changed rule using its old definition")
	}
	call("delete_rule", map[string]any{"rule_id": id})
	visible, err := app.rules()
	if err != nil || !reflect.DeepEqual(visible.(map[string]any)["rules"], []any{second}) {
		t.Fatalf("desktop rules after deletion = %v (%v)", visible, err)
	}

	s.mu.Lock()
	s.config.Clients[0].ManageRules = false
	s.mu.Unlock()
	result, err := session.CallTool(context.Background(), &mcp.CallToolParams{Name: "list_rules", Arguments: map[string]any{}})
	if err == nil && !result.IsError {
		t.Fatal("existing session can still list rules after permission was removed")
	}
}

// Exercise the public MCP transport against the real core and mail server.
// Mutations are checked over independent IMAP connections, not the core cache.
func TestIntegrationMCP(t *testing.T) {
	server := startMailServer(t)
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "allowed", "alice@mail.test")
	connectAccount(t, sidecar, server, "excluded", "bob@mail.test")
	app := &App{sidecar: sidecar}
	s := testMCPService(t, app, true)
	s.config.Clients[0].Send = true
	s.config.Clients[0].Organize = true
	s.config.Clients[0].Delete = true

	// Each subtest gets its own HTTP session and deadline.
	appendMessage := func(t *testing.T, user, folder, subject string) uint32 {
		t.Helper()
		imapAppend(t, server.imapPort, user, testPassword, folder, rawMessage([]string{
			"From: sender@mail.test", "To: " + user,
			"Subject: " + subject, fmt.Sprintf("Message-ID: <%s@mail.test>", strings.ReplaceAll(subject, " ", "-")),
			"Content-Type: text/plain; charset=utf-8",
		}, "Body for "+subject))
		return mcpAssertServerCopies(t, server, user, folder, subject, 1)[0]
	}

	t.Run("search read and account isolation", func(t *testing.T) {
		session := connectTestMCP(t, s)
		subject := "MCP visible message"
		uid := appendMessage(t, "alice@mail.test", "INBOX", subject)
		appendMessage(t, "bob@mail.test", "INBOX", "MCP private message")
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
		mcpAssertServerCopies(t, server, "bob@mail.test", "INBOX", "MCP private message", 1)
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", subject, 1)
		if flags := imapFlags(t, server.imapPort, "alice@mail.test", testPassword, "INBOX", uid); strings.Contains(flags, `\Seen`) {
			t.Fatalf("MCP read marked message seen: %s", flags)
		}
	})

	t.Run("draft create update and trash", func(t *testing.T) {
		session := connectTestMCP(t, s)
		args := map[string]any{"account_id": "allowed", "to": "bob@mail.test", "subject": "MCP original draft", "body": "Original draft body"}
		created := operationResult(t, callMCP(t, session, "create_draft", args))
		mcpAssertServerCopies(t, server, "alice@mail.test", "Drafts", "MCP original draft", 1)
		args["draft_id"] = created["draft_id"]
		args["subject"] = "MCP updated draft"
		args["body"] = "Updated draft body"
		operationResult(t, callMCP(t, session, "update_draft", args))
		mcpAssertServerCopies(t, server, "alice@mail.test", "Drafts", "MCP original draft", 0)
		uids := mcpAssertServerCopies(t, server, "alice@mail.test", "Drafts", "MCP updated draft", 1)
		client := dialIMAP(t, server.imapPort, "alice@mail.test", testPassword)
		client.selectFolder("Drafts")
		body := strings.Join(client.do("UID FETCH %d (BODY.PEEK[TEXT])", uids[0]), "\n")
		client.close()
		if !strings.Contains(body, "Updated draft body") || strings.Contains(body, "Original draft body") {
			t.Fatalf("server draft body = %s", body)
		}
		mcpAssertServerCopies(t, server, "bob@mail.test", "INBOX", "MCP original draft", 0)
		mcpAssertServerCopies(t, server, "bob@mail.test", "INBOX", "MCP updated draft", 0)
		operationResult(t, callMCP(t, session, "organize_messages", map[string]any{"account_id": "allowed", "folder_id": "Drafts", "uids": uids, "action": "trash"}))
		mcpAssertServerCopies(t, server, "alice@mail.test", "Drafts", "MCP updated draft", 0)
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP updated draft", 1)
	})

	t.Run("approved send and retry", func(t *testing.T) {
		session := connectTestMCP(t, s)
		args := map[string]any{"account_id": "allowed", "to": "bob@mail.test", "subject": "MCP approved send", "body": "Approved body", "request_id": "send-integration"}
		queued := operationResult(t, callMCP(t, session, "send_message", args))
		id := mcpRequireStatus(t, queued, "pending_approval")
		mcpAssertServerCopies(t, server, "bob@mail.test", "INBOX", "MCP approved send", 0)
		mcpRequireStatus(t, resolveMCP(t, app, id, true), "completed")
		pollInbox(t, sidecar, "excluded", func(m map[string]any) bool { return str(m, "subject") == "MCP approved send" })
		retry := operationResult(t, callMCP(t, session, "send_message", args))
		if mcpRequireStatus(t, retry, "completed") != id {
			t.Fatalf("retry created a new operation: %v", retry)
		}
		mcpRequireStatus(t, resolveMCP(t, app, id, true), "completed")
		mcpRequireStatus(t, operationResult(t, callMCP(t, session, "operation_status", map[string]any{"operation_id": id})), "completed")
		mcpAssertServerCopies(t, server, "bob@mail.test", "INBOX", "MCP approved send", 1)
		mcpAssertServerCopies(t, server, "alice@mail.test", "Sent", "MCP approved send", 1)
	})

	t.Run("trash moves only selected inbox messages", func(t *testing.T) {
		session := connectTestMCP(t, s)
		uid := appendMessage(t, "alice@mail.test", "INBOX", "MCP trash target")
		appendMessage(t, "alice@mail.test", "INBOX", "MCP trash neighbor")
		operationResult(t, callMCP(t, session, "organize_messages", map[string]any{
			"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{uid}, "action": "trash",
		}))
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP trash target", 0)
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash target", 1)
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP trash neighbor", 1)
	})

	t.Run("permanent deletion preserves unrelated deleted messages", func(t *testing.T) {
		session := connectTestMCP(t, s)
		uid := appendMessage(t, "alice@mail.test", "INBOX", "MCP delete target")
		other := appendMessage(t, "alice@mail.test", "INBOX", "MCP unrelated deleted flag")
		client := dialIMAP(t, server.imapPort, "alice@mail.test", testPassword)
		client.selectFolder("INBOX")
		client.do(`UID STORE %d +FLAGS (\Deleted)`, other)
		client.close()
		args := map[string]any{"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{uid}, "request_id": "delete-supported"}
		queued := operationResult(t, callMCP(t, session, "delete_permanently", args))
		id := mcpRequireStatus(t, queued, "pending_approval")
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP delete target", 1)
		mcpRequireStatus(t, resolveMCP(t, app, id, true), "completed")
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP delete target", 0)
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP unrelated deleted flag", 1)
		mcpRequireStatus(t, operationResult(t, callMCP(t, session, "delete_permanently", args)), "completed")
	})

	t.Run("empty trash uses the approved snapshot", func(t *testing.T) {
		session := connectTestMCP(t, s)
		appendMessage(t, "alice@mail.test", "Trash", "MCP trash snapshot")
		args := map[string]any{"account_id": "allowed", "folder_id": "Trash", "request_id": "empty-trash-supported"}
		queued := operationResult(t, callMCP(t, session, "empty_trash", args))
		id := mcpRequireStatus(t, queued, "pending_approval")
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash snapshot", 1)
		appendMessage(t, "alice@mail.test", "Trash", "MCP trash new arrival")
		mcpRequireStatus(t, resolveMCP(t, app, id, true), "completed")
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash snapshot", 0)
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash new arrival", 1)
		mcpRequireStatus(t, operationResult(t, callMCP(t, session, "empty_trash", args)), "completed")
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash new arrival", 1)
	})
}

// Keep the no-UIDPLUS refusal coverage with a Dovecot fixture whose advertised
// capabilities omit UIDPLUS. Production deletion must never fall back to a
// mailbox-wide EXPUNGE that could remove another client's flagged messages.
func TestIntegrationMCPWithoutUIDPLUS(t *testing.T) {
	server := startMailServerWithConfig(t, "imap_capability = IMAP4rev1 IDLE MOVE\n")
	sidecar, _ := startSidecar(t)
	connectAccount(t, sidecar, server, "allowed", "alice@mail.test")
	app := &App{sidecar: sidecar}
	s := testMCPService(t, app, true)
	s.config.Clients[0].Delete = true
	appendMessage := func(t *testing.T, user, folder, subject string) uint32 {
		t.Helper()
		imapAppend(t, server.imapPort, user, testPassword, folder, rawMessage([]string{
			"From: sender@mail.test", "To: " + user, "Subject: " + subject,
			fmt.Sprintf("Message-ID: <%s@mail.test>", strings.ReplaceAll(subject, " ", "-")),
			"Content-Type: text/plain; charset=utf-8",
		}, "Body for "+subject))
		return mcpAssertServerCopies(t, server, user, folder, subject, 1)[0]
	}
	t.Run("permanent deletion fails closed without UIDPLUS", func(t *testing.T) {
		session := connectTestMCP(t, s)
		uid := appendMessage(t, "alice@mail.test", "INBOX", "MCP delete target")
		other := appendMessage(t, "alice@mail.test", "INBOX", "MCP unrelated deleted flag")
		// Refusing deletion must also preserve messages flagged by another client.
		client := dialIMAP(t, server.imapPort, "alice@mail.test", testPassword)
		client.selectFolder("INBOX")
		client.do(`UID STORE %d +FLAGS (\Deleted)`, other)
		client.close()
		args := map[string]any{"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{uid}, "request_id": "delete-unsupported"}
		for range 2 {
			mcpRequireUIDPLUSRefusal(t, callMCP(t, session, "delete_permanently", args))
		}
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP delete target", 1)
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP delete target", 0)
		mcpAssertServerCopies(t, server, "alice@mail.test", "INBOX", "MCP unrelated deleted flag", 1)
		if flags := imapFlags(t, server.imapPort, "alice@mail.test", testPassword, "INBOX", uid); strings.Contains(flags, `\Deleted`) {
			t.Fatalf("refused deletion changed flags: %s", flags)
		}
	})

	t.Run("empty trash fails closed without UIDPLUS", func(t *testing.T) {
		session := connectTestMCP(t, s)
		appendMessage(t, "alice@mail.test", "Trash", "MCP trash snapshot")
		args := map[string]any{"account_id": "allowed", "folder_id": "Trash", "request_id": "empty-trash"}
		mcpRequireUIDPLUSRefusal(t, callMCP(t, session, "empty_trash", args))
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash snapshot", 1)
		appendMessage(t, "alice@mail.test", "Trash", "MCP trash new arrival")
		mcpRequireUIDPLUSRefusal(t, callMCP(t, session, "empty_trash", args))
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash snapshot", 1)
		mcpAssertServerCopies(t, server, "alice@mail.test", "Trash", "MCP trash new arrival", 1)
		pending, err := app.mcpSettings("mcp.pending", nil)
		if err != nil || len(pending.([]any)) != 0 {
			t.Fatalf("unsupported deletion queued approval: %v (%v)", pending, err)
		}
	})
}

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

func mcpAssertServerCopies(t *testing.T, server *mailServer, user, folder, subject string, want int) []uint32 {
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
