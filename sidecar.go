package main

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

type Sidecar struct {
	path    string
	cmd     *exec.Cmd
	stdin   io.WriteCloser
	started bool
	mu      sync.Mutex
	nextID  uint64
	pending map[uint64]chan sidecarResponse
	cancel  context.CancelFunc
	stderr  io.Writer
	onEvent func(name string, detail any)
}

type sidecarResponse struct {
	Result any
	Error  any
}

func NewSidecar(path string, stderr io.Writer) *Sidecar {
	if stderr == nil {
		stderr = os.Stderr
	}
	return &Sidecar{path: path, stderr: stderr, pending: map[uint64]chan sidecarResponse{}}
}

func (s *Sidecar) Started() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.started
}

func (s *Sidecar) Start(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.started {
		return nil
	}
	runCtx, cancel := context.WithCancel(context.Background())
	cmd := exec.CommandContext(runCtx, s.path)
	cmd.Env = sidecarEnv()
	cmd.Stderr = s.stderr
	stdin, err := cmd.StdinPipe()
	if err != nil {
		cancel()
		return err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		cancel()
		return err
	}
	if err := cmd.Start(); err != nil {
		cancel()
		return err
	}
	s.cmd = cmd
	s.stdin = stdin
	s.cancel = cancel
	s.started = true
	go s.readLoop(ctx, stdout)
	go func() {
		_ = cmd.Wait()
		s.mu.Lock()
		s.started = false
		for id, ch := range s.pending {
			delete(s.pending, id)
			ch <- sidecarResponse{Error: "sidecar exited"}
		}
		s.mu.Unlock()
	}()
	return nil
}

func sidecarEnv() []string {
	// Pass storage/media paths explicitly so the Go bridge and Rust sidecar use
	// the same dev or production profile.
	env := append(os.Environ(),
		"MERON_CORE_DB="+filepath.Join(appConfigDir(), "meron.db"),
		"MERON_MEDIA_DIR="+mediaDir(),
	)
	// Pass the baked OAuth credentials down so the sidecar's refresh path can
	// read them from the environment without its own deobfuscation.
	if id := googleClientID(); id != "" {
		env = append(env, "MERON_GOOGLE_CLIENT_ID="+id)
	}
	if secret := googleClientSecret(); secret != "" {
		env = append(env, "MERON_GOOGLE_CLIENT_SECRET="+secret)
	}
	if id := outlookClientID(); id != "" {
		env = append(env, "MERON_OUTLOOK_CLIENT_ID="+id)
	}
	return env
}

func (s *Sidecar) Close() {
	s.mu.Lock()
	cancel := s.cancel
	cmd := s.cmd
	s.started = false
	s.mu.Unlock()
	if cancel != nil {
		cancel()
	}
	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
}

func (s *Sidecar) Call(method string, params any) (any, error) {
	timeout := sidecarCallTimeout(method)
	s.mu.Lock()
	if !s.started {
		s.mu.Unlock()
		return nil, errors.New("sidecar not started")
	}
	s.nextID++
	id := s.nextID
	ch := make(chan sidecarResponse, 1)
	s.pending[id] = ch
	req := map[string]any{"id": id, "method": method, "params": params}
	line, err := json.Marshal(req)
	if err != nil {
		delete(s.pending, id)
		s.mu.Unlock()
		return nil, err
	}
	line = append(line, '\n')
	start := time.Now()
	_, err = s.stdin.Write(line)
	s.mu.Unlock()
	if err != nil {
		return nil, err
	}
	select {
	case res := <-ch:
		if res.Error != nil {
			return nil, fmt.Errorf("sidecar %s error: %s", method, sidecarErrorMessage(res.Error))
		}
		return res.Result, nil
	case <-time.After(timeout):
		s.mu.Lock()
		delete(s.pending, id)
		s.mu.Unlock()
		return nil, fmt.Errorf("sidecar %s timeout after %s", method, time.Since(start).Round(time.Millisecond))
	}
}

func (s *Sidecar) readLoop(ctx context.Context, stdout io.Reader) {
	scanner := bufio.NewScanner(stdout)
	scanner.Buffer(make([]byte, 0, 64*1024), 16*1024*1024)
	for scanner.Scan() {
		var object map[string]any
		if err := json.Unmarshal(scanner.Bytes(), &object); err != nil {
			continue
		}
		if eventName, ok := object["event"].(string); ok {
			if ctx != nil {
				wailsRuntime.EventsEmit(ctx, eventName, object["detail"])
			}
			if s.onEvent != nil {
				s.onEvent(eventName, object["detail"])
			}
			continue
		}
		idFloat, ok := object["id"].(float64)
		if !ok {
			continue
		}
		id := uint64(idFloat)
		s.mu.Lock()
		ch := s.pending[id]
		delete(s.pending, id)
		s.mu.Unlock()
		if ch == nil {
			continue
		}
		if errValue, ok := object["error"]; ok {
			ch <- sidecarResponse{Error: errValue}
		} else {
			ch <- sidecarResponse{Result: object["result"]}
		}
	}
}

// coreBinaryPath returns the path to the Rust core engine sidecar, resolving it
// once and caching the result. Resolution order:
//
//  1. MERON_CORE_SERVER env var (explicit override; used in dev and custom installs)
//  2. the embedded sidecar, extracted to the cache dir (release builds)
//  3. the freshly built debug binary relative to the repo root (plain `wails dev`)
//
// Resolving relative to the working directory was the cause of the release
// binary showing the setup screen: launched outside the repo, it never found
// the sidecar, so account.list returned empty.
func coreBinaryPath() string {
	mailServerOnce.Do(func() {
		mailServerResolved = resolveMailServerPath()
	})
	return mailServerResolved
}

var (
	mailServerOnce     sync.Once
	mailServerResolved string
)

func resolveMailServerPath() string {
	if value := os.Getenv("MERON_CORE_SERVER"); value != "" {
		return value
	}
	if len(embeddedSidecar) > 0 {
		if path, err := extractEmbeddedSidecar(); err == nil {
			return path
		} else {
			fmt.Fprintf(os.Stderr, "meron: failed to extract embedded sidecar: %v\n", err)
		}
	}
	return "meron-core/target/debug/meron-core"
}

// extractEmbeddedSidecar writes the embedded sidecar to the cache dir under a
// content-hashed filename, so repeated launches reuse the same file and a new
// build lands in a new file without clobbering a running one. The write is
// atomic (temp file + rename) to survive concurrent or interrupted launches.
func extractEmbeddedSidecar() (string, error) {
	sum := sha256.Sum256(embeddedSidecar)
	name := "meron-core-" + hex.EncodeToString(sum[:8])
	dir := filepath.Join(appCacheDir(), "bin")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	path := filepath.Join(dir, name)
	if info, err := os.Stat(path); err == nil && info.Size() == int64(len(embeddedSidecar)) {
		return path, nil
	}
	tmp, err := os.CreateTemp(dir, name+".tmp-*")
	if err != nil {
		return "", err
	}
	tmpName := tmp.Name()
	if _, err := tmp.Write(embeddedSidecar); err != nil {
		tmp.Close()
		os.Remove(tmpName)
		return "", err
	}
	if err := tmp.Chmod(0o755); err != nil {
		tmp.Close()
		os.Remove(tmpName)
		return "", err
	}
	if err := tmp.Close(); err != nil {
		os.Remove(tmpName)
		return "", err
	}
	if err := os.Rename(tmpName, path); err != nil {
		os.Remove(tmpName)
		return "", err
	}
	return path, nil
}

func sidecarErrorMessage(value any) string {
	switch typed := value.(type) {
	case map[string]any:
		if message, ok := typed["message"].(string); ok {
			return message
		}
		if data, err := json.Marshal(typed); err == nil {
			return string(data)
		}
	case string:
		return typed
	}
	return fmt.Sprint(value)
}

func sidecarCallTimeout(method string) time.Duration {
	switch method {
	case "account.connect":
		return 45 * time.Second
	case "messages.read", "messages.thread", "messages.markRead", "messages.markAllRead", "send", "save_draft":
		return 30 * time.Second
	case "folders.list", "folders.create", "folders.archive", "messages.recent", "messages.sync", "rss.markRead", "watch.start", "watch.stop", "discard_draft", "account.addRss", "feed.add", "rss.importOpml":
		return 15 * time.Second
	default:
		return 5 * time.Second
	}
}
