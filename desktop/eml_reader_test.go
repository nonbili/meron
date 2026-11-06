package main

import (
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wailsapp/wails/v3/pkg/application"
)

type fakeEmlWindow struct {
	shown, focused, closed int
	onClose                func()
}

func (w *fakeEmlWindow) Show() application.Window { w.shown++; return nil }
func (w *fakeEmlWindow) Focus()                   { w.focused++ }
func (w *fakeEmlWindow) Close()                   { w.closed++; w.onClose() }

func TestEmlReaderReusesCanonicalPathAndCleansOnlyItsSession(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	path := filepath.Join(t.TempDir(), "Saved mail.EML")
	original := []byte("Subject: Hello\r\n\r\nBody")
	if err := os.WriteFile(path, original, 0o600); err != nil {
		t.Fatal(err)
	}
	message := map[string]any{"subject": "Hello", "body": "Body", "attachments": []any{}}
	app, sidecar := newMailHandlerTestApp(t, sidecarResponsePlan{Result: message})
	oldFactory := newEmlReaderWindow
	defer func() { newEmlReaderWindow = oldFactory }()
	var window *fakeEmlWindow
	var token string
	newEmlReaderWindow = func(options application.WebviewWindowOptions, closed func()) emlReaderWindow {
		token = options.Name
		if options.URL != "/?eml="+token || !options.Hidden {
			t.Fatalf("unexpected document options: %+v", options)
		}
		window = &fakeEmlWindow{onClose: closed}
		return window
	}
	if _, err := app.openEml(path); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(filepath.Join(mediaDir(), "_eml", token)) })
	rawURL := (&url.URL{Scheme: "file", Path: filepath.ToSlash(path)}).String()
	if _, err := app.openEml(rawURL); err != nil {
		t.Fatal(err)
	}
	if window.shown != 2 || window.focused != 2 || len(sidecar.calls) != 1 {
		t.Fatalf("did not focus existing reader: %+v, calls %v", window, sidecar.calls)
	}
	call := sidecar.calls[0]
	if call.Method != "eml.read" || call.Params["session"] != token || call.Params["path"] == path {
		t.Fatalf("expected isolated snapshot, got %+v", call)
	}
	snapshot := call.Params["path"].(string)
	if _, err := os.Stat(snapshot); !os.IsNotExist(err) {
		t.Fatal("raw snapshot retained after parsing")
	}
	if data, err := app.emlCommand("eml.get", map[string]any{"session": token}); err != nil || data.(map[string]any)["subject"] != "Hello" {
		t.Fatalf("document unavailable: %v, %v", data, err)
	}
	dir := filepath.Dir(snapshot)
	if err := os.WriteFile(filepath.Join(dir, "attachment.png"), []byte("image"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := app.emlCommand("eml.saveAttachment", map[string]any{"session": token, "key": "another-account/INBOX/1/secret.pdf"}); err == nil {
		t.Fatal("allowed another message's attachment")
	}
	if _, err := app.emlCommand("eml.close", map[string]any{"session": token}); err != nil {
		t.Fatal(err)
	}
	if len(app.emlReaders) != 0 || window.closed != 1 {
		t.Fatal("reader was not closed")
	}
	if _, err := os.Stat(dir); !os.IsNotExist(err) {
		t.Fatal("reader media not cleaned up")
	}
	if _, err := app.emlCommand("eml.get", map[string]any{"session": token}); err == nil {
		t.Fatal("closed reader still accessible")
	}
	bytes, err := os.ReadFile(path)
	if err != nil || string(bytes) != string(original) {
		t.Fatal("original document changed")
	}
}

func TestEmlPathsAndStartupQueue(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "message.eml")
	if err := os.WriteFile(path, []byte("Subject: test\r\n\r\nbody"), 0o600); err != nil {
		t.Fatal(err)
	}
	rawURL := (&url.URL{Scheme: "file", Path: filepath.ToSlash(path)}).String()
	for _, raw := range []string{path, rawURL} {
		actual, err := emlPath(raw)
		if err != nil || actual != path {
			t.Fatalf("emlPath(%q) = %q, %v", raw, actual, err)
		}
	}
	for _, raw := range []string{root, "file://example.com/message.eml", filepath.Join(root, "missing.eml"), filepath.Join(root, "notes.txt")} {
		if _, err := emlPath(raw); err == nil {
			t.Fatalf("accepted invalid document %q", raw)
		}
	}
	huge := filepath.Join(root, "huge.eml")
	file, err := os.Create(huge)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(100*1024*1024 + 1); err != nil {
		t.Fatal(err)
	}
	file.Close()
	if _, err := emlPath(huge); err == nil || !strings.Contains(err.Error(), "100 MiB") {
		t.Fatalf("oversized document: %v", err)
	}
	app := &App{}
	if !app.openEmlArguments([]string{"--flag", path, rawURL, "mailto:test@example.com"}) || len(app.pendingEml) != 2 {
		t.Fatalf("startup queue: %v", app.pendingEml)
	}
}

func TestEmlArgumentsUseLaunchingProcessDirectory(t *testing.T) {
	dir := t.TempDir()
	rawURL := (&url.URL{Scheme: "file", Path: filepath.ToSlash(filepath.Join(dir, "url.eml"))}).String()
	mailto := "mailto:alice@example.com?subject=message.eml"
	args := []string{"meron", "relative.eml", rawURL, mailto, "https://example.com/message.eml"}
	resolved := emlArgumentsAt(args, dir)
	if resolved[1] != filepath.Join(dir, "relative.eml") || args[1] != "relative.eml" {
		t.Fatalf("relative path or caller arguments changed incorrectly: %v, %v", resolved, args)
	}
	if resolved[2] != rawURL || resolved[3] != mailto || isEmlArgument(mailto) || isEmlArgument(args[4]) {
		t.Fatalf("URLs treated as document paths: %v", resolved)
	}
}

func TestEmlSubjectFilenamesWithColonRemainPaths(t *testing.T) {
	for _, name := range []string{"Re: Budget.eml", "Fwd:Report.eml"} {
		if !isEmlArgument(name) {
			t.Fatalf("subject filename rejected: %s", name)
		}
		if got := emlArgumentsAt([]string{name}, "/messages")[0]; got != filepath.Join("/messages", name) {
			t.Fatalf("subject filename resolved as %s", got)
		}
	}
}

func TestClearCachePreservesOpenEmlAttachments(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	session := filepath.Join(mediaDir(), "_eml", "reader-test", "1", "attachment.pdf")
	cached := filepath.Join(mediaDir(), "account", "INBOX", "1", "attachment.pdf")
	for _, path := range []string{session, cached} {
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte("attachment"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := (&App{}).storageClearCache(); err != nil {
		t.Fatal(err)
	}
	if data, err := os.ReadFile(session); err != nil || string(data) != "attachment" {
		t.Fatalf("reader attachment lost: %q, %v", data, err)
	}
	if _, err := os.Stat(cached); !os.IsNotExist(err) {
		t.Fatalf("mail cache was not cleared: %v", err)
	}
}
