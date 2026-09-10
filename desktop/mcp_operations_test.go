package main

import (
	"context"
	"encoding/json"
	"reflect"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func operationResult(t *testing.T, result *mcp.CallToolResult) map[string]any {
	t.Helper()
	if result.IsError {
		t.Fatalf("tool failed: %#v", result)
	}
	data, err := json.Marshal(result.StructuredContent)
	if err != nil {
		t.Fatal(err)
	}
	var out map[string]any
	if err := json.Unmarshal(data, &out); err != nil || out == nil {
		t.Fatalf("no structured result: %s (%v)", data, err)
	}
	return out
}
func sendArgs() map[string]any {
	return map[string]any{"account_id": "allowed", "to": "reader@example.com", "cc": "copy@example.com", "bcc": "private@example.com", "subject": "Review", "body": "Exact body to review", "request_id": "send-1"}
}
func resolveMCP(t *testing.T, app *App, id string, approve bool) map[string]any {
	t.Helper()
	out, err := app.mcpSettings("mcp.resolve", map[string]any{"id": id, "approve": approve})
	if err != nil {
		t.Fatal(err)
	}
	return out.(map[string]any)
}

func TestMCPSendNeedsMeronApprovalAndExecutesOnce(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}})
	s := testMCPService(t, app, false)
	s.config.Clients[0].Send = true
	session := connectTestMCP(t, s)
	queued := operationResult(t, callMCP(t, session, "send_message", sendArgs()))
	id := queued["operation_id"].(string)
	if queued["status"] != "pending_approval" || len(writer.calls) != 0 {
		t.Fatalf("sent before approval: %v %v", queued, writer.calls)
	}
	pending, err := app.mcpSettings("mcp.pending", nil)
	if err != nil {
		t.Fatal(err)
	}
	preview := pending.([]any)[0].(map[string]any)["preview"].(map[string]any)
	if preview["body"] != "Exact body to review" || preview["bcc"] != "private@example.com" {
		t.Fatal(preview)
	}
	// The external client cannot submit approval or smuggle an approval argument.
	forged := sendArgs()
	forged["confirmed"] = true
	if !callMCP(t, session, "send_message", forged).IsError {
		t.Fatal("accepted agent confirmation")
	}
	if result, err := connectTestMCP(t, s).CallTool(context.Background(), &mcp.CallToolParams{Name: "mcp.resolve", Arguments: map[string]any{"id": id, "approve": true}}); err == nil && !result.IsError {
		t.Fatal("approval exposed over MCP")
	}
	if result := resolveMCP(t, app, id, true); result["status"] != "completed" {
		t.Fatal(result)
	}
	if len(writer.calls) != 1 || writer.calls[0].Method != "send" || writer.calls[0].Params["body"] != "Exact body to review" || writer.calls[0].Params["bcc"] != "private@example.com" {
		t.Fatal(writer.calls)
	}
	resolveMCP(t, app, id, true)
	retry := operationResult(t, callMCP(t, session, "send_message", sendArgs()))
	if retry["operation_id"] != id || retry["status"] != "completed" || len(writer.calls) != 1 {
		t.Fatal("retry sent again")
	}
	changed := sendArgs()
	changed["body"] = "Changed after approval"
	if !callMCP(t, session, "send_message", changed).IsError {
		t.Fatal("request ID reused with changed body")
	}
	status := operationResult(t, callMCP(t, session, "operation_status", map[string]any{"operation_id": id}))
	if status["status"] != "completed" {
		t.Fatal(status)
	}
}

func TestMCPPendingOperationsFailClosed(t *testing.T) {
	for _, action := range []string{"deny", "expire", "revoke", "disable", "remove_account", "permission_removed"} {
		t.Run(action, func(t *testing.T) {
			app, writer := newMailHandlerTestApp(t)
			s := testMCPService(t, app, false)
			s.config.Clients[0].Send = true
			session := connectTestMCP(t, s)
			queued := operationResult(t, callMCP(t, session, "send_message", sendArgs()))
			id := queued["operation_id"].(string)
			switch action {
			case "deny":
				resolveMCP(t, app, id, false)
			case "expire":
				s.mu.Lock()
				s.operations[id].ExpiresAt = time.Now().Add(-time.Second)
				s.mu.Unlock()
			case "revoke":
				if _, err := app.mcpSettings("mcp.clientRevoke", map[string]any{"id": "client"}); err != nil {
					t.Fatal(err)
				}
			case "disable":
				if _, err := app.mcpSettings("mcp.enable", map[string]any{"enabled": false}); err != nil {
					t.Fatal(err)
				}
			case "remove_account":
				if err := s.removeAccount("allowed"); err != nil {
					t.Fatal(err)
				}
			case "permission_removed":
				s.mu.Lock()
				s.config.Clients[0].Send = false
				s.mu.Unlock()
			}
			result := resolveMCP(t, app, id, true)
			if result["status"] == "completed" || result["status"] == "pending_approval" || len(writer.calls) != 0 {
				t.Fatalf("did not fail closed: %v", result)
			}
		})
	}
}

func TestMCPWithoutConfirmationStillNeedsPermissionAndDoesNotRetryFailures(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Error: "SMTP response lost"})
	s := testMCPService(t, app, false)
	s.config.Clients[0].Send = true
	s.config.Clients[0].SendWithoutConfirmation = true
	session := connectTestMCP(t, s)
	result := operationResult(t, callMCP(t, session, "send_message", sendArgs()))
	if result["status"] != "failed" || len(writer.calls) != 1 {
		t.Fatal(result)
	}
	operationResult(t, callMCP(t, session, "send_message", sendArgs()))
	if len(writer.calls) != 1 {
		t.Fatal("failed send retried")
	}
	s.mu.Lock()
	s.config.Clients[0].Send = false
	s.mu.Unlock()
	args := sendArgs()
	args["request_id"] = "another"
	result2, err := session.CallTool(context.Background(), &mcp.CallToolParams{Name: "send_message", Arguments: args})
	if err == nil && !result2.IsError {
		t.Fatal("no-confirmation policy granted send permission")
	}
}

func TestMCPDeleteApprovalUsesFrozenServerSnapshot(t *testing.T) {
	snapshot := map[string]any{"account": "allowed", "folder": "Trash", "uidvalidity": float64(42), "uids": []any{float64(7)}, "messages": []any{map[string]any{"uid": float64(7), "subject": "Old message", "from": "sender@example.com"}}}
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: snapshot}, sidecarResponsePlan{Result: map[string]any{"ok": true, "deleted": 1}})
	s := testMCPService(t, app, false)
	s.config.Clients[0].Delete = true
	session := connectTestMCP(t, s)
	result := operationResult(t, callMCP(t, session, "empty_trash", map[string]any{"account_id": "allowed", "folder_id": "Trash", "request_id": "empty-1"}))
	if len(writer.calls) != 1 || writer.calls[0].Method != "mcp.prepareDelete" || writer.calls[0].Params["empty_trash"] != true {
		t.Fatal(writer.calls)
	}
	// A copy sent to the frontend cannot change the captured execution payload.
	pending, _ := app.mcpSettings("mcp.pending", nil)
	selection := pending.([]any)[0].(map[string]any)["preview"].(map[string]any)["selection"].(map[string]any)
	selection["uids"] = []any{float64(99)}
	id := result["operation_id"].(string)
	if resolveMCP(t, app, id, true)["status"] != "completed" {
		t.Fatal("delete failed")
	}
	if len(writer.calls) != 2 || writer.calls[1].Method != "mcp.delete" || !reflect.DeepEqual(writer.calls[1].Params, snapshot) {
		t.Fatalf("selection expanded or changed: %v", writer.calls)
	}
}

func TestMCPOrganizeUsesOnlyConstrainedCorePath(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}})
	s := testMCPService(t, app, false)
	s.config.Clients[0].Organize = true
	session := connectTestMCP(t, s)
	result := callMCP(t, session, "organize_messages", map[string]any{"account_id": "allowed", "folder_id": "Drafts", "uids": []uint32{7}, "action": "trash"})
	if result.IsError || len(writer.calls) != 1 || writer.calls[0].Method != "mcp.organize" || writer.calls[0].Params["action"] != "trash" {
		t.Fatal(writer.calls)
	}
	for _, args := range []map[string]any{
		{"account_id": "excluded", "folder_id": "INBOX", "uids": []uint32{7}, "action": "trash"},
		{"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{}, "action": "trash"},
		{"account_id": "allowed", "folder_id": "INBOX", "uids": []uint32{7}, "action": "delete_permanently"},
	} {
		if !callMCP(t, session, "organize_messages", args).IsError {
			t.Fatal("unsafe organization accepted")
		}
	}
}

func TestMCPOperationStatusIsClientScoped(t *testing.T) {
	app, _ := newMailHandlerTestApp(t)
	s := testMCPService(t, app, false)
	s.config.Clients[0].Send = true
	session := connectTestMCP(t, s)
	result := operationResult(t, callMCP(t, session, "send_message", sendArgs()))
	id := result["operation_id"].(string)
	s.mu.Lock()
	s.operations[id].client.ID = "another-client"
	s.mu.Unlock()
	if !callMCP(t, session, "operation_status", map[string]any{"operation_id": id}).IsError {
		t.Fatal("other client's operation disclosed")
	}
}
