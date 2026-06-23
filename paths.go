package main

import (
	"bytes"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"io"
	"log"
	"net/http"
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

// mediaHandler serves attachment files written by the sidecar at `/media/<key>`,
// so image bytes load straight into the webview instead of round-tripping as
// base64 through the JSON bridge. The AssetServer tries embedded frontend assets
// first and only falls through here for unmatched paths. http.Dir cleans the
// path and blocks `..` traversal out of the media root.
func mediaHandler() http.Handler {
	fs := http.StripPrefix("/media/", http.FileServer(http.Dir(mediaDir())))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasPrefix(r.URL.Path, "/media/") {
			http.NotFound(w, r)
			return
		}
		fs.ServeHTTP(w, r)
	})
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
	}, nil
}

// storageClearCache deletes the attachment cache and recreates the empty dir so
// the sidecar can keep writing. It never touches the DB.
func (a *App) storageClearCache() (any, error) {
	dir := mediaDir()
	if err := os.RemoveAll(dir); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
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

func appUniqueID() string {
	if isWailsDevLaunch() {
		return "jp.nonbili.meron-dev"
	}
	return "jp.nonbili.meron"
}

func isWailsDevLaunch() bool {
	return os.Getenv("devserver") != "" || os.Getenv("frontenddevserverurl") != ""
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
	return log.New(io.MultiWriter(os.Stderr, file), "meron: ", log.LstdFlags|log.Lmicroseconds), file
}

func (a *App) logWriter() io.Writer {
	if a.logFile == nil {
		return os.Stderr
	}
	return io.MultiWriter(os.Stderr, a.logFile)
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
