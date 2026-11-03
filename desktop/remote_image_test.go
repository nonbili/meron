package main

import (
	"bytes"
	"encoding/base64"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
)

func TestCacheRemoteImage(t *testing.T) {
	setupMediaTestEnv(t)
	raw, _ := base64.StdEncoding.DecodeString(tinyPNGBase64)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "image/png")
		w.Write(raw)
	}))
	defer server.Close()
	app := &App{}
	result, err := app.cacheRemoteImage(map[string]any{"url": server.URL + "/photo.png?size=large"})
	if err != nil {
		t.Fatal(err)
	}
	image := result.(map[string]any)
	repeated, err := app.cacheRemoteImage(map[string]any{"url": server.URL + "/photo.png?size=large"})
	if err != nil || repeated.(map[string]any)["key"] != image["key"] {
		t.Fatalf("repeated download should reuse its cache path: %v", err)
	}
	if image["mime"] != "image/png" {
		t.Fatalf("unexpected MIME: %v", image)
	}
	path, err := resolveMediaPath(image["key"].(string))
	if err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(got, raw) {
		t.Fatalf("cached bytes mismatch: %v", err)
	}
}

func TestCacheRemoteImageRejectsInvalidResources(t *testing.T) {
	app := &App{}
	for _, url := range []string{"", "file:///etc/passwd", "data:image/png;base64,AAAA", "ftp://example.com/a.png"} {
		if _, err := app.cacheRemoteImage(map[string]any{"url": url}); err == nil {
			t.Fatalf("accepted URL %q", url)
		}
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/missing" {
			w.WriteHeader(http.StatusNotFound)
			return
		}
		w.Write([]byte("not an image"))
	}))
	defer server.Close()
	for _, path := range []string{"/missing", "/text"} {
		if _, err := app.cacheRemoteImage(map[string]any{"url": server.URL + path}); err == nil {
			t.Fatalf("accepted resource %q", path)
		}
	}
}
