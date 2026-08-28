package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestJSONHelpersCoerceExpectedTypes(t *testing.T) {
	if got := jsonString("value"); got != "value" {
		t.Fatalf("jsonString string = %q", got)
	}
	if got := jsonString(12); got != "" {
		t.Fatalf("jsonString non-string = %q, want empty", got)
	}

	if got := jsonBool(true); got != true {
		t.Fatalf("jsonBool true = %v", got)
	}
	if got := jsonBool("true"); got != false {
		t.Fatalf("jsonBool non-bool = %v, want false", got)
	}

	tests := []struct {
		value any
		want  int64
	}{
		{float64(42.9), 42},
		{json.Number("43"), 43},
		{int64(44), 44},
		{"45", 0},
		{json.Number("not-a-number"), 0},
	}
	for _, tt := range tests {
		if got := jsonNumber(tt.value); got != tt.want {
			t.Fatalf("jsonNumber(%#v) = %d, want %d", tt.value, got, tt.want)
		}
	}
}

// The App Store build ships the sidecar at Meron.app/Contents/MacOS/meron-core
// and relies on this lookup to find it: the sandbox forbids exec'ing the copy
// the other builds extract to the cache dir.
func TestBundledSidecarPathIn(t *testing.T) {
	dir := t.TempDir()
	if got := bundledSidecarPathIn(dir); got != "" {
		t.Fatalf("empty dir = %q, want no sidecar", got)
	}

	// A directory of that name is not a sidecar.
	path := filepath.Join(dir, sidecarBinaryName)
	if err := os.Mkdir(path, 0o755); err != nil {
		t.Fatal(err)
	}
	if got := bundledSidecarPathIn(dir); got != "" {
		t.Fatalf("directory = %q, want no sidecar", got)
	}

	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, nil, 0o755); err != nil {
		t.Fatal(err)
	}
	if got := bundledSidecarPathIn(dir); got != path {
		t.Fatalf("bundledSidecarPathIn = %q, want %q", got, path)
	}
}

func TestSidecarErrorMessage(t *testing.T) {
	if got := sidecarErrorMessage(map[string]any{"message": "bad credentials"}); got != "bad credentials" {
		t.Fatalf("sidecarErrorMessage message = %q", got)
	}
	if got := sidecarErrorMessage(map[string]any{"code": "NO"}); got != `{"code":"NO"}` {
		t.Fatalf("sidecarErrorMessage map = %q", got)
	}
	if got := sidecarErrorMessage("plain"); got != "plain" {
		t.Fatalf("sidecarErrorMessage string = %q", got)
	}
	if got := sidecarErrorMessage(12); got != "12" {
		t.Fatalf("sidecarErrorMessage int = %q", got)
	}
}

func TestSidecarCallTimeouts(t *testing.T) {
	tests := map[string]time.Duration{
		"account.connect":     45 * time.Second,
		"messages.thread":     30 * time.Second,
		"messages.markRead":   30 * time.Second,
		"folders.list":        15 * time.Second,
		"folders.delete":      30 * time.Second,
		"rss.importOpml":      15 * time.Second,
		"backup.export":       30 * time.Second,
		"backup.import":       30 * time.Second,
		"unknown.sidecarCall": 5 * time.Second,
	}
	for method, want := range tests {
		if got := sidecarCallTimeout(method); got != want {
			t.Fatalf("sidecarCallTimeout(%q) = %s, want %s", method, got, want)
		}
	}
}

func TestFileExists(t *testing.T) {
	if fileExists("") {
		t.Fatal("fileExists(\"\") = true")
	}
	path := t.TempDir() + "/file.txt"
	if fileExists(path) {
		t.Fatal("fileExists before create = true")
	}
	if err := os.WriteFile(path, []byte("x"), 0644); err != nil {
		t.Fatal(err)
	}
	if !fileExists(path) {
		t.Fatal("fileExists after create = false")
	}
}

func TestOAuthConfigIgnoresEnvironmentOverrides(t *testing.T) {
	t.Setenv("MERON_GOOGLE_CLIENT_ID", "google-id")
	t.Setenv("MERON_GOOGLE_CLIENT_SECRET", "google-secret")
	t.Setenv("MERON_OUTLOOK_CLIENT_ID", "outlook-id")

	if got := googleClientID(); got == "" || got == "google-id" {
		t.Fatalf("googleClientID = %q, want baked id", got)
	}
	if got := googleClientSecret(); got == "" || got == "google-secret" {
		t.Fatalf("googleClientSecret = %q, want baked secret", got)
	}
	if !gmailOAuthConfigured() {
		t.Fatal("gmailOAuthConfigured = false with baked credentials")
	}
	if got := outlookClientID(); got == "" || got == "outlook-id" {
		t.Fatalf("outlookClientID = %q, want baked id", got)
	}
	if !outlookOAuthConfigured() {
		t.Fatal("outlookOAuthConfigured = false with baked client id")
	}
}

func TestOutlookOAuthConfiguredFalseWithoutClientID(t *testing.T) {
	t.Setenv("MERON_OUTLOOK_CLIENT_ID", "")

	old := outlookClientIDObf
	outlookClientIDObf = ""
	t.Cleanup(func() {
		outlookClientIDObf = old
	})

	if got := outlookClientID(); got != "" {
		t.Fatalf("outlookClientID = %q, want empty", got)
	}
	if outlookOAuthConfigured() {
		t.Fatal("outlookOAuthConfigured = true, want false")
	}
}

func TestOutlookOAuthConfiguredFromBakedClientID(t *testing.T) {
	t.Setenv("MERON_OUTLOOK_CLIENT_ID", "")

	old := outlookClientIDObf
	t.Cleanup(func() {
		outlookClientIDObf = old
	})

	if got := outlookClientID(); got == "" {
		t.Fatal("outlookClientID = empty with baked client id")
	}
	if !outlookOAuthConfigured() {
		t.Fatal("outlookOAuthConfigured = false with baked client id")
	}
}
