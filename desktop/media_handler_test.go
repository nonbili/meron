package main

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestMediaHandler(t *testing.T) {
	root := t.TempDir()
	files := map[string]string{
		"acct/inbox/1/0.png":  "\x89PNG\r\n\x1a\n",
		"acct/inbox/1/1.svg":  `<svg xmlns="http://www.w3.org/2000/svg"/>`,
		"acct/inbox/1/2.mp4":  "video",
		"acct/inbox/1/3.html": "<script src=4.js></script>",
		"acct/inbox/1/4.js":   "alert(1)",
		"acct/inbox/1/5.bin":  "<html><script>alert(1)</script>",
		"acct/inbox/1/6.PNG":  "\x89PNG\r\n\x1a\n",
	}
	for name, body := range files {
		path := filepath.Join(root, name)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "acct/inbox/1/index.html"), []byte("<p>listing</p>"), 0o644); err != nil {
		t.Fatal(err)
	}
	handler := mediaHandlerAt(root)

	for _, test := range []struct {
		name, method, path string
		status             int
		contentType        string
	}{
		{name: "image", path: "/media/acct/inbox/1/0.png", status: 200, contentType: "image/png"},
		{name: "upper-case extension", path: "/media/acct/inbox/1/6.PNG", status: 200, contentType: "image/png"},
		{name: "svg", path: "/media/acct/inbox/1/1.svg", status: 200, contentType: "image/svg+xml"},
		{name: "video", path: "/media/acct/inbox/1/2.mp4", status: 200, contentType: "video/mp4"},
		{name: "html attachment", path: "/media/acct/inbox/1/3.html", status: 200, contentType: "application/octet-stream"},
		{name: "script attachment", path: "/media/acct/inbox/1/4.js", status: 200, contentType: "application/octet-stream"},
		{name: "unknown type is not sniffed", path: "/media/acct/inbox/1/5.bin", status: 200, contentType: "application/octet-stream"},
		{name: "HEAD", method: "HEAD", path: "/media/acct/inbox/1/0.png", status: 200, contentType: "image/png"},
		{name: "POST", method: "POST", path: "/media/acct/inbox/1/0.png", status: 405},
		{name: "directory", path: "/media/acct/inbox/1/", status: 404},
		{name: "directory without slash", path: "/media/acct/inbox/1", status: 404},
		{name: "root", path: "/media/", status: 404},
		{name: "traversal", path: "/media/../../etc/passwd", status: 404},
		{name: "missing", path: "/media/acct/inbox/1/9.png", status: 404},
	} {
		t.Run(test.name, func(t *testing.T) {
			method := test.method
			if method == "" {
				method = "GET"
			}
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, httptest.NewRequest(method, test.path, nil))
			if response.Code != test.status {
				t.Fatalf("status = %d, want %d", response.Code, test.status)
			}
			if test.status != 200 {
				return
			}
			header := response.Header()
			if got := header.Get("Content-Type"); got != test.contentType {
				t.Errorf("Content-Type = %q, want %q", got, test.contentType)
			}
			if got := header.Get("Content-Security-Policy"); got != mediaContentSecurityPolicy {
				t.Errorf("Content-Security-Policy = %q", got)
			}
			if got := header.Get("X-Content-Type-Options"); got != "nosniff" {
				t.Errorf("X-Content-Type-Options = %q", got)
			}
		})
	}
}

// The app-wide CSP middleware runs first; the media handler's stricter policy
// must be the one that reaches the webview.
func TestMediaPolicyOverridesAppPolicy(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "a"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "a/0.html"), []byte("<p>x</p>"), 0o644); err != nil {
		t.Fatal(err)
	}
	handler := cspMiddleware(mediaHandlerAt(root))
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, httptest.NewRequest("GET", "/media/a/0.html", nil))
	if got := response.Header().Values("Content-Security-Policy"); len(got) != 1 || got[0] != mediaContentSecurityPolicy {
		t.Fatalf("Content-Security-Policy = %q", got)
	}
	if response.Code != http.StatusOK {
		t.Fatalf("status = %d", response.Code)
	}
}
