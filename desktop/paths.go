package main

import (
	"bytes"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"io"
	"log"
	"os"
	"path/filepath"
	"strings"
)

func decode(payload map[string]any, out any) error {
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	return decoder.Decode(out)
}

// accountID derives the stable primary key for a mail account from its email.
// The normalized address is used verbatim: a SQLite TEXT key accepts any
// character, and keeping the address intact avoids collisions (an earlier
// scheme that mapped @ . + _ all to '-' folded a.b@x.com and a-b@x.com onto the
// same id). Mail ids always contain '@', so they never collide with the
// "rss-" namespace minted by the sidecar (see isRSSAccountID).
func accountID(email string) string {
	return strings.ToLower(strings.TrimSpace(email))
}

// tlsMode maps a port to a transport security choice. With TLS enabled, the
// standard submission/IMAPS ports (993, 465) use implicit TLS, the cleartext
// submission ports (143, 25, 587) upgrade via STARTTLS, and the local test
// ports stay plaintext. Returns (implicitTLS, starttls); at most one is true.
func tlsMode(tls bool, port uint16) (bool, bool) {
	if !tls {
		return false, false
	}
	switch port {
	case 3143, 3587:
		return false, false
	case 143, 25, 587:
		return false, true
	default:
		return true, false
	}
}

func randomBase64URL(length int) string {
	buf := make([]byte, length)
	_, _ = rand.Read(buf)
	return base64.RawURLEncoding.EncodeToString(buf)
}

func loadDotEnvLocal() {
	data, err := os.ReadFile(".env.local")
	if err != nil {
		return
	}
	for _, rawLine := range strings.Split(string(data), "\n") {
		line := strings.TrimSpace(rawLine)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		line = strings.TrimPrefix(line, "export ")
		key, value, ok := strings.Cut(line, "=")
		if !ok || os.Getenv(strings.TrimSpace(key)) != "" {
			continue
		}
		value = strings.Trim(strings.TrimSpace(value), `"'`)
		_ = os.Setenv(strings.TrimSpace(key), value)
	}
}

// secureMediaPrefix marks media keys of decrypted (end-to-end encrypted)
// mail. The sidecar writes those files under secureMediaDir, never in the
// plain attachment cache (meron-core parse::SECURE_MEDIA_PREFIX).
const secureMediaPrefix = "e2ee/"

// secureMediaDir holds attachments of decrypted mail. It is private to this
// user and the same path in every run, so the wipe before the sidecar starts
// (prepareSecureMedia) also clears what a crash or kill left behind. Meron
// empties it at start and at quit, so decrypted files exist only while it runs
// (after a crash, until the next start); a missing file makes the sidecar fetch
// and decrypt the message again.
//   - Linux desktop session: $XDG_RUNTIME_DIR (tmpfs, owner-only, per-user).
//   - Otherwise (no logind session on Linux; macOS; Windows): the per-user
//     cache dir, which no other local user can write. Not the shared temp dir:
//     a fixed name there could be read or pre-planted by another local user,
//     and a random one (as before) can't be found again after a crash.
func secureMediaDir() string {
	if runtime := os.Getenv("XDG_RUNTIME_DIR"); runtime != "" {
		return filepath.Join(runtime, appDirName(), "secure-media")
	}
	return filepath.Join(appCacheDir(), "secure-media")
}

// prepareSecureMedia empties the secure dir (what an earlier run left) and
// recreates it owner-only (0700). The wipe removes whatever stood there, a
// planted symlink included (never what it points to), so the dir is always
// created fresh. Called before the sidecar starts.
func prepareSecureMedia() error {
	wipeSecureMedia()
	return os.MkdirAll(secureMediaDir(), 0o700)
}

// wipeSecureMedia removes every decrypted attachment, and the copies opened
// from them. Called before the sidecar starts and after it stops. Errors are
// ignored (on Windows a file another app holds open can't go): the next start
// tries again.
func wipeSecureMedia() {
	_ = os.RemoveAll(secureMediaDir())
}

// mediaDir is where the mail sidecar writes attachment files and the asset
// handler serves them from at `/media/<key>`. Both sides must agree, so the
// bridge sets MERON_MEDIA_DIR on the sidecar to this exact path.
func mediaDir() string {
	return filepath.Join(appCacheDir(), "attachments")
}

// storageUsage reports disk used by the clearable attachment cache and by the
// SQLite DB. They are returned separately so the UI can offer to clear the
// cache while making clear the DB (the user's mail) is not touched here.
func (a *App) storageUsage() (any, error) {
	return map[string]any{
		"cacheBytes": dirSize(mediaDir()),
		"dbBytes":    fileSize(filepath.Join(appConfigDir(), "meron.db")),
		// Local folders: the only copy of that mail, never cleared here.
		"archiveBytes": fileSize(filepath.Join(appConfigDir(), "archive.db")),
	}, nil
}

// storageClearCache removes refetchable attachment media while preserving open
// document sessions and the database.
func (a *App) storageClearCache() (any, error) {
	dir := mediaDir()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	for _, entry := range entries {
		// External reader media is the window's working copy, not a refetchable
		// mailbox cache. Its owner removes it when the reader closes.
		if entry.Name() == "_eml" {
			continue
		}
		if err := os.RemoveAll(filepath.Join(dir, entry.Name())); err != nil {
			return nil, err
		}
	}
	return a.storageUsage()
}

// dirSize sums the sizes of all regular files under root. A missing dir is 0.
func dirSize(root string) int64 {
	var total int64
	_ = filepath.WalkDir(root, func(_ string, d os.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return nil
		}
		if info, err := d.Info(); err == nil {
			total += info.Size()
		}
		return nil
	})
	return total
}

// fileSize returns the size of a single file, or 0 if it is missing.
func fileSize(path string) int64 {
	if info, err := os.Stat(path); err == nil {
		return info.Size()
	}
	return 0
}

func appConfigDir() string {
	return filepath.Join(configHome(), appDirName())
}

func appCacheDir() string {
	return filepath.Join(cacheHome(), appDirName())
}

func appDirName() string {
	if isWailsDevLaunch() {
		return "meron-dev"
	}
	return "meron"
}

func appTitle() string {
	if isWailsDevLaunch() {
		return "Meron-dev"
	}
	return "Meron"
}

func appUniqueID() string {
	if isWailsDevLaunch() {
		return "jp.nonbili.meron-dev"
	}
	return "jp.nonbili.meron"
}

func isWailsDevLaunch() bool {
	return os.Getenv("FRONTEND_DEVSERVER_URL") != "" || os.Getenv("devserver") != "" || os.Getenv("frontenddevserverurl") != ""
}

func cacheHome() string {
	if dir, err := os.UserCacheDir(); err == nil {
		return dir
	}
	return "."
}

func configHome() string {
	if dir, err := os.UserConfigDir(); err == nil {
		return dir
	}
	return "."
}

// maxLogBytes caps the on-disk log. Rotation happens once, at open: if the file
// is over the cap it's renamed to meron.log.1 (replacing any prior .1) and a fresh
// file is started. A desktop app restarts often enough that open-time rotation is
// sufficient to keep the log from growing without bound.
const maxLogBytes = 5 << 20

func openAppLog() (*log.Logger, *os.File) {
	path := filepath.Join(appConfigDir(), "meron.log")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return log.New(os.Stderr, "meron: ", log.LstdFlags|log.Lmicroseconds), nil
	}
	if info, err := os.Stat(path); err == nil && info.Size() > maxLogBytes {
		_ = os.Rename(path, path+".1")
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return log.New(os.Stderr, "meron: ", log.LstdFlags|log.Lmicroseconds), nil
	}
	return log.New(logSinks{os.Stderr, file}, "meron: ", log.LstdFlags|log.Lmicroseconds), file
}

func (a *App) logWriter() io.Writer {
	if a.logFile == nil {
		return os.Stderr
	}
	return logSinks{os.Stderr, a.logFile}
}

func (a *App) logf(format string, args ...any) {
	if a.logger != nil {
		a.logger.Printf(format, args...)
	}
}

func fileExists(path string) bool {
	if path == "" {
		return false
	}
	_, err := os.Stat(path)
	return err == nil
}

func jsonString(value any) string {
	out, _ := value.(string)
	return out
}

func jsonBool(value any) bool {
	out, _ := value.(bool)
	return out
}

func jsonNumber(value any) int64 {
	switch v := value.(type) {
	case float64:
		return int64(v)
	case json.Number:
		n, _ := v.Int64()
		return n
	case int64:
		return v
	default:
		return 0
	}
}
