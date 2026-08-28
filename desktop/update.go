package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// The updater reads a manifest published as a release asset. GitHub serves
// `releases/latest/download/<asset>` as a redirect to the newest non-prerelease
// release, so the URL never changes and there is no API rate limit to hit.
const updateManifestURL = "https://github.com/nonbili/meron/releases/latest/download/latest.json"

// releasesPageURL is where the UI sends users whose channel can't self-update
// (a root-owned install, an unreadable DMG).
const releasesPageURL = "https://github.com/nonbili/meron/releases/latest"

// releaseTagURLPrefix builds the link to one desktop release. The repo also
// publishes `android/v*` tags, so once a known desktop version is in hand the
// exact tag beats the `releases/latest` pointer, which follows whichever
// release was published most recently regardless of platform.
const releaseTagURLPrefix = "https://github.com/nonbili/meron/releases/tag/v"

// releasesURLFor points at the specific release when the check succeeded, and
// falls back to the latest-release page when it didn't.
func releasesURLFor(version string) string {
	if version == "" {
		return releasesPageURL
	}
	return releaseTagURLPrefix + strings.TrimPrefix(version, "v")
}

const (
	updateStateIdle        = "idle"
	updateStateChecking    = "checking"
	updateStateAvailable   = "available"
	updateStateDownloading = "downloading"
	updateStateReady       = "ready"
	updateStateInstalling  = "installing"
	updateStateError       = "error"
)

// maxUpdatePayloadBytes caps a download. The largest asset today is the ~120 MB
// DMG; the ceiling exists so a bad manifest can't fill the user's disk.
const maxUpdatePayloadBytes = 512 << 20

// updateManifest is the shape of latest.json (see scripts/gen-latest-json.sh).
type updateManifest struct {
	Version string `json:"version"`
	PubDate string `json:"pubDate"`
	// Platforms is keyed "<goos>-<goarch>", then by channel kind.
	Platforms map[string]map[string]updateAsset `json:"platforms"`
}

type updateAsset struct {
	URL    string `json:"url"`
	SHA256 string `json:"sha256"`
	Size   int64  `json:"size"`
}

// updater owns the check/download/install state machine. Exactly one update can
// be in flight; the mutex guards every field.
type updater struct {
	app *App

	mu         sync.Mutex
	state      string
	latest     string
	pubDate    string
	errMessage string
	downloaded int64
	total      int64
	asset      updateAsset
	stagedPath string
	busy       bool
	lastEmit   time.Time
}

func newUpdater(app *App) *updater {
	return &updater{app: app, state: updateStateIdle}
}

// updateStatusPayload is what both the update.status command and the
// update.status event carry — the frontend replaces its state wholesale.
type updateStatusPayload struct {
	State          string `json:"state"`
	Channel        string `json:"channel"`
	Managed        bool   `json:"managed"`
	Supported      bool   `json:"supported"`
	CurrentVersion string `json:"currentVersion"`
	LatestVersion  string `json:"latestVersion"`
	PubDate        string `json:"pubDate"`
	Downloaded     int64  `json:"downloaded"`
	Total          int64  `json:"total"`
	Error          string `json:"error"`
	ReleasesURL    string `json:"releasesUrl"`
}

// status builds the payload. Callers must hold u.mu.
func (u *updater) statusLocked() updateStatusPayload {
	channel := u.app.updateChannel()
	return updateStatusPayload{
		State:          u.state,
		Channel:        channel.Kind,
		Managed:        channel.Managed,
		Supported:      channel.SelfUpdatable() && !u.app.updatesDisabled(),
		CurrentVersion: appVersion(),
		LatestVersion:  u.latest,
		PubDate:        u.pubDate,
		Downloaded:     u.downloaded,
		Total:          u.total,
		Error:          u.errMessage,
		ReleasesURL:    releasesURLFor(u.latest),
	}
}

// emitLocked pushes the current status to the webview. Download progress calls
// this on every chunk, so non-terminal states are throttled; state changes
// always go through (force).
func (u *updater) emitLocked(force bool) {
	if !force && time.Since(u.lastEmit) < 100*time.Millisecond {
		return
	}
	u.lastEmit = time.Now()
	if u.app.ctx != nil {
		wailsRuntime.EventsEmit(u.app.ctx, "update.status", u.statusLocked())
	}
}

func (u *updater) setErrorLocked(err error) {
	u.state = updateStateError
	u.errMessage = err.Error()
	u.busy = false
	u.emitLocked(true)
}

// ---------------------------------------------------------------- App wiring

func (a *App) updateChannel() updateChannel {
	a.updateChannelOnce.Do(func() { a.updateChannelValue = detectChannel() })
	return a.updateChannelValue
}

// updatesDisabled turns the updater off for `wails dev`, where the running
// binary lives in a build dir and replacing it would be meaningless.
func (a *App) updatesDisabled() bool {
	return a.ctx != nil && wailsRuntime.Environment(a.ctx).BuildType == "dev"
}

func (a *App) ensureUpdater() *updater {
	a.updaterOnce.Do(func() { a.updater = newUpdater(a) })
	return a.updater
}

func (a *App) updateStatus() (any, error) {
	u := a.ensureUpdater()
	u.mu.Lock()
	defer u.mu.Unlock()
	return u.statusLocked(), nil
}

func (a *App) updateCheck() (any, error) {
	return a.ensureUpdater().check()
}

func (a *App) updateDownload() (any, error) {
	return a.ensureUpdater().startDownload()
}

func (a *App) updateInstall() (any, error) {
	return a.ensureUpdater().install()
}

// --------------------------------------------------------------------- Check

func (u *updater) check() (any, error) {
	channel := u.app.updateChannel()
	u.mu.Lock()
	if u.app.updatesDisabled() || !channel.SelfUpdatable() {
		status := u.statusLocked()
		u.mu.Unlock()
		return status, nil
	}
	if u.busy || u.state == updateStateDownloading || u.state == updateStateInstalling {
		status := u.statusLocked()
		u.mu.Unlock()
		return status, nil
	}
	// A staged payload for a still-current version survives a re-check.
	if u.state != updateStateReady {
		u.state = updateStateChecking
		u.errMessage = ""
	}
	u.busy = true
	u.emitLocked(true)
	u.mu.Unlock()

	manifest, err := fetchUpdateManifest(updateManifestSource())

	u.mu.Lock()
	defer u.mu.Unlock()
	u.busy = false
	if err != nil {
		u.app.logf("update: check failed: %v", err)
		if u.state == updateStateReady {
			// A background re-check failing (offline, GitHub hiccup) must not
			// throw away a payload the user already downloaded.
			u.emitLocked(true)
			return u.statusLocked(), nil
		}
		u.setErrorLocked(err)
		return u.statusLocked(), nil
	}

	asset, ok := manifest.assetFor(runtime.GOOS, runtime.GOARCH, channel.Kind)
	if !ok {
		u.state = updateStateIdle
		u.latest = manifest.Version
		u.pubDate = manifest.PubDate
		u.errMessage = ""
		u.app.logf("update: manifest %s has no %s asset for %s-%s", manifest.Version, channel.Kind, runtime.GOOS, runtime.GOARCH)
		u.emitLocked(true)
		return u.statusLocked(), nil
	}

	u.latest = manifest.Version
	u.pubDate = manifest.PubDate
	u.errMessage = ""
	if compareVersions(manifest.Version, appVersion()) <= 0 {
		u.state = updateStateIdle
		u.asset = updateAsset{}
		u.discardStagedLocked()
	} else if u.state != updateStateReady || u.asset.SHA256 != asset.SHA256 {
		// A newer release landed while an older one was staged: throw the
		// stale payload away and offer the new one.
		u.discardStagedLocked()
		u.asset = asset
		u.total = asset.Size
		u.downloaded = 0
		u.state = updateStateAvailable
	}
	u.app.logf("update: current=%s latest=%s state=%s channel=%s", appVersion(), u.latest, u.state, channel.Kind)
	u.emitLocked(true)
	return u.statusLocked(), nil
}

// updateManifestSource allows pointing a local build at a test manifest.
func updateManifestSource() string {
	if override := strings.TrimSpace(os.Getenv("MERON_UPDATE_MANIFEST_URL")); override != "" {
		return override
	}
	return updateManifestURL
}

func fetchUpdateManifest(url string) (*updateManifest, error) {
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", updateUserAgent())
	req.Header.Set("Accept", "application/json")
	client := &http.Client{Timeout: 20 * time.Second}
	res, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("update manifest: unexpected status %d", res.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(res.Body, 1<<20))
	if err != nil {
		return nil, err
	}
	var manifest updateManifest
	if err := json.Unmarshal(body, &manifest); err != nil {
		return nil, fmt.Errorf("update manifest: %w", err)
	}
	if strings.TrimSpace(manifest.Version) == "" {
		return nil, errors.New("update manifest: missing version")
	}
	return &manifest, nil
}

func (m *updateManifest) assetFor(goos, goarch, kind string) (updateAsset, bool) {
	assets, ok := m.Platforms[goos+"-"+goarch]
	if !ok {
		return updateAsset{}, false
	}
	asset, ok := assets[kind]
	if !ok || asset.URL == "" || asset.Size <= 0 || len(asset.SHA256) != 64 {
		return updateAsset{}, false
	}
	return asset, true
}

func updateUserAgent() string {
	return "Meron/" + appVersion() + " (" + runtime.GOOS + "; " + runtime.GOARCH + ")"
}

// ------------------------------------------------------------------ Download

func (u *updater) startDownload() (any, error) {
	u.mu.Lock()
	defer u.mu.Unlock()
	if u.state == updateStateReady || u.state == updateStateDownloading {
		return u.statusLocked(), nil
	}
	if u.state != updateStateAvailable || u.asset.URL == "" {
		return u.statusLocked(), errors.New("no update available to download")
	}
	asset := u.asset
	u.state = updateStateDownloading
	u.errMessage = ""
	u.downloaded = 0
	u.total = asset.Size
	u.busy = true
	u.emitLocked(true)
	go u.download(asset)
	return u.statusLocked(), nil
}

func (u *updater) download(asset updateAsset) {
	path, err := u.downloadTo(asset)

	u.mu.Lock()
	defer u.mu.Unlock()
	u.busy = false
	if err != nil {
		u.app.logf("update: download failed: %v", err)
		u.setErrorLocked(err)
		return
	}
	u.stagedPath = path
	u.state = updateStateReady
	u.downloaded = asset.Size
	u.app.logf("update: staged %s", path)
	u.emitLocked(true)
}

// downloadTo streams the asset to the update cache dir, hashing as it goes, and
// only returns a path once size and digest both match the manifest.
func (u *updater) downloadTo(asset updateAsset) (string, error) {
	dir := updateCacheDir()
	// One payload at a time: wipe anything left over from a previous attempt.
	if err := os.RemoveAll(dir); err != nil {
		return "", err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	if asset.Size > maxUpdatePayloadBytes {
		return "", fmt.Errorf("update payload too large: %d bytes", asset.Size)
	}

	req, err := http.NewRequest(http.MethodGet, asset.URL, nil)
	if err != nil {
		return "", err
	}
	req.Header.Set("User-Agent", updateUserAgent())
	// No overall timeout: a 120 MB DMG on a slow line legitimately takes a
	// while. The transport still bounds connect/TLS/header time.
	client := &http.Client{
		Transport: &http.Transport{
			TLSHandshakeTimeout:   30 * time.Second,
			ResponseHeaderTimeout: 60 * time.Second,
		},
	}
	res, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer res.Body.Close()
	if res.StatusCode != http.StatusOK {
		return "", fmt.Errorf("update download: unexpected status %d", res.StatusCode)
	}

	path := filepath.Join(dir, updateFileName(asset.URL))
	file, err := os.OpenFile(path, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o755)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	written, copyErr := io.Copy(io.MultiWriter(file, hasher), &progressReader{
		reader:  io.LimitReader(res.Body, asset.Size+1),
		updater: u,
	})
	closeErr := file.Close()
	if copyErr != nil {
		os.RemoveAll(dir)
		return "", copyErr
	}
	if closeErr != nil {
		os.RemoveAll(dir)
		return "", closeErr
	}
	if written != asset.Size {
		os.RemoveAll(dir)
		return "", fmt.Errorf("update download: expected %d bytes, got %d", asset.Size, written)
	}
	if digest := hex.EncodeToString(hasher.Sum(nil)); !strings.EqualFold(digest, asset.SHA256) {
		os.RemoveAll(dir)
		return "", fmt.Errorf("update download: checksum mismatch (expected %s, got %s)", asset.SHA256, digest)
	}
	return path, nil
}

// progressReader reports bytes read back to the updater so the UI can draw a
// progress bar. Emission is throttled inside emitLocked.
type progressReader struct {
	reader  io.Reader
	updater *updater
}

func (p *progressReader) Read(buf []byte) (int, error) {
	n, err := p.reader.Read(buf)
	if n > 0 {
		p.updater.mu.Lock()
		p.updater.downloaded += int64(n)
		p.updater.emitLocked(false)
		p.updater.mu.Unlock()
	}
	return n, err
}

func updateCacheDir() string {
	return filepath.Join(appCacheDir(), "updates")
}

// updateFileName keeps the asset's own extension — the install step dispatches
// on it (.dmg mounts, .tar.gz extracts, .exe runs).
func updateFileName(rawURL string) string {
	parsed, err := url.Parse(rawURL)
	if err != nil {
		return "meron-update"
	}
	// path.Base, not filepath.Base: URL paths use "/" on every OS. Taking the
	// base also keeps a "../" in a hostile manifest from escaping the cache dir.
	name := path.Base(strings.ReplaceAll(parsed.Path, "\\", "/"))
	if name == "" || name == "." || name == "/" || name == ".." {
		return "meron-update"
	}
	return name
}

// discardStagedLocked drops a staged payload and its cache dir.
func (u *updater) discardStagedLocked() {
	if u.stagedPath == "" {
		return
	}
	u.stagedPath = ""
	_ = os.RemoveAll(updateCacheDir())
}

// cleanupUpdateCache removes leftovers from a previous run. Called on startup:
// a payload that survived a restart has either been installed already or was
// abandoned, and either way it is re-downloadable.
func cleanupUpdateCache() {
	_ = os.RemoveAll(updateCacheDir())
}

// ------------------------------------------------------------- Install shared

// ensureWritableDir checks that we can create files in dir, which is what every
// install strategy needs before it starts moving things around. A root-owned
// /opt or /Applications gives a clear error up front instead of a half-applied
// update.
func ensureWritableDir(dir string) error {
	probe, err := os.CreateTemp(dir, ".meron-write-probe-")
	if err != nil {
		return err
	}
	name := probe.Name()
	_ = probe.Close()
	return os.Remove(name)
}

func copyFile(source, dest string, mode os.FileMode) error {
	in, err := os.Open(source)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.OpenFile(dest, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, mode)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}

// ------------------------------------------------------------------- Install

func (u *updater) install() (any, error) {
	u.mu.Lock()
	if u.state != updateStateReady || u.stagedPath == "" {
		status := u.statusLocked()
		u.mu.Unlock()
		return status, errors.New("no downloaded update to install")
	}
	channel := u.app.updateChannel()
	staged := u.stagedPath
	u.state = updateStateInstalling
	u.errMessage = ""
	u.emitLocked(true)
	u.mu.Unlock()

	u.app.logf("update: installing %s via %s", staged, channel.Kind)
	if err := applyUpdate(channel, staged); err != nil {
		u.app.logf("update: install failed: %v", err)
		u.mu.Lock()
		defer u.mu.Unlock()
		// Keep the payload: the user can retry, or open it by hand.
		u.state = updateStateReady
		u.errMessage = err.Error()
		u.emitLocked(true)
		return u.statusLocked(), nil
	}

	u.mu.Lock()
	status := u.statusLocked()
	u.mu.Unlock()
	// applyUpdate has queued the relaunch; quitting releases the single-instance
	// lock so the new copy can start.
	if u.app.ctx != nil {
		wailsRuntime.Quit(u.app.ctx)
	}
	return status, nil
}
