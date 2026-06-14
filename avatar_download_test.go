package main

import (
	"encoding/base64"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDownloadAndSaveAvatar(t *testing.T) {
	cacheHome := t.TempDir()
	t.Setenv("XDG_CACHE_HOME", cacheHome)
	t.Setenv("devserver", "")
	t.Setenv("frontenddevserverurl", "")

	// 1x1 transparent PNG
	pngBytes, err := base64.StdEncoding.DecodeString("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=")
	if err != nil {
		t.Fatal(err)
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "image/png")
		w.Write(pngBytes)
	}))
	defer server.Close()

	app := &App{}
	localURL := app.downloadAndSaveAvatar("test-user@example.com", server.URL)

	if !strings.HasPrefix(localURL, "/media/avatars/test-user-example-com/") {
		t.Errorf("expected local url to start with /media/avatars/test-user-example-com/, got %q", localURL)
	}
	if !strings.HasSuffix(localURL, ".png") {
		t.Errorf("expected local url to end with .png, got %q", localURL)
	}

	// Verify file exists on disk
	path := filepath.Join(mediaDir(), strings.TrimPrefix(localURL, "/media/"))
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("downloaded avatar file missing: %v", err)
	}
}

func TestDownloadAndSaveAvatarIgnoresNonRemote(t *testing.T) {
	app := &App{}
	url1 := app.downloadAndSaveAvatar("user", "/media/avatars/user/abc.png")
	if url1 != "/media/avatars/user/abc.png" {
		t.Errorf("expected local url to be untouched, got %q", url1)
	}

	url2 := app.downloadAndSaveAvatar("user", "")
	if url2 != "" {
		t.Errorf("expected empty url to be untouched, got %q", url2)
	}
}
