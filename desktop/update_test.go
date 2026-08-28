package main

import (
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"
)

const testManifest = `{
  "version": "0.1.13",
  "pubDate": "2026-07-25T00:00:00Z",
  "platforms": {
    "linux-amd64": {
      "appimage": {"url": "https://example.test/meron.AppImage", "sha256": "0000000000000000000000000000000000000000000000000000000000000000", "size": 42},
      "tarball": {"url": "https://example.test/meron.tar.gz", "sha256": "1111111111111111111111111111111111111111111111111111111111111111", "size": 43}
    },
    "darwin-arm64": {
      "dmg": {"url": "https://example.test/meron.dmg", "sha256": "2222222222222222222222222222222222222222222222222222222222222222", "size": 44}
    }
  }
}`

func TestFetchUpdateManifest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasPrefix(r.Header.Get("User-Agent"), "Meron/") {
			t.Errorf("missing User-Agent, got %q", r.Header.Get("User-Agent"))
		}
		_, _ = w.Write([]byte(testManifest))
	}))
	defer server.Close()

	manifest, err := fetchUpdateManifest(server.URL)
	if err != nil {
		t.Fatalf("fetchUpdateManifest: %v", err)
	}
	if manifest.Version != "0.1.13" {
		t.Fatalf("Version = %q, want 0.1.13", manifest.Version)
	}

	asset, ok := manifest.assetFor("linux", "amd64", channelAppImage)
	if !ok {
		t.Fatal("no linux-amd64 appimage asset")
	}
	if asset.URL != "https://example.test/meron.AppImage" || asset.Size != 42 {
		t.Fatalf("unexpected asset: %+v", asset)
	}

	// Channels the manifest deliberately omits (snap, appx) must not resolve,
	// and neither must an unbuilt platform.
	if _, ok := manifest.assetFor("linux", "amd64", channelSnap); ok {
		t.Error("snap resolved to an asset; store builds must not self-update")
	}
	if _, ok := manifest.assetFor("linux", "arm64", channelAppImage); ok {
		t.Error("linux-arm64 resolved to an asset")
	}
}

func TestFetchUpdateManifestRejectsBadResponses(t *testing.T) {
	cases := []struct {
		name   string
		status int
		body   string
	}{
		{"not found", http.StatusNotFound, "nope"},
		{"not json", http.StatusOK, "<html>"},
		{"no version", http.StatusOK, `{"platforms":{}}`},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
				w.WriteHeader(tc.status)
				_, _ = w.Write([]byte(tc.body))
			}))
			defer server.Close()
			if _, err := fetchUpdateManifest(server.URL); err == nil {
				t.Fatal("expected an error")
			}
		})
	}
}

// An asset entry missing a field, or carrying a truncated digest, must be
// treated as absent rather than downloaded unverifiably.
func TestAssetForRejectsIncompleteEntries(t *testing.T) {
	manifest := &updateManifest{
		Version: "0.1.13",
		Platforms: map[string]map[string]updateAsset{
			"linux-amd64": {
				"noURL":    {SHA256: strings.Repeat("a", 64), Size: 10},
				"noSize":   {URL: "https://example.test/x", SHA256: strings.Repeat("a", 64)},
				"shortSum": {URL: "https://example.test/x", SHA256: "abc", Size: 10},
			},
		},
	}
	for _, kind := range []string{"noURL", "noSize", "shortSum"} {
		if _, ok := manifest.assetFor("linux", "amd64", kind); ok {
			t.Errorf("%s resolved to a usable asset", kind)
		}
	}
}

func TestDownloadToVerifiesPayload(t *testing.T) {
	payload := []byte("meron update payload")
	sum := sha256.Sum256(payload)
	digest := hex.EncodeToString(sum[:])

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write(payload)
	}))
	defer server.Close()

	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	u := newUpdater(&App{})

	t.Run("good payload lands in the cache dir", func(t *testing.T) {
		path, err := u.downloadTo(updateAsset{URL: server.URL + "/meron.AppImage", SHA256: digest, Size: int64(len(payload))})
		if err != nil {
			t.Fatalf("downloadTo: %v", err)
		}
		if filepath.Base(path) != "meron.AppImage" {
			t.Errorf("file name = %q, want meron.AppImage", filepath.Base(path))
		}
		got, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		if string(got) != string(payload) {
			t.Error("downloaded bytes differ from the served payload")
		}
	})

	t.Run("checksum mismatch is rejected and cleaned up", func(t *testing.T) {
		_, err := u.downloadTo(updateAsset{
			URL:    server.URL + "/meron.AppImage",
			SHA256: strings.Repeat("f", 64),
			Size:   int64(len(payload)),
		})
		if err == nil {
			t.Fatal("expected a checksum error")
		}
		if !strings.Contains(err.Error(), "checksum") {
			t.Fatalf("error = %v, want a checksum mismatch", err)
		}
		assertUpdateCacheEmpty(t)
	})

	t.Run("size mismatch is rejected and cleaned up", func(t *testing.T) {
		_, err := u.downloadTo(updateAsset{
			URL:    server.URL + "/meron.AppImage",
			SHA256: digest,
			Size:   int64(len(payload)) + 5,
		})
		if err == nil {
			t.Fatal("expected a size error")
		}
		assertUpdateCacheEmpty(t)
	})
}

func assertUpdateCacheEmpty(t *testing.T) {
	t.Helper()
	entries, err := os.ReadDir(updateCacheDir())
	if err != nil {
		if os.IsNotExist(err) {
			return
		}
		t.Fatal(err)
	}
	if len(entries) > 0 {
		t.Fatalf("rejected payload left %d file(s) behind", len(entries))
	}
}

func TestUpdateFileName(t *testing.T) {
	cases := map[string]string{
		"https://example.test/v0.1.13/meron-linux-amd64.AppImage": "meron-linux-amd64.AppImage",
		"https://example.test/meron.dmg?token=abc":                "meron.dmg",
		"https://example.test/":                                   "meron-update",
		// A path-traversal attempt in the manifest must not escape the cache dir.
		"https://example.test/../../etc/passwd": "passwd",
	}
	for url, want := range cases {
		if got := updateFileName(url); got != want {
			t.Errorf("updateFileName(%q) = %q, want %q", url, got, want)
		}
	}
}

// End-to-end over the real HTTP path: a manifest served like a GitHub release
// asset, a payload that hashes to what the manifest claims, and the state
// machine walked from idle to a staged, verified download.
func TestUpdaterCheckThenDownload(t *testing.T) {
	payload := []byte("pretend this is an AppImage")
	sum := sha256.Sum256(payload)

	mux := http.NewServeMux()
	server := httptest.NewServer(mux)
	defer server.Close()
	mux.HandleFunc("/meron.AppImage", func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write(payload)
	})
	mux.HandleFunc("/latest.json", func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`{
		  "version": "99.0.0",
		  "platforms": {"` + runtime.GOOS + `-` + runtime.GOARCH + `": {"appimage": {
		    "url": "` + server.URL + `/meron.AppImage",
		    "sha256": "` + hex.EncodeToString(sum[:]) + `",
		    "size": ` + strconv.Itoa(len(payload)) + `}}}
		}`))
	})

	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	t.Setenv("MERON_UPDATE_MANIFEST_URL", server.URL+"/latest.json")

	app := &App{}
	// Pretend we're running from an AppImage regardless of the test host.
	app.updateChannelOnce.Do(func() {
		app.updateChannelValue = updateChannel{Kind: channelAppImage, Target: filepath.Join(t.TempDir(), "Meron.AppImage")}
	})
	u := app.ensureUpdater()

	got, err := u.check()
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	after := got.(updateStatusPayload)
	if after.State != updateStateAvailable {
		t.Fatalf("state = %q (error %q), want %q", after.State, after.Error, updateStateAvailable)
	}
	if after.LatestVersion != "99.0.0" || !after.Supported {
		t.Fatalf("unexpected status: %+v", after)
	}

	if _, err := u.startDownload(); err != nil {
		t.Fatalf("startDownload: %v", err)
	}
	waitForUpdateState(t, u, updateStateReady)

	u.mu.Lock()
	staged := u.stagedPath
	u.mu.Unlock()
	content, err := os.ReadFile(staged)
	if err != nil {
		t.Fatalf("read staged payload: %v", err)
	}
	if string(content) != string(payload) {
		t.Fatal("staged payload differs from what the server served")
	}

	// Re-checking with the same version staged must not throw the download away.
	if _, err := u.check(); err != nil {
		t.Fatalf("re-check: %v", err)
	}
	u.mu.Lock()
	state, restaged := u.state, u.stagedPath
	u.mu.Unlock()
	if state != updateStateReady || restaged != staged {
		t.Fatalf("re-check discarded the staged update: state=%q path=%q", state, restaged)
	}

	// Neither must a re-check that can't reach the manifest at all.
	t.Setenv("MERON_UPDATE_MANIFEST_URL", "http://127.0.0.1:1/latest.json")
	if _, err := u.check(); err != nil {
		t.Fatalf("offline re-check: %v", err)
	}
	u.mu.Lock()
	state, restaged = u.state, u.stagedPath
	u.mu.Unlock()
	if state != updateStateReady || restaged != staged {
		t.Fatalf("offline re-check discarded the staged update: state=%q path=%q", state, restaged)
	}
}

// An up-to-date manifest leaves the updater idle and offers nothing to install.
func TestUpdaterCheckWhenCurrent(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`{"version": "0.0.1", "platforms": {"` + runtime.GOOS + `-` + runtime.GOARCH +
			`": {"appimage": {"url": "https://example.test/x", "sha256": "` + strings.Repeat("a", 64) + `", "size": 1}}}}`))
	}))
	defer server.Close()

	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	t.Setenv("MERON_UPDATE_MANIFEST_URL", server.URL)

	app := &App{}
	app.updateChannelOnce.Do(func() {
		app.updateChannelValue = updateChannel{Kind: channelAppImage, Target: "/tmp/Meron.AppImage"}
	})
	u := app.ensureUpdater()

	got, _ := u.check()
	after := got.(updateStatusPayload)
	if after.State != updateStateIdle {
		t.Fatalf("state = %q, want %q", after.State, updateStateIdle)
	}
	if _, err := u.startDownload(); err == nil {
		t.Fatal("startDownload succeeded with nothing to download")
	}
	if _, err := u.install(); err == nil {
		t.Fatal("install succeeded with nothing staged")
	}
}

// A store-managed build must not even reach the network.
func TestUpdaterCheckSkipsManagedChannel(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(_ http.ResponseWriter, _ *http.Request) {
		t.Error("managed build fetched the update manifest")
	}))
	defer server.Close()
	t.Setenv("MERON_UPDATE_MANIFEST_URL", server.URL)

	app := &App{}
	app.updateChannelOnce.Do(func() {
		app.updateChannelValue = updateChannel{Kind: channelSnap, Managed: true}
	})
	got, err := app.ensureUpdater().check()
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	after := got.(updateStatusPayload)
	if after.Supported || !after.Managed || after.State != updateStateIdle {
		t.Fatalf("unexpected status: %+v", after)
	}
}

func TestUpdaterCheckSkipsEnvironmentDisabledUpdates(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(_ http.ResponseWriter, _ *http.Request) {
		t.Error("environment-disabled build fetched the update manifest")
	}))
	defer server.Close()
	t.Setenv("MERON_UPDATE_MANIFEST_URL", server.URL)
	t.Setenv("MERON_DISABLE_SELF_UPDATE", "1")

	got, err := (&App{}).ensureUpdater().check()
	if err != nil {
		t.Fatalf("check: %v", err)
	}
	after := got.(updateStatusPayload)
	if after.Supported || !after.Managed || after.State != updateStateIdle {
		t.Fatalf("unexpected status: %+v", after)
	}
}

func waitForUpdateState(t *testing.T, u *updater, want string) {
	t.Helper()
	for i := 0; i < 200; i++ {
		u.mu.Lock()
		state, message := u.state, u.errMessage
		u.mu.Unlock()
		if state == want {
			return
		}
		if state == updateStateError {
			t.Fatalf("updater errored while waiting for %q: %s", want, message)
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf("timed out waiting for state %q", want)
}

func TestUpdateManifestSourceOverride(t *testing.T) {
	if got := updateManifestSource(); got != updateManifestURL {
		t.Fatalf("default source = %q, want %q", got, updateManifestURL)
	}
	t.Setenv("MERON_UPDATE_MANIFEST_URL", "http://127.0.0.1:8099/latest.json")
	if got := updateManifestSource(); got != "http://127.0.0.1:8099/latest.json" {
		t.Fatalf("override ignored, got %q", got)
	}
}
