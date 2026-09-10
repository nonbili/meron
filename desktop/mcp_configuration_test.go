package main

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestMCPConfigurationPermissionAndSignatureLimit(t *testing.T) {
	if mcpPermitted(mcpClient{}, mcpReadConfiguration) {
		t.Fatal("configuration read granted without permission")
	}
	for _, c := range []mcpClient{{ManageAccounts: true}, {ManageSettings: true}} {
		if !mcpPermitted(c, mcpReadConfiguration) {
			t.Fatal("configuration grant denied")
		}
	}
	for _, specs := range []map[string]mcpSettingSpec{mcpAppSettings, mcpAccountSettings} {
		var value any = strings.Repeat("é", 128*1024+1)
		if specs["signature"].Type == "signature" {
			value = map[string]any{"mode": "custom", "html": value}
		}
		if mcpValidateSetting(specs, "signature", value) == nil {
			t.Fatal("oversize signature accepted")
		}
	}
	if err := mcpValidateSetting(mcpAppSettings, "signature", strings.Repeat("x", 256*1024)); err != nil {
		t.Fatal(err)
	}
}

func TestMCPSignaturesSanitizedBeforePersistence(t *testing.T) {
	dirty := `<p>Hello <strong>world</strong><script>alert(1)</script><img src="https://example.com/a.png" onerror="alert(2)"><a href="jav&#x61;script:alert(3)">bad</a><a href="https://example.com">safe</a><svg onload="alert(4)"></svg></p>`
	for _, account := range []bool{false, true} {
		t.Run(fmt.Sprint(account), func(t *testing.T) {
			plans := []sidecarResponsePlan{}
			if account {
				plans = append(plans, sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "a"}}}})
			}
			plans = append(plans, sidecarResponsePlan{Result: map[string]any{"ok": true}})
			app, writer := newMailHandlerTestApp(t, plans...)
			s := testMCPService(t, app, false)
			s.config.Clients[0].ManageAccounts, s.config.Clients[0].ManageSettings = true, true
			tool, field := "set_app_setting", "value"
			args := map[string]any{"key": "signature", "value": dirty}
			if account {
				tool, field = "set_account_setting", "signature"
				args["account_id"] = "a"
				args["value"] = map[string]any{"mode": "custom", "html": dirty}
			}
			session := connectTestMCP(t, s)
			result := callMCP(t, session, tool, args)
			if !result.IsError || len(writer.calls) != 0 {
				t.Fatal("unsafe signature was not rejected before persistence")
			}
			for _, unsupported := range []string{`<img src="data:image/png;base64,aGVsbG8=" alt="logo">`, `<div style="color:red">Name</div>`} {
				args["value"] = unsupported
				if account {
					args["value"] = map[string]any{"mode": "custom", "html": unsupported}
				}
				if !callMCP(t, session, tool, args).IsError || len(writer.calls) != 0 {
					t.Fatal("lossy signature was accepted")
				}
			}
			safe := `<p>Hello <strong>world</strong> <a href="https://example.com">safe</a></p>`
			args["value"] = safe
			if account {
				args["value"] = map[string]any{"mode": "custom", "html": safe}
			}
			result = callMCP(t, session, tool, args)
			if result.IsError {
				t.Fatal(result)
			}
			value := writer.calls[len(writer.calls)-1].Params[field]
			if account {
				value = value.(map[string]any)["html"]
			}
			html := value.(string)
			if html != safe {
				t.Fatalf("signature changed: %s", html)
			}
			for _, forbidden := range []string{"<script", "onerror", "onload", "javascript:", "<svg"} {
				if strings.Contains(html, forbidden) {
					t.Fatal(html)
				}
			}
			if !strings.Contains(html, "<strong>world</strong>") || !strings.Contains(html, `href="https://example.com"`) {
				t.Fatal(html)
			}
		})
	}
	if mcpValidateSetting(mcpAppSettings, "auto_update_check", false) == nil {
		t.Fatal("update checks exposed")
	}
}

func TestMCPAccountSetupSecuritySchema(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	s.config.Clients[0].ManageAccounts = true
	session := connectTestMCP(t, s)
	tools, err := session.ListTools(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, tool := range tools.Tools {
		if tool.Name != "create_password_account" {
			continue
		}
		found = true
		if tool.Annotations.DestructiveHint == nil || !*tool.Annotations.DestructiveHint {
			t.Fatal("account creation not marked destructive")
		}
		data, _ := json.Marshal(tool.InputSchema)
		if strings.Contains(string(data), "cert_pin") {
			t.Fatalf("pin exposed: %s", data)
		}
	}
	if !found {
		t.Fatal("missing setup tool")
	}
	yes, no := true, false
	for _, args := range []mcpCreateAccountArgs{{TLS: &no}, {SMTPTLS: &no, SMTPStartTLS: &no}, {TLS: &no, StartTLS: &no, SMTPTLS: &yes}} {
		if args.validateTLS() == nil {
			t.Fatal("plaintext accepted")
		}
	}
	for _, args := range []mcpCreateAccountArgs{{}, {TLS: &no, StartTLS: &yes, SMTPTLS: &no, SMTPStartTLS: &yes, IMAPPort: 143, SMTPPort: 587}} {
		if err := args.validateTLS(); err != nil {
			t.Fatal(err)
		}
	}
}

func TestMCPAccountSetupDoesNotBlockControlPlane(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	s.config.Clients[0].ManageAccounts = true
	client := s.config.Clients[0]
	server := mcp.NewServer(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	started, release := make(chan struct{}), make(chan struct{})
	mcpRegister(s, server, client, "blocked_setup", "test", mcpCreateAccount, func(struct{}) string { return "" }, func(struct{}, mcpClient) (any, error) {
		close(started)
		<-release
		return map[string]any{"ok": true}, nil
	})
	st, ct := mcp.NewInMemoryTransports()
	ss, err := server.Connect(context.Background(), st, nil)
	if err != nil {
		t.Fatal(err)
	}
	cs, err := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil).Connect(context.Background(), ct, nil)
	if err != nil {
		close(release)
		ss.Close()
		t.Fatal(err)
	}
	defer func() { close(release); cs.Close(); ss.Close() }()
	go cs.CallTool(context.Background(), &mcp.CallToolParams{Name: "blocked_setup", Arguments: map[string]any{}})
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("setup did not start")
	}
	done := make(chan error, 1)
	go func() {
		_, err := s.app.mcpSettings("mcp.status", nil)
		if err == nil {
			_, err = s.app.mcpSettings("mcp.clientRevoke", map[string]any{"id": client.ID})
		}
		done <- err
	}()
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("setup blocked status/revocation")
	}
}

func TestMCPConfigurationPermissionsAndSecrets(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"accounts": []any{
		map[string]any{"id": "excluded", "imap_host": "imap.example.com", "access_token": "secret-access", "refresh_token": "secret-refresh", "proxy": map[string]any{"password": "secret-proxy"}},
	}}})
	s := testMCPService(t, app, false)
	s.config.Clients[0].ManageAccounts = true
	s.config.Clients[0].Accounts = nil
	session := connectTestMCP(t, s)
	result := callMCP(t, session, "list_account_configurations", map[string]any{})
	data, _ := json.Marshal(result)
	if result.IsError || !strings.Contains(string(data), "imap.example.com") || strings.Contains(string(data), "secret-") {
		t.Fatalf("%s", data)
	}
	if !callMCP(t, session, "search_messages", map[string]any{"account_id": "excluded"}).IsError {
		t.Fatal("configuration grant allowed mail read")
	}
	if !callMCP(t, session, "set_account_setting", map[string]any{"account_id": "excluded", "key": "muted", "value": "yes"}).IsError {
		t.Fatal("invalid boolean accepted")
	}
	if len(writer.calls) != 1 {
		t.Fatal(writer.calls)
	}
	s.mu.Lock()
	s.config.Clients[0].ManageAccounts = false
	s.mu.Unlock()
	result, err := session.CallTool(context.Background(), &mcp.CallToolParams{Name: "list_account_configurations", Arguments: map[string]any{}})
	if err == nil && !result.IsError {
		t.Fatal("revoked configuration permission accepted")
	}
}

func TestMCPSettingsAllowlistAndDispatch(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}})
	s := testMCPService(t, app, false)
	s.config.Clients[0].ManageSettings = true
	session := connectTestMCP(t, s)
	for _, args := range []map[string]any{
		{"key": "mcp", "value": true}, {"key": "proxy", "value": map[string]any{}},
		{"key": "send_shortcut", "value": "invalid"}, {"key": "spell_check", "value": "false"},
	} {
		if !callMCP(t, session, "set_app_setting", args).IsError {
			t.Fatal(args)
		}
	}
	if result := callMCP(t, session, "set_app_setting", map[string]any{"key": "spell_check", "value": false}); result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 1 || writer.calls[0].Method != "app.prefsSet" || writer.calls[0].Params["value"] != false {
		t.Fatal(writer.calls)
	}
	tools, err := session.ListTools(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, tool := range tools.Tools {
		if tool.Name == "create_password_account" {
			t.Fatal("settings grant exposes account creation")
		}
	}
}

func TestMCPCreateRejectsExistingAccount(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": accountID("a@example.com")}}}})
	s := testMCPService(t, app, false)
	s.config.Clients[0].ManageAccounts = true
	args := mcpCreateAccountArgs{Email: "a@example.com", Username: "a", Password: "secret", IMAPHost: "imap.example.com", SMTPHost: "smtp.example.com"}
	result := callMCP(t, connectTestMCP(t, s), "create_password_account", args)
	if !result.IsError || len(writer.calls) != 1 {
		t.Fatalf("%v %v", result, writer.calls)
	}
}

func TestMCPSetupOnlyClient(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	if _, err := s.app.mcpSettings("mcp.clientSave", map[string]any{"name": "Setup", "accounts": []any{}, "manage_accounts": true}); err != nil {
		t.Fatal(err)
	}
	if _, err := s.app.mcpSettings("mcp.clientSave", map[string]any{"name": "Empty", "accounts": []any{}}); err == nil {
		t.Fatal("empty grant accepted")
	}
}

func TestMCPCreateAccountDoesNotGrantMail(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{}}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)
	s := testMCPService(t, app, false)
	s.config.Clients[0].ManageAccounts = true
	s.config.Clients[0].Accounts = nil
	args := mcpCreateAccountArgs{Email: "new@example.com", Username: "new", Password: "private-password", IMAPHost: "imap.example.com", SMTPHost: "smtp.example.com"}
	result := callMCP(t, connectTestMCP(t, s), "create_password_account", args)
	data, _ := json.Marshal(result)
	if result.IsError || strings.Contains(string(data), args.Password) {
		t.Fatalf("%s", data)
	}
	if len(writer.calls) != 3 || writer.calls[1].Method != "account.connect" || writer.calls[1].Params["password"] != args.Password {
		t.Fatal(writer.calls)
	}
	if len(s.config.Clients[0].Accounts) != 0 {
		t.Fatal("new account granted mail access")
	}
}

func TestMCPAccountSettingPreservesFalse(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "other"}}}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)
	s := testMCPService(t, app, false)
	s.config.Clients[0].ManageAccounts = true
	result := callMCP(t, connectTestMCP(t, s), "set_account_setting", map[string]any{"account_id": "other", "key": "included_in_unified", "value": false})
	if result.IsError || len(writer.calls) != 2 || writer.calls[1].Method != "account.setUnified" || writer.calls[1].Params["enabled"] != false || writer.calls[1].Params["account"] != "other" {
		t.Fatalf("%v %v", result, writer.calls)
	}
}

func TestMCPDisableSignaturePreservesHTMLAndAuditsAccount(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "other", "signature": map[string]any{"mode": "custom", "html": "<p>Saved signature</p>"}}}}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)
	s := testMCPService(t, app, false)
	s.config.Clients[0].ManageAccounts = true
	result := callMCP(t, connectTestMCP(t, s), "set_account_setting", map[string]any{"account_id": "other", "key": "signature", "value": map[string]any{"mode": "none"}})
	if result.IsError {
		t.Fatal(result)
	}
	signature := writer.calls[1].Params["signature"].(map[string]any)
	if signature["mode"] != "none" || signature["html"] != "<p>Saved signature</p>" {
		t.Fatal(signature)
	}
	s.activityMu.Lock()
	defer s.activityMu.Unlock()
	if len(s.activity) != 1 || s.activity[0].Account != "other" {
		t.Fatal(s.activity)
	}
}
