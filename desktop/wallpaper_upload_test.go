package main

import (
	"encoding/base64"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestWriteChatWallpaperFileWritesValidatedImage(t *testing.T) {
	cacheHome := t.TempDir()
	t.Setenv("XDG_CACHE_HOME", cacheHome)
	t.Setenv("devserver", "")
	t.Setenv("frontenddevserverurl", "")

	png, err := base64.StdEncoding.DecodeString("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=")
	if err != nil {
		t.Fatal(err)
	}

	app := &App{}
	res, err := app.writeChatWallpaperFile(map[string]any{
		"id":   "acct/../one",
		"data": base64.StdEncoding.EncodeToString(png),
	})
	if err != nil {
		t.Fatalf("writeChatWallpaperFile error: %v", err)
	}
	obj, ok := res.(map[string]any)
	if !ok {
		t.Fatalf("result = %#v", res)
	}
	url, _ := obj["url"].(string)
	if !strings.HasPrefix(url, "/media/wallpapers/acct") || !strings.HasSuffix(url, ".png") {
		t.Fatalf("url = %q", url)
	}
	rel := strings.TrimPrefix(url, "/media/wallpapers/")
	accountPart := strings.SplitN(rel, "/", 2)[0]
	if strings.Contains(accountPart, ".") || strings.Contains(accountPart, "/") {
		t.Fatalf("unsafe account path segment in url = %q", url)
	}
	path := filepath.Join(mediaDir(), strings.TrimPrefix(url, "/media/"))
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("wallpaper file missing: %v", err)
	}
}

func TestWriteChatWallpaperFileRejectsNonImage(t *testing.T) {
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	t.Setenv("devserver", "")
	t.Setenv("frontenddevserverurl", "")

	app := &App{}
	_, err := app.writeChatWallpaperFile(map[string]any{
		"id":   "acct",
		"data": base64.StdEncoding.EncodeToString([]byte("not an image")),
	})
	if err == nil || !strings.Contains(err.Error(), "wallpaper must be") {
		t.Fatalf("err = %v", err)
	}
}
