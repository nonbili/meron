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

func TestAccountAddOAuthSavesProviderPhoto(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	t.Setenv("devserver", "")
	t.Setenv("frontenddevserverurl", "")
	photo := []byte("\xff\xd8\xff\xe0photo")
	for _, provider := range []string{"gmail", "outlook"} {
		t.Run(provider, func(t *testing.T) {
			app, writer := newMailHandlerTestApp(t,
				sidecarResponsePlan{Result: map[string]any{"accounts": []any{}}},
				sidecarResponsePlan{Result: map[string]any{"data": base64.StdEncoding.EncodeToString(photo), "ext": "jpg"}},
				sidecarResponsePlan{Result: map[string]any{"ok": true}},
				sidecarResponsePlan{Result: map[string]any{"ok": true}},
			)
			payload := map[string]any{"email": "me@example.com", "access_token": "access", "refresh_token": "refresh"}
			if _, err := addAccountForTest(app, provider, payload); err != nil {
				t.Fatal(err)
			}
			fetch := writer.calls[1]
			if fetch.Method != "oauth.fetchPhoto" || fetch.Params["provider"] != provider || fetch.Params["refresh_token"] != "refresh" {
				t.Fatalf("photo call = %#v", fetch)
			}
			avatar, _ := writer.calls[2].Params["avatar_url"].(string)
			if !strings.HasPrefix(avatar, "/media/avatars/me-example-com/") || !strings.HasSuffix(avatar, ".jpg") {
				t.Fatalf("connect avatar_url = %q", avatar)
			}
			saved, err := os.ReadFile(filepath.Join(mediaDir(), strings.TrimPrefix(avatar, "/media/")))
			if err != nil || string(saved) != string(photo) {
				t.Fatalf("saved photo = %q, %v", saved, err)
			}
		})
	}
}

func TestAccountReconnectOAuthKeepsAvatar(t *testing.T) {
	for _, provider := range []string{"gmail", "outlook"} {
		t.Run(provider, func(t *testing.T) {
			// No oauth.fetchPhoto: the stored avatar may be one the user chose.
			app, writer := newMailHandlerTestApp(t,
				sidecarResponsePlan{Result: map[string]any{"accounts": []any{map[string]any{"id": "me@example.com", "email": "me@example.com", "avatar_url": "/media/avatars/me/custom.png"}}}},
				sidecarResponsePlan{Result: map[string]any{"ok": true}},
				sidecarResponsePlan{Result: map[string]any{"ok": true}},
			)
			payload := map[string]any{"id": "me@example.com", "email": "me@example.com", "avatar_url": "https://example.com/provider.png", "access_token": "access", "refresh_token": "refresh"}
			if _, err := addAccountForTest(app, provider, payload); err != nil {
				t.Fatal(err)
			}
			if connect := writer.calls[1]; connect.Method != "account.connect" || connect.Params["avatar_url"] != "/media/avatars/me/custom.png" {
				t.Fatalf("connect = %#v", connect)
			}
		})
	}
}

func TestAccountAddOAuthRequiresTokens(t *testing.T) {
	app, writer := newMailHandlerTestApp(t)
	_, err := app.accountAddOutlookOAuth(map[string]any{"email": "me@example.com", "access_token": "access"})
	if err == nil || len(writer.calls) != 0 {
		t.Fatalf("err = %v, calls = %#v", err, writer.calls)
	}
}
