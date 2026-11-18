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

// Decrypted attachments are served at /media/e2ee/<key> from the secure dir,
// through the same hardened handler as the plain cache, and the two roots
// never reach each other.
func TestMediaHandlerServesDecryptedFilesFromTheSecureDir(t *testing.T) {
	base := t.TempDir()
	t.Setenv("XDG_RUNTIME_DIR", filepath.Join(base, "run"))
	t.Setenv("XDG_CACHE_HOME", filepath.Join(base, "cache"))
	t.Setenv("HOME", filepath.Join(base, "home"))
	secure, plain := secureMediaDir(), mediaDir()
	write := func(root, name, body string) {
		path := filepath.Join(root, name)
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(body), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	write(secure, "acct/1.png", "decrypted png")
	write(secure, "acct/2.html", "<script>alert(1)</script>")
	write(secure, "acct/4.png", "decrypted only")
	write(plain, "acct/1.png", "plain png")
	write(plain, "acct/3.png", "plain only")
	// From the secure root up to the plain cache: a `..` that escaped the
	// secure root would land on the plain-only file.
	escape, err := filepath.Rel(secure, filepath.Join(plain, "acct/3.png"))
	if err != nil {
		t.Fatal(err)
	}
	handler := mediaHandler()

	for _, test := range []struct {
		name, path, body, contentType string
		status                        int
	}{
		{name: "decrypted image", path: "/media/e2ee/acct/1.png", status: 200, body: "decrypted png", contentType: "image/png"},
		{name: "decrypted html is opaque", path: "/media/e2ee/acct/2.html", status: 200, body: "<script>alert(1)</script>", contentType: "application/octet-stream"},
		{name: "plain key keeps the plain copy", path: "/media/acct/1.png", status: 200, body: "plain png", contentType: "image/png"},
		{name: "secure route can't reach the plain cache", path: "/media/e2ee/acct/3.png", status: 404},
		{name: "plain route can't reach decrypted files", path: "/media/acct/4.png", status: 404},
		{name: "traversal stays inside the secure dir", path: "/media/e2ee/" + filepath.ToSlash(escape), status: 404},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, httptest.NewRequest("GET", test.path, nil))
			if response.Code != test.status {
				t.Fatalf("status = %d, want %d", response.Code, test.status)
			}
			if test.status != 200 {
				return
			}
			if got := response.Body.String(); got != test.body {
				t.Errorf("body = %q, want %q", got, test.body)
			}
			header := response.Header()
			if got := header.Get("Content-Type"); got != test.contentType {
				t.Errorf("Content-Type = %q, want %q", got, test.contentType)
			}
			if got := header.Get("Content-Security-Policy"); got != mediaContentSecurityPolicy {
				t.Errorf("Content-Security-Policy = %q", got)
			}
		})
	}
}
