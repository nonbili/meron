//go:build integration

package main

// Test harness for a real CalDAV server: a throwaway Stalwart container,
// configured over its admin API the way ~/repo/mservers does it, with one user
// whose calendars the tests sync against.

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os/exec"
	"strings"
	"testing"
	"time"
)

const (
	stalwartImage = "stalwartlabs/stalwart:v1.0.3"
	// Stalwart's default password policy rejects common passwords.
	calendarPassword = "Quartz-Lantern-42"
	calendarUser     = "alice@" + mailHostname
)

type calendarServer struct {
	container string
	url       string
}

// startStalwart launches a Stalwart container, completes its first-run setup
// through the admin API, and creates calendarUser.
func startStalwart(t *testing.T) *calendarServer {
	t.Helper()
	docker := dockerBin(t)
	port := freePort(t)
	admin := "admin:" + calendarPassword
	server := &calendarServer{url: fmt.Sprintf("http://127.0.0.1:%d", port)}
	server.container = runCmd(t, docker, "run", "-d", "--rm",
		"-e", "STALWART_RECOVERY_ADMIN="+admin,
		"-p", fmt.Sprintf("127.0.0.1:%d:8080", port),
		stalwartImage)
	t.Cleanup(func() {
		if t.Failed() {
			logs, _ := exec.Command(docker, "logs", server.container).CombinedOutput()
			t.Logf("stalwart logs:\n%s", logs)
		}
		exec.Command(docker, "stop", "-t", "1", server.container).Run()
	})

	server.waitForHTTP(t, docker)
	server.admin(t, admin, []any{"x:Bootstrap/set", map[string]any{"update": map[string]any{"singleton": map[string]any{
		"defaultHostname":       "mail." + mailHostname,
		"defaultDomain":         mailHostname,
		"requestTlsCertificate": false,
		"generateDkimKeys":      false,
		"tracer":                map[string]any{"@type": "Stdout"},
	}}}})
	// Setup takes effect on restart; the recovery admin keeps working after.
	runCmd(t, docker, "restart", "-t", "2", server.container)
	server.waitForHTTP(t, docker)

	domains := server.admin(t, admin, []any{"x:Domain/query", map[string]any{}})
	ids, _ := domains["ids"].([]any)
	if len(ids) != 1 {
		t.Fatalf("expected one domain after setup, got %v", domains)
	}
	created := server.admin(t, admin, []any{"x:Account/set", map[string]any{"create": map[string]any{"a": map[string]any{
		"@type":       "User",
		"name":        strings.Split(calendarUser, "@")[0],
		"domainId":    ids[0],
		"credentials": map[string]any{"0": map[string]any{"@type": "Password", "secret": calendarPassword}},
	}}}})
	if _, ok := created["created"]; !ok || created["notCreated"] != nil {
		t.Fatalf("create calendar user: %v", created)
	}
	return server
}

func (s *calendarServer) waitForHTTP(t *testing.T, docker string) {
	t.Helper()
	deadline := time.Now().Add(60 * time.Second)
	for time.Now().Before(deadline) {
		resp, err := http.Get(s.url + "/")
		if err == nil {
			resp.Body.Close()
			return
		}
		time.Sleep(300 * time.Millisecond)
	}
	logs, _ := exec.Command(docker, "logs", s.container).CombinedOutput()
	t.Fatalf("stalwart did not answer at %s\n%s", s.url, logs)
}

// admin runs one JMAP admin call and returns its result object.
func (s *calendarServer) admin(t *testing.T, credentials string, call []any) map[string]any {
	t.Helper()
	body, _ := json.Marshal(map[string]any{
		"using":       []string{"urn:ietf:params:jmap:core", "urn:stalwart:jmap"},
		"methodCalls": []any{append(call, "c0")},
	})
	user, password, _ := strings.Cut(credentials, ":")
	req, _ := http.NewRequest("POST", s.url+"/jmap/", bytes.NewReader(body))
	req.SetBasicAuth(user, password)
	req.Header.Set("Content-Type", "application/json")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("stalwart admin %v: %v", call[0], err)
	}
	defer resp.Body.Close()
	var decoded struct {
		MethodResponses [][]json.RawMessage `json:"methodResponses"`
	}
	raw, _ := io.ReadAll(resp.Body)
	if err := json.Unmarshal(raw, &decoded); err != nil || len(decoded.MethodResponses) != 1 {
		t.Fatalf("stalwart admin %v: %d %s", call[0], resp.StatusCode, raw)
	}
	var name string
	var result map[string]any
	json.Unmarshal(decoded.MethodResponses[0][0], &name)
	json.Unmarshal(decoded.MethodResponses[0][1], &result)
	if name == "error" {
		t.Fatalf("stalwart admin %v: %s", call[0], raw)
	}
	return result
}

// dav sends one request to the calendar server as calendarUser.
func (s *calendarServer) dav(t *testing.T, method, path, body string, headers map[string]string) (int, string) {
	t.Helper()
	req, _ := http.NewRequest(method, s.url+path, strings.NewReader(body))
	req.SetBasicAuth(calendarUser, calendarPassword)
	for name, value := range headers {
		req.Header.Set(name, value)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("%s %s: %v", method, path, err)
	}
	defer resp.Body.Close()
	raw, _ := io.ReadAll(resp.Body)
	return resp.StatusCode, string(raw)
}

// calendarPath is where Stalwart keeps calendarUser's default calendar.
func calendarPath() string {
	return "/dav/cal/" + strings.ReplaceAll(calendarUser, "@", "%40") + "/default/"
}

// serverCalendarData returns every object in the default calendar, concatenated.
func (s *calendarServer) serverCalendarData(t *testing.T) string {
	t.Helper()
	return s.calendarDataAt(t, calendarPath())
}

// calendarDataAt returns every object in the calendar at path, concatenated.
func (s *calendarServer) calendarDataAt(t *testing.T, path string) string {
	t.Helper()
	status, body := s.dav(t, "REPORT", path,
		`<?xml version="1.0"?><c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><c:calendar-data/></d:prop><c:filter><c:comp-filter name="VCALENDAR"/></c:filter></c:calendar-query>`,
		map[string]string{"Depth": "1", "Content-Type": "application/xml"})
	if status != 207 {
		t.Fatalf("calendar-query: %d %s", status, body)
	}
	return body
}
