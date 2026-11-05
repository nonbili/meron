package main

import (
	"net/http/httptest"
	"testing"
	"testing/fstest"
)

func TestDesktopAssetHandler(t *testing.T) {
	handler := desktopAssetHandlerWithAssets(fstest.MapFS{
		"index.html": &fstest.MapFile{Data: []byte("<!doctype html><title>Meron test</title>")},
	})
	for _, path := range []string{"/", "/media/meron-missing-attachment"} {
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, httptest.NewRequest("GET", path, nil))
		if path == "/" && response.Code != 200 {
			t.Fatalf("frontend status: %d", response.Code)
		}
		if path != "/" && response.Code != 404 {
			t.Fatalf("missing media must not fall back to frontend: %d", response.Code)
		}
	}
}
