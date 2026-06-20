package main

import (
	"encoding/base64"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const tinyPNGBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII="

func setupMediaTestEnv(t *testing.T) {
	t.Helper()
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	t.Setenv("devserver", "")
	t.Setenv("frontenddevserverurl", "")
}

func TestResolveMediaPathConfinesKeysToMediaDir(t *testing.T) {
	setupMediaTestEnv(t)

	root := mediaDir()
	got, err := resolveMediaPath("avatars/acct/photo.png")
	if err != nil {
		t.Fatalf("resolveMediaPath returned error: %v", err)
	}
	if want := filepath.Join(root, "avatars", "acct", "photo.png"); got != want {
		t.Fatalf("resolveMediaPath = %q, want %q", got, want)
	}

	for _, key := range []string{"", "../secret.png", "/../secret.png", "avatars/../../../secret.png"} {
		if path, err := resolveMediaPath(key); err == nil {
			t.Fatalf("resolveMediaPath(%q) = %q, nil error; want traversal rejection", key, path)
		}
	}
}

func TestImageMimeForPathMapsKnownImageExtensions(t *testing.T) {
	tests := map[string]string{
		"photo.png":   "image/png",
		"photo.JPG":   "image/jpeg",
		"photo.jpeg":  "image/jpeg",
		"photo.gif":   "image/gif",
		"photo.webp":  "image/webp",
		"photo.bmp":   "image/bmp",
		"photo.other": "image/png",
	}

	for path, want := range tests {
		if got := imageMimeForPath(path); got != want {
			t.Fatalf("imageMimeForPath(%q) = %q, want %q", path, got, want)
		}
	}
}

func TestHasClipboardTypeMatchesCaseAndWhitespace(t *testing.T) {
	types := []string{" text/plain ", "IMAGE/PNG", "image/jpeg"}
	if !hasClipboardType(types, "image/png") {
		t.Fatal("hasClipboardType did not match case-insensitive image/png")
	}
	if hasClipboardType(types, "image/gif") {
		t.Fatal("hasClipboardType matched missing image/gif")
	}
}

func TestCopyImageRejectsInvalidKeyBeforeClipboardAccess(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	for _, key := range []string{"", "../secret.png"} {
		if _, err := app.copyImage(map[string]any{"key": key}); err == nil || !strings.Contains(err.Error(), "attachment key") {
			t.Fatalf("copyImage(%q) err = %v, want attachment key error", key, err)
		}
	}
}

func TestWriteMediaFileWritesDecodedDataWithSafeGeneratedName(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	res, err := app.writeMediaFile(map[string]any{
		"filename": "note.image.jpeg",
		"data":     base64.StdEncoding.EncodeToString([]byte("image bytes")),
	})
	if err != nil {
		t.Fatalf("writeMediaFile error: %v", err)
	}

	url, ok := res.(string)
	if !ok {
		t.Fatalf("writeMediaFile result = %#v, want string", res)
	}
	if !strings.HasPrefix(url, "/media/") || !strings.HasSuffix(url, ".jpeg") {
		t.Fatalf("writeMediaFile url = %q", url)
	}
	key := strings.TrimPrefix(url, "/media/")
	if strings.ContainsAny(key, `/\`) {
		t.Fatalf("writeMediaFile key contains path separator: %q", key)
	}
	path := filepath.Join(mediaDir(), key)
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read written media file: %v", err)
	}
	if string(got) != "image bytes" {
		t.Fatalf("written media content = %q", got)
	}
}

func TestWriteMediaFileDefaultsMissingExtensionToPNG(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	res, err := app.writeMediaFile(map[string]any{
		"filename": "pasted-image",
		"data":     base64.StdEncoding.EncodeToString([]byte("image bytes")),
	})
	if err != nil {
		t.Fatalf("writeMediaFile error: %v", err)
	}
	url := res.(string)
	if !strings.HasSuffix(url, ".png") {
		t.Fatalf("writeMediaFile url = %q, want .png suffix", url)
	}
}

func TestWriteMediaFileRejectsMissingAndInvalidData(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	if _, err := app.writeMediaFile(map[string]any{"filename": "x.png"}); err == nil || !strings.Contains(err.Error(), "missing data") {
		t.Fatalf("missing data err = %v", err)
	}
	if _, err := app.writeMediaFile(map[string]any{"filename": "x.png", "data": "not-base64"}); err == nil || !strings.Contains(err.Error(), "decode base64") {
		t.Fatalf("invalid base64 err = %v", err)
	}
}

func TestWriteAvatarFileWritesValidatedImageWithSanitizedAccount(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	res, err := app.writeAvatarFile(map[string]any{
		"account_id": "acct/../one@example.com",
		"data":       tinyPNGBase64,
	})
	if err != nil {
		t.Fatalf("writeAvatarFile error: %v", err)
	}
	obj, ok := res.(map[string]any)
	if !ok {
		t.Fatalf("writeAvatarFile result = %#v", res)
	}
	url, _ := obj["url"].(string)
	if !strings.HasPrefix(url, "/media/avatars/acct----one-example-com/") || !strings.HasSuffix(url, ".png") {
		t.Fatalf("writeAvatarFile url = %q", url)
	}
	path := filepath.Join(mediaDir(), strings.TrimPrefix(url, "/media/"))
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("avatar file missing: %v", err)
	}
}

func TestWriteAvatarFileRejectsInvalidInputs(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	if _, err := app.writeAvatarFile(map[string]any{"data": tinyPNGBase64}); err == nil || !strings.Contains(err.Error(), "account id required") {
		t.Fatalf("missing account err = %v", err)
	}
	if _, err := app.writeAvatarFile(map[string]any{"id": "acct"}); err == nil || !strings.Contains(err.Error(), "missing data") {
		t.Fatalf("missing data err = %v", err)
	}
	if _, err := app.writeAvatarFile(map[string]any{"id": "acct", "data": "not-base64"}); err == nil || !strings.Contains(err.Error(), "decode base64") {
		t.Fatalf("invalid base64 err = %v", err)
	}
	if _, err := app.writeAvatarFile(map[string]any{
		"id":   "acct",
		"data": base64.StdEncoding.EncodeToString([]byte("not an image")),
	}); err == nil || !strings.Contains(err.Error(), "avatar must be") {
		t.Fatalf("non-image err = %v", err)
	}
}

func TestPruneComposerMediaRemovesOnlyRootFilesNotKept(t *testing.T) {
	setupMediaTestEnv(t)

	root := mediaDir()
	if err := os.MkdirAll(filepath.Join(root, "avatars", "acct"), 0755); err != nil {
		t.Fatal(err)
	}
	for name, data := range map[string]string{
		"keep.png":        "keep",
		"remove.png":      "remove",
		"remove-too.webp": "remove too",
	} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(data), 0644); err != nil {
			t.Fatal(err)
		}
	}
	nestedPath := filepath.Join(root, "avatars", "acct", "avatar.png")
	if err := os.WriteFile(nestedPath, []byte("avatar"), 0644); err != nil {
		t.Fatal(err)
	}

	app := &App{}
	res, err := app.pruneComposerMedia(map[string]any{
		"keys": []any{"keep.png", "", 12, "missing.png"},
	})
	if err != nil {
		t.Fatalf("pruneComposerMedia error: %v", err)
	}
	obj, ok := res.(map[string]any)
	if !ok {
		t.Fatalf("pruneComposerMedia result = %#v", res)
	}
	if got, want := obj["removed"], 2; got != want {
		t.Fatalf("removed = %#v, want %d", got, want)
	}
	for _, path := range []string{filepath.Join(root, "keep.png"), nestedPath} {
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("expected file to remain %s: %v", path, err)
		}
	}
	for _, name := range []string{"remove.png", "remove-too.webp"} {
		if _, err := os.Stat(filepath.Join(root, name)); !os.IsNotExist(err) {
			t.Fatalf("expected %s to be removed, stat err = %v", name, err)
		}
	}
}

func TestPruneComposerMediaMissingDirectoryIsNoop(t *testing.T) {
	setupMediaTestEnv(t)

	app := &App{}
	res, err := app.pruneComposerMedia(map[string]any{})
	if err != nil {
		t.Fatalf("pruneComposerMedia error: %v", err)
	}
	obj := res.(map[string]any)
	if got, want := obj["removed"], 0; got != want {
		t.Fatalf("removed = %#v, want %d", got, want)
	}
}
