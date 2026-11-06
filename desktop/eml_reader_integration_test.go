//go:build integration

package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func TestStandaloneEmlReaderIntegration(t *testing.T) {
	sidecar, _ := startSidecar(t)
	app := &App{sidecar: sidecar}
	oldFactory := newEmlReaderWindow
	defer func() { newEmlReaderWindow = oldFactory }()
	var token string
	newEmlReaderWindow = func(options application.WebviewWindowOptions, closed func()) emlReaderWindow {
		token = options.Name
		return &fakeEmlWindow{onClose: closed}
	}
	path := filepath.Join(t.TempDir(), "message.eml")
	raw := "From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: External document\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/html\r\n\r\n<p>External body</p>\r\n--b\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=report.pdf\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8=\r\n--b--\r\n"
	if err := os.WriteFile(path, []byte(raw), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := app.openEml(path); err != nil {
		t.Fatal(err)
	}
	defer app.emlCommand("eml.close", map[string]any{"session": token})
	data, err := app.emlCommand("eml.get", map[string]any{"session": token})
	if err != nil {
		t.Fatal(err)
	}
	message := data.(map[string]any)
	if message["subject"] != "External document" || !strings.Contains(str(message, "body_html"), "Content-Security-Policy") {
		t.Fatalf("bad parsed message: %v", message)
	}
	attachments := message["attachments"].([]any)
	attachment := attachments[0].(map[string]any)
	key := str(attachment, "key")
	if !strings.HasPrefix(key, "_eml/"+token+"/1/") {
		t.Fatalf("bad attachment key: %s", key)
	}
	media, err := os.ReadFile(filepath.Join(mediaDir(), key))
	if err != nil || string(media) != "hello" {
		t.Fatalf("attachment: %q, %v", media, err)
	}
	accounts := callMap(t, sidecar, "account.list", map[string]any{})
	if len(accounts["accounts"].([]any)) != 0 {
		t.Fatal("reading an EML created an account")
	}
	if _, err := app.emlCommand("eml.close", map[string]any{"session": token}); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(mediaDir(), "_eml", token)); !os.IsNotExist(err) {
		t.Fatal("reader media survived close")
	}
	original, err := os.ReadFile(path)
	if err != nil || string(original) != raw {
		t.Fatal("original EML changed")
	}
}

func TestOversizedEmlResponseLeavesSidecarUsable(t *testing.T) {
	sidecar, _ := startSidecar(t)
	app := &App{sidecar: sidecar}
	path := filepath.Join(t.TempDir(), "large.eml")
	raw := "Subject: Large body\r\nContent-Type: text/plain\r\n\r\n" + strings.Repeat("x", 9*1024*1024)
	if err := os.WriteFile(path, []byte(raw), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := app.openEml(path); err == nil || !strings.Contains(err.Error(), "display limit") {
		t.Fatalf("expected bounded display error, got %v", err)
	}
	if _, err := sidecar.Call("ping", nil); err != nil {
		t.Fatalf("large EML broke the engine connection: %v", err)
	}
	entries, err := os.ReadDir(filepath.Join(mediaDir(), "_eml"))
	if err != nil || len(entries) != 0 {
		t.Fatalf("failed reader left temporary data: %v, %v", entries, err)
	}
}
