package main

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"
)

func testMCPService(t *testing.T, app *App, drafts bool) *mcpService {
	t.Helper()
	hash := sha256.Sum256([]byte("test-secret"))
	s := &mcpService{app: app, path: filepath.Join(t.TempDir(), "mcp.json"), config: mcpConfig{Enabled: true, Clients: []mcpClient{{ID: "client", Name: "Assistant", Accounts: []string{"allowed"}, Drafts: drafts, TokenHash: hex.EncodeToString(hash[:])}}}}
	app.mcp = s
	return s
}

type mcpTestTransport struct{ base http.RoundTripper }

func (t mcpTestTransport) RoundTrip(r *http.Request) (*http.Response, error) {
	r = r.Clone(r.Context())
	r.Host = mcpAddress
	r.Header.Set("Authorization", "Bearer test-secret")
	return t.base.RoundTrip(r)
}
func connectTestMCP(t *testing.T, s *mcpService) *mcp.ClientSession {
	t.Helper()
	host := httptest.NewServer(s.handler())
	t.Cleanup(host.Close)
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	t.Cleanup(cancel)
	session, err := client.Connect(ctx, &mcp.StreamableClientTransport{Endpoint: host.URL + "/mcp", HTTPClient: &http.Client{Transport: mcpTestTransport{http.DefaultTransport}}, DisableStandaloneSSE: true}, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })
	return session
}
func callMCP(t *testing.T, session *mcp.ClientSession, name string, args any) *mcp.CallToolResult {
	t.Helper()
	result, err := session.CallTool(context.Background(), &mcp.CallToolParams{Name: name, Arguments: args})
	if err != nil {
		t.Fatal(err)
	}
	return result
}
func TestMCPRejectsUntrustedHTTP(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	for _, tc := range []struct {
		name, host, origin, auth, path string
		want                           int
	}{
		{"no credential", mcpAddress, "", "", "/mcp", 401},
		{"bad credential", mcpAddress, "", "Bearer bad", "/mcp", 401},
		{"wrong scheme", mcpAddress, "", "test-secret", "/mcp", 401},
		{"rebinding", "evil.example", "", "Bearer test-secret", "/mcp", 403},
		{"website", mcpAddress, "https://evil.example", "Bearer test-secret", "/mcp", 403},
		{"loopback website", mcpAddress, "http://127.0.0.1:3000", "Bearer test-secret", "/mcp", 403},
		{"query credential", mcpAddress, "", "", "/mcp?token=test-secret", 404},
	} {
		t.Run(tc.name, func(t *testing.T) {
			req := httptest.NewRequest("POST", "http://"+tc.host+tc.path, strings.NewReader(`{}`))
			req.Header.Set("Authorization", tc.auth)
			if tc.origin != "" {
				req.Header.Set("Origin", tc.origin)
			}
			rec := httptest.NewRecorder()
			s.handler().ServeHTTP(rec, req)
			if rec.Code != tc.want {
				t.Fatalf("status=%d body=%s", rec.Code, rec.Body.String())
			}
		})
	}
}
func TestMCPAcceptsClientsWithoutEventStreamAccept(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	req := httptest.NewRequest("POST", "http://"+mcpAddress+"/mcp", strings.NewReader(`{"jsonrpc":"2.0","method":"notifications/roots/list_changed"}`))
	req.Header.Set("Authorization", "Bearer test-secret")
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	rec := httptest.NewRecorder()
	s.handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusAccepted {
		t.Fatalf("status=%d body=%s", rec.Code, rec.Body.String())
	}
}
func TestMCPAccountBoundaryAndReadOnlyTools(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	session := connectTestMCP(t, s)
	list, err := session.ListTools(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, tool := range list.Tools {
		if tool.Name == "create_draft" || tool.Name == "send" || tool.Name == "delete" {
			t.Fatalf("unexpected tool %s", tool.Name)
		}
	}
	if len(list.Tools) != 6 {
		t.Fatalf("tools=%v", list.Tools)
	}
	for _, tc := range []struct {
		name string
		args any
	}{
		{"search_messages", map[string]any{"account_id": "excluded"}},
		{"search_messages", map[string]any{"account_id": "unified"}},
		{"search_messages", map[string]any{"filter": "important"}},
		{"search_messages", map[string]any{"folder_id": "INBOX"}},
		{"search_messages", map[string]any{"before_cursor": "page2"}},
		{"list_folders", map[string]any{"account_id": "excluded"}},
		{"read_thread", map[string]any{"thread_id": formatImapThreadID("excluded", "INBOX", "key")}},
		{"read_thread", map[string]any{"thread_id": "excluded#rss#feed"}},
		{"read_thread", map[string]any{"thread_id": "garbage"}},
	} {
		if !callMCP(t, session, tc.name, tc.args).IsError {
			t.Fatalf("allowed %s %#v", tc.name, tc.args)
		}
	}
}
func TestMCPListsOnlyGrantedAccountMetadata(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"accounts": []any{
		map[string]any{"id": "allowed", "email": "allowed@example.com", "imap_host": "private-host", "password": "never-return"},
		map[string]any{"id": "excluded", "email": "excluded@example.com"},
	}}})
	session := connectTestMCP(t, testMCPService(t, app, false))
	result := callMCP(t, session, "list_accounts", map[string]any{})
	data, _ := json.Marshal(result)
	if result.IsError || !strings.Contains(string(data), "allowed@example.com") || strings.Contains(string(data), "excluded") || strings.Contains(string(data), "private-host") || strings.Contains(string(data), "never-return") {
		t.Fatalf("result=%s", data)
	}
	if len(writer.calls) != 1 || writer.calls[0].Method != "account.list" {
		t.Fatal(writer.calls)
	}
}

func TestMCPAllAccountsIncludesFutureAccountsAndCanBeNarrowed(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "allowed"}}}},
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "allowed"}, map[string]any{"id": "new-account"}}}},
	)
	s := testMCPService(t, app, false)
	s.config.Clients[0].AllAccounts = true
	session := connectTestMCP(t, s)
	for _, wantNew := range []bool{false, true} {
		result := callMCP(t, session, "list_accounts", map[string]any{})
		data, _ := json.Marshal(result)
		if result.IsError || strings.Contains(string(data), "new-account") != wantNew {
			t.Fatalf("%s", data)
		}
	}
	s.mu.Lock()
	s.config.Clients[0].AllAccounts = false
	s.mu.Unlock()
	if !callMCP(t, session, "list_folders", map[string]any{"account_id": "new-account"}).IsError {
		t.Fatal("narrowed grant still allowed new account")
	}
	if len(writer.calls) != 2 {
		t.Fatal(writer.calls)
	}
}

func TestMCPAllAccountScopeDoesNotWidenPermissions(t *testing.T) {
	c := mcpClient{AllAccounts: true}
	for _, id := range []string{"future-mail", "future-feed"} {
		if !mcpAllows(c, id) {
			t.Fatalf("denied %s", id)
		}
	}
	for _, id := range []string{"", "unified"} {
		if mcpAllows(c, id) {
			t.Fatalf("allowed synthetic account %q", id)
		}
	}
	for _, permission := range []mcpPermission{mcpDraft, mcpOrganize, mcpSend, mcpDelete, mcpManageAccounts, mcpManageSettings} {
		if mcpPermitted(c, permission) {
			t.Fatalf("enabled %s", permission)
		}
	}
	var old mcpClient
	if err := json.Unmarshal([]byte(`{"accounts":["selected"]}`), &old); err != nil {
		t.Fatal(err)
	}
	if old.AllAccounts || mcpAllows(old, "future-mail") {
		t.Fatal("legacy grant widened")
	}
}

func TestMCPAllAccountsGrantPersistsWithoutAccounts(t *testing.T) {
	s := testMCPService(t, &App{}, false)
	_, err := s.app.mcpSettings("mcp.clientSave", map[string]any{"name": "All", "accounts": []any{}, "all_accounts": true})
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	var cfg mcpConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		t.Fatal(err)
	}
	if len(cfg.Clients) != 2 || !cfg.Clients[1].AllAccounts {
		t.Fatalf("%s", data)
	}
	if err := s.removeAccount("future-mail"); err != nil {
		t.Fatal(err)
	}
	if !mcpAllows(s.config.Clients[1], "future-mail") {
		t.Fatal("removal disabled dynamic scope")
	}
}
func TestMCPUnscopedSearchMergesOnlyApprovedAccounts(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{
			map[string]any{"id": "allowed", "email": "allowed@example.com"},
			map[string]any{"id": "excluded", "email": "excluded@example.com"},
		}}},
		sidecarResponsePlan{Result: map[string]any{"cards": []any{
			map[string]any{"thread_key": "old", "subject": "older", "date": float64(100)},
			map[string]any{"thread_key": "new", "subject": "newer", "date": float64(200)},
		}}})
	session := connectTestMCP(t, testMCPService(t, app, false))
	result := callMCP(t, session, "search_messages", map[string]any{"filter": "unread"})
	if result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 2 || writer.calls[0].Method != "account.list" {
		t.Fatal(writer.calls)
	}
	search := writer.calls[1]
	if search.Method != "messages.recent" || search.Params["account"] != "allowed" || search.Params["filter"] != "unread" {
		t.Fatal(search)
	}
	data, _ := json.Marshal(result)
	if strings.Contains(string(data), "excluded") {
		t.Fatalf("result=%s", data)
	}
	// The merged page is newest first, whichever account a conversation came from.
	var page struct {
		Results struct{ Threads []Message } `json:"results"`
	}
	encoded, _ := json.Marshal(result.StructuredContent)
	if err := json.Unmarshal(encoded, &page); err != nil {
		t.Fatal(err)
	}
	if len(page.Results.Threads) != 2 || page.Results.Threads[0].Subject != "newer" {
		t.Fatalf("threads=%#v", page.Results.Threads)
	}
}
func TestMCPSearchPassesFilterForOneAccount(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"cards": []any{}}})
	session := connectTestMCP(t, testMCPService(t, app, false))
	if result := callMCP(t, session, "search_messages", map[string]any{"account_id": "allowed", "filter": "starred"}); result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 1 || writer.calls[0].Params["filter"] != "starred" {
		t.Fatal(writer.calls)
	}
}
func TestMCPDraftKeepsReplyThreading(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}})
	session := connectTestMCP(t, testMCPService(t, app, true))
	result := callMCP(t, session, "create_draft", map[string]any{"account_id": "allowed", "to": "sender@example.com",
		"subject": "Re: hello", "body": "reply", "in_reply_to": "<a@example.com>", "references": "<root@example.com> <a@example.com>"})
	if result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 1 || writer.calls[0].Params["in_reply_to"] != "<a@example.com>" ||
		writer.calls[0].Params["references"] != "<root@example.com> <a@example.com>" {
		t.Fatal(writer.calls)
	}
	for _, args := range []map[string]any{
		{"account_id": "allowed", "body": "reply", "in_reply_to": "not-a-message-id"},
		{"account_id": "allowed", "body": "reply", "references": "<a@example.com>"},
		{"account_id": "allowed", "body": "reply", "in_reply_to": "<a@example.com>", "references": "garbage"},
	} {
		if !callMCP(t, session, "create_draft", args).IsError {
			t.Fatalf("allowed %#v", args)
		}
	}
}
func TestMCPReadDoesNotMarkRead(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"messages": []any{}}})
	session := connectTestMCP(t, testMCPService(t, app, false))
	id := formatImapThreadID("allowed", "INBOX", "key")
	result := callMCP(t, session, "read_thread", map[string]any{"thread_id": id})
	if result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 1 || writer.calls[0].Method != "messages.thread" || writer.calls[0].Params["account"] != "allowed" || writer.calls[0].Params["limit"] != float64(50) {
		t.Fatal(writer.calls)
	}
}
func TestMCPDraftIsNewAndNeverSends(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}}, sidecarResponsePlan{Result: map[string]any{"ok": true}})
	session := connectTestMCP(t, testMCPService(t, app, true))
	for i := 0; i < 2; i++ {
		result := callMCP(t, session, "create_draft", map[string]any{"account_id": "allowed", "body": "Please review", "to": "reader@example.com"})
		if result.IsError {
			t.Fatal(result)
		}
	}
	if len(writer.calls) != 2 {
		t.Fatal(writer.calls)
	}
	for _, call := range writer.calls {
		if call.Method != "save_draft" || call.Params["account"] != "allowed" || call.Params["from"] != "" || call.Params["attachments"] != nil {
			t.Fatal(call)
		}
	}
	if writer.calls[0].Params["draft_id"] == writer.calls[1].Params["draft_id"] {
		t.Fatal("draft ID reused")
	}
	if !callMCP(t, session, "create_draft", map[string]any{"account_id": "allowed", "body": "body", "subject": "subject\r\nBcc: victim@example.com"}).IsError {
		t.Fatal("accepted header injection")
	}
	if !callMCP(t, session, "create_draft", map[string]any{"account_id": "allowed", "body": "body", "attachments": []any{map[string]any{"path": "/etc/passwd"}}}).IsError {
		t.Fatal("accepted unknown attachment input")
	}
}
func TestMCPGrantChangesAndRevocation(t *testing.T) {
	app := &App{}
	s := testMCPService(t, app, true)
	session := connectTestMCP(t, s)
	s.mu.Lock()
	s.config.Clients[0].Accounts = []string{"other"}
	s.config.Clients[0].Drafts = false
	s.mu.Unlock()
	if !callMCP(t, session, "list_folders", map[string]any{"account_id": "allowed"}).IsError {
		t.Fatal("stale account access")
	}
	tools, err := session.ListTools(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, tool := range tools.Tools {
		if tool.Name == "create_draft" {
			t.Fatal("stale draft tool")
		}
	}
	if _, err := app.mcpSettings("mcp.clientRevoke", map[string]any{"id": "client"}); err != nil {
		t.Fatal(err)
	}
	if _, err := session.ListTools(context.Background(), nil); err == nil {
		t.Fatal("revoked credential accepted")
	}
}
func TestMCPSettingsPersistOnlyHashAndStripRemovedAccounts(t *testing.T) {
	app, _ := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "allowed"}}}})
	s := testMCPService(t, app, false)
	result, err := app.mcpSettings("mcp.clientSave", map[string]any{"name": "New assistant", "accounts": []string{"allowed"}})
	if err != nil {
		t.Fatal(err)
	}
	token := result.(map[string]any)["token"].(string)
	if len(token) != 64 {
		t.Fatalf("token length=%d", len(token))
	}
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(data), token) {
		t.Fatal("plaintext secret persisted")
	}
	var persisted mcpConfig
	if err := json.Unmarshal(data, &persisted); err != nil {
		t.Fatal(err)
	}
	if len(persisted.Clients) != 2 || persisted.Clients[1].TokenHash == "" {
		t.Fatal(persisted)
	}
	status, _ := json.Marshal(s.status())
	if strings.Contains(string(status), "token_hash") {
		t.Fatal("status exposed hash")
	}
	if err := s.removeAccount("allowed"); err != nil {
		t.Fatal(err)
	}
	for _, client := range s.config.Clients {
		if len(client.Accounts) != 0 {
			t.Fatal("removed account remains granted")
		}
	}
}

func TestMCPSearchScopesAndSources(t *testing.T) {
	for _, online := range []bool{false, true} {
		t.Run(map[bool]string{false: "cache", true: "server"}[online], func(t *testing.T) {
			app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"messages": []any{}}})
			session := connectTestMCP(t, testMCPService(t, app, false))
			result := callMCP(t, session, "search_messages", map[string]any{"account_id": "allowed", "folder_id": "Archive", "query": "invoice", "server_search": online})
			if result.IsError {
				t.Fatal(result)
			}
			if len(writer.calls) != 1 || writer.calls[0].Method != "messages.recent" || writer.calls[0].Params["account"] != "allowed" || writer.calls[0].Params["folder"] != "Archive" || writer.calls[0].Params["refresh"] != online {
				t.Fatal(writer.calls)
			}
			encoded, _ := json.Marshal(result)
			expected := map[bool]string{false: "local_cache", true: "server_search_or_snapshot"}[online]
			if !strings.Contains(string(encoded), expected) {
				t.Fatalf("source missing: %s", encoded)
			}
		})
	}
}

func TestMCPDisabledRejectsExistingCredential(t *testing.T) {
	app := &App{}
	s := testMCPService(t, app, false)
	session := connectTestMCP(t, s)
	if _, err := app.mcpSettings("mcp.enable", map[string]any{"enabled": false}); err != nil {
		t.Fatal(err)
	}
	if _, err := session.ListTools(context.Background(), nil); err == nil {
		t.Fatal("disabled MCP accepted a request")
	}
	if s.config.Enabled {
		t.Fatal("disable was not persisted")
	}
}

func TestMCPAttachmentStaysInsideApprovedAccount(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	write := func(key string, size int) {
		t.Helper()
		path := filepath.Join(mediaDir(), filepath.FromSlash(key))
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(strings.Repeat("a", size)), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	write("allowed/INBOX/7/0.png", 3)
	write("excluded/INBOX/7/0.png", 3)
	write("allowed/INBOX/7/1.bin", mcpAttachmentMaxBytes+1)
	// Every read that clears the key prefix asks the core which accounts exist,
	// so the collision check has a live answer to work from.
	accounts := sidecarResponsePlan{Result: map[string]any{"accounts": []any{
		map[string]any{"id": "allowed", "email": "allowed@example.com"},
		map[string]any{"id": "excluded", "email": "excluded@example.com"},
	}}}
	app, _ := newMailHandlerTestApp(t, accounts, accounts, accounts)
	session := connectTestMCP(t, testMCPService(t, app, false))

	result := callMCP(t, session, "read_attachment", map[string]any{"account_id": "allowed", "key": "allowed/INBOX/7/0.png"})
	encoded, _ := json.Marshal(result)
	if result.IsError || !strings.Contains(string(encoded), base64.StdEncoding.EncodeToString([]byte("aaa"))) || !strings.Contains(string(encoded), "image/png") {
		t.Fatalf("result=%s", encoded)
	}
	for _, tc := range []struct {
		name string
		args map[string]any
	}{
		{"other account", map[string]any{"account_id": "allowed", "key": "excluded/INBOX/7/0.png"}},
		{"unapproved account", map[string]any{"account_id": "excluded", "key": "excluded/INBOX/7/0.png"}},
		{"traversal", map[string]any{"account_id": "allowed", "key": "allowed/../excluded/INBOX/7/0.png"}},
		{"absolute", map[string]any{"account_id": "allowed", "key": "/etc/passwd"}},
		{"bare key", map[string]any{"account_id": "allowed", "key": "allowed"}},
		{"not cached", map[string]any{"account_id": "allowed", "key": "allowed/INBOX/9/0.png"}},
		{"too large", map[string]any{"account_id": "allowed", "key": "allowed/INBOX/7/1.bin"}},
	} {
		if !callMCP(t, session, "read_attachment", tc.args).IsError {
			t.Fatalf("allowed %s", tc.name)
		}
	}
}

// Two account IDs can fold to one media segment ("a+b@" and "a_b@" both become
// "a_b_example.com"), so the key prefix stops proving ownership and the read is
// refused rather than serving the other account's cache.
func TestMCPAttachmentRefusesSharedMediaSegment(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	path := filepath.Join(mediaDir(), filepath.FromSlash("a_b_example.com/INBOX/7/0.png"))
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("aaa"), 0o600); err != nil {
		t.Fatal(err)
	}
	app, _ := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"accounts": []any{
		map[string]any{"id": "a+b@example.com", "email": "a+b@example.com"},
		map[string]any{"id": "a_b@example.com", "email": "a_b@example.com"},
	}}})
	service := testMCPService(t, app, false)
	service.config.Clients[0].Accounts = []string{"a+b@example.com"}
	session := connectTestMCP(t, service)
	result := callMCP(t, session, "read_attachment", map[string]any{"account_id": "a+b@example.com", "key": "a_b_example.com/INBOX/7/0.png"})
	encoded, _ := json.Marshal(result)
	if !result.IsError || strings.Contains(string(encoded), base64.StdEncoding.EncodeToString([]byte("aaa"))) {
		t.Fatalf("result=%s", encoded)
	}
}

// accountList reports no accounts when the core is down, and no accounts looks
// exactly like no collision. The read has to be refused instead, or a stopped
// core reopens the shared-prefix hole against attachments still on disk.
func TestMCPAttachmentRefusesWhenCoreIsUnavailable(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	path := filepath.Join(mediaDir(), filepath.FromSlash("a_b_example.com/INBOX/7/0.png"))
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("aaa"), 0o600); err != nil {
		t.Fatal(err)
	}
	service := testMCPService(t, &App{}, false)
	service.config.Clients[0].Accounts = []string{"a+b@example.com"}
	session := connectTestMCP(t, service)
	result := callMCP(t, session, "read_attachment", map[string]any{"account_id": "a+b@example.com", "key": "a_b_example.com/INBOX/7/0.png"})
	encoded, _ := json.Marshal(result)
	if !result.IsError || strings.Contains(string(encoded), base64.StdEncoding.EncodeToString([]byte("aaa"))) {
		t.Fatalf("result=%s", encoded)
	}
}

// The merge keeps only the newest 50 across accounts, so an account whose older
// results were dropped gets no cursor: following one would page past mail this
// response never carried.
func TestMCPUnscopedSearchWithholdsCursorsForTruncatedAccounts(t *testing.T) {
	cards := func(base int64) []any {
		out := []any{}
		for i := 0; i < mcpSearchLimit; i++ {
			out = append(out, map[string]any{"thread_key": fmt.Sprintf("t%d-%d", base, i), "subject": "s", "date": float64(base - int64(i))})
		}
		return out
	}
	app, _ := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"accounts": []any{
			map[string]any{"id": "allowed", "email": "allowed@example.com"},
			map[string]any{"id": "second", "email": "second@example.com"},
		}}},
		sidecarResponsePlan{Result: map[string]any{"cards": cards(100000), "next_cursor": "cursor-allowed"}},
		sidecarResponsePlan{Result: map[string]any{"cards": cards(200), "next_cursor": "cursor-second"}})
	service := testMCPService(t, app, false)
	service.config.Clients[0].Accounts = []string{"allowed", "second"}
	session := connectTestMCP(t, service)
	result := callMCP(t, session, "search_messages", nil)
	if result.IsError {
		t.Fatal(result)
	}
	var page struct {
		Accounts []struct {
			AccountID  string `json:"account_id"`
			Returned   int    `json:"returned"`
			NextCursor string `json:"next_cursor"`
			Truncated  bool   `json:"truncated_by_merge"`
		} `json:"accounts"`
		Results struct{ Threads []Message } `json:"results"`
	}
	encoded, _ := json.Marshal(result.StructuredContent)
	if err := json.Unmarshal(encoded, &page); err != nil {
		t.Fatal(err)
	}
	if len(page.Results.Threads) != mcpSearchLimit || len(page.Accounts) != 2 {
		t.Fatalf("page=%s", encoded)
	}
	// "allowed" owns the whole merged page and pages on with its own cursor;
	// "second" contributed nothing, so its cursor must not be handed out.
	if page.Accounts[0].Returned != mcpSearchLimit || page.Accounts[0].NextCursor != "cursor-allowed" || page.Accounts[0].Truncated {
		t.Fatalf("allowed=%#v", page.Accounts[0])
	}
	if page.Accounts[1].Returned != 0 || page.Accounts[1].NextCursor != "" || !page.Accounts[1].Truncated {
		t.Fatalf("second=%#v", page.Accounts[1])
	}
}

func TestMCPUpdateDraftReplacesDraftInPlace(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}}, sidecarResponsePlan{Result: map[string]any{"ok": true}}, sidecarResponsePlan{Result: map[string]any{"ok": true}})
	session := connectTestMCP(t, testMCPService(t, app, true))
	created := callMCP(t, session, "create_draft", map[string]any{"account_id": "allowed", "body": "first"})
	if created.IsError {
		t.Fatal(created)
	}
	draftID, _ := writer.calls[0].Params["draft_id"].(string)
	if draftID == "" {
		t.Fatal(writer.calls)
	}
	if result := callMCP(t, session, "update_draft", map[string]any{"account_id": "allowed", "draft_id": draftID, "body": "second", "subject": "Revised"}); result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 2 {
		t.Fatal(writer.calls)
	}
	update := writer.calls[1]
	if update.Method != "save_draft" || update.Params["draft_id"] != draftID || update.Params["body"] != "second" || update.Params["subject"] != "Revised" {
		t.Fatal(update)
	}
	// A draft the user started in Meron carries its own Message-ID, which
	// read_thread reports; revising it is the point of the tool.
	if result := callMCP(t, session, "update_draft", map[string]any{"account_id": "allowed", "draft_id": "<compose-1@example.com>", "body": "revised"}); result.IsError {
		t.Fatal(result)
	}
	if len(writer.calls) != 3 || writer.calls[2].Params["draft_id"] != "<compose-1@example.com>" {
		t.Fatal(writer.calls)
	}
	for _, tc := range []struct {
		name string
		args map[string]any
	}{
		{"malformed draft_id", map[string]any{"account_id": "allowed", "draft_id": "compose-1", "body": "body"}},
		{"header injection in draft_id", map[string]any{"account_id": "allowed", "draft_id": "<a@b>\r\nBcc: victim@example.com", "body": "body"}},
		{"missing draft_id", map[string]any{"account_id": "allowed", "body": "body"}},
		{"other account", map[string]any{"account_id": "excluded", "draft_id": draftID, "body": "body"}},
		{"header injection", map[string]any{"account_id": "allowed", "draft_id": draftID, "body": "body", "to": "a@example.com\r\nBcc: victim@example.com"}},
	} {
		if !callMCP(t, session, "update_draft", tc.args).IsError {
			t.Fatalf("allowed %s", tc.name)
		}
	}
}

func TestMCPPortIsSettableAndKeepsWorkingListenerOnConflict(t *testing.T) {
	app := &App{}
	s := testMCPService(t, app, false)
	if err := s.listen(); err != nil {
		t.Skipf("default port unavailable: %v", err)
	}
	t.Cleanup(s.close)
	if s.status().(map[string]any)["port"] != mcpDefaultPort {
		t.Fatal(s.status())
	}

	// A port another process already holds must be refused, leaving the server
	// listening where it was.
	busy, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer busy.Close()
	taken := busy.Addr().(*net.TCPAddr).Port
	if _, err := app.mcpSettings("mcp.setPort", map[string]any{"port": float64(taken)}); err == nil {
		t.Fatal("accepted a port already in use")
	}
	status := s.status().(map[string]any)
	if status["port"] != mcpDefaultPort || status["running"] != true {
		t.Fatal(status)
	}

	for _, port := range []any{float64(80), float64(70000), float64(8080.5), "8080"} {
		if _, err := app.mcpSettings("mcp.setPort", map[string]any{"port": port}); err == nil {
			t.Fatalf("accepted port %v", port)
		}
	}

	free, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	next := free.Addr().(*net.TCPAddr).Port
	free.Close()
	if _, err := app.mcpSettings("mcp.setPort", map[string]any{"port": float64(next)}); err != nil {
		t.Fatal(err)
	}
	status = s.status().(map[string]any)
	if status["port"] != next || status["running"] != true || !strings.Contains(status["url"].(string), strconv.Itoa(next)) {
		t.Fatal(status)
	}
	// The moved listener answers, and only under its own Host.
	if _, err := net.Dial("tcp", mcpAddressFor(next)); err != nil {
		t.Fatal(err)
	}
	req := httptest.NewRequest("POST", "http://"+mcpAddress+"/mcp", strings.NewReader(`{}`))
	req.Header.Set("Authorization", "Bearer test-secret")
	rec := httptest.NewRecorder()
	s.handler().ServeHTTP(rec, req)
	if rec.Code != 403 {
		t.Fatalf("stale host accepted: %d", rec.Code)
	}
	var persisted mcpConfig
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, &persisted); err != nil {
		t.Fatal(err)
	}
	if persisted.Port != next {
		t.Fatal(persisted)
	}
}

func TestMCPRegenerateKeepsGrantAndBreaksOldCredential(t *testing.T) {
	app := &App{}
	s := testMCPService(t, app, true)
	session := connectTestMCP(t, s)
	if _, err := session.ListTools(context.Background(), nil); err != nil {
		t.Fatal(err)
	}
	before := s.config.Clients[0]

	result, err := app.mcpSettings("mcp.clientRegenerate", map[string]any{"id": "client"})
	if err != nil {
		t.Fatal(err)
	}
	token := result.(map[string]any)["token"].(string)
	if len(token) != 64 {
		t.Fatalf("token length=%d", len(token))
	}
	after := s.config.Clients[0]
	if after.ID != before.ID || after.Name != before.Name || after.TokenHash == before.TokenHash ||
		!reflect.DeepEqual(after.Accounts, before.Accounts) || after.Drafts != before.Drafts {
		t.Fatalf("grant changed: %#v -> %#v", before, after)
	}
	hash := sha256.Sum256([]byte(token))
	if after.TokenHash != hex.EncodeToString(hash[:]) {
		t.Fatal("stored hash is not the issued token's")
	}
	data, err := os.ReadFile(s.path)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(data), token) {
		t.Fatal("plaintext secret persisted")
	}

	// The old credential stops working; the new one takes over.
	if _, err := session.ListTools(context.Background(), nil); err == nil {
		t.Fatal("replaced credential still accepted")
	}
	for auth, want := range map[string]int{"Bearer test-secret": 401, "Bearer " + token: 200} {
		req := httptest.NewRequest("POST", "http://"+mcpAddress+"/mcp", strings.NewReader(
			`{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}`))
		req.Header.Set("Authorization", auth)
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Accept", "application/json, text/event-stream")
		rec := httptest.NewRecorder()
		s.handler().ServeHTTP(rec, req)
		if rec.Code != want {
			t.Fatalf("%s: status=%d body=%s", auth, rec.Code, rec.Body.String())
		}
	}
	if _, err := app.mcpSettings("mcp.clientRegenerate", map[string]any{"id": "gone"}); err == nil {
		t.Fatal("regenerated a client that does not exist")
	}
}
