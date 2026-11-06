package main

import (
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
	"meron/internal/desktopruntime"
)

type emlReaderWindow interface {
	Show() application.Window
	Focus()
	Close()
}

var newEmlReaderWindow = func(options application.WebviewWindowOptions, closed func()) emlReaderWindow {
	window := desktopruntime.App.Window.NewWithOptions(options)
	window.OnWindowEvent(events.Common.WindowClosing, func(_ *application.WindowEvent) { closed() })
	return window
}

type emlReader struct {
	path   string
	data   any
	window emlReaderWindow
}

// emlPath accepts command-line paths and local file URLs, including escaped
// spaces. Canonical paths let another launch focus an already-open document.
func emlPath(raw string) (string, error) {
	if strings.HasPrefix(strings.ToLower(raw), "file:") {
		u, err := url.Parse(raw)
		if err != nil || (u.Host != "" && u.Host != "localhost") {
			return "", errors.New("invalid local file URL")
		}
		raw = filepath.FromSlash(u.Path)
		if filepath.Separator == '\\' && len(raw) > 2 && raw[0] == '\\' && raw[2] == ':' {
			raw = raw[1:]
		}
	}
	if !strings.EqualFold(filepath.Ext(raw), ".eml") {
		return "", errors.New("choose an .eml file")
	}
	path, err := filepath.Abs(raw)
	if err != nil {
		return "", err
	}
	path, err = filepath.EvalSymlinks(path)
	if err != nil {
		return "", err
	}
	info, err := os.Stat(path)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", errors.New("EML document must be a regular file")
	}
	if info.Size() > 100*1024*1024 {
		return "", errors.New("EML file exceeds 100 MiB")
	}
	return path, nil
}

func isEmlArgument(raw string) bool {
	if u, err := url.Parse(raw); err == nil && u.Scheme != "" {
		if strings.EqualFold(u.Scheme, "file") {
			raw = u.Path
		} else if strings.EqualFold(u.Scheme, "mailto") || strings.Contains(raw, "://") {
			// Preserve subject-derived filenames such as "Re: Budget.eml".
			// Reject mailto links and hierarchical URLs that end in .eml.
			return false
		}
	}
	return strings.EqualFold(filepath.Ext(raw), ".eml")
}

func emlArgumentsAt(args []string, workingDir string) []string {
	resolved := append([]string(nil), args...)
	for i, arg := range resolved {
		if isEmlArgument(arg) && !filepath.IsAbs(arg) && !strings.HasPrefix(strings.ToLower(arg), "file:") {
			resolved[i] = filepath.Join(workingDir, arg)
		}
	}
	return resolved
}

func (a *App) openEml(path string) (any, error) {
	path, err := emlPath(path)
	if err != nil {
		return nil, err
	}
	// Serialize opens without holding the registry mutex across native calls.
	a.emlOpenMu.Lock()
	defer a.emlOpenMu.Unlock()
	a.emlMu.Lock()
	var existing *emlReader
	for _, reader := range a.emlReaders {
		if reader.path == path {
			existing = reader
			break
		}
	}
	a.emlMu.Unlock()
	if existing != nil {
		existing.window.Show()
		existing.window.Focus()
		return map[string]any{"opened": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	root := filepath.Join(mediaDir(), "_eml")
	if err := os.MkdirAll(root, 0o700); err != nil {
		return nil, err
	}
	dir, err := os.MkdirTemp(root, "reader-")
	if err != nil {
		return nil, err
	}
	token := filepath.Base(dir)
	// Read the OS-granted file in the parent process: a sandboxed sidecar
	// may not share a file-picker grant received after it was launched.
	snapshot := filepath.Join(dir, "source.eml")
	if err := snapshotEml(path, snapshot); err != nil {
		_ = os.RemoveAll(dir)
		return nil, err
	}
	data, err := a.sidecar.Call("eml.read", map[string]any{"path": snapshot, "session": token})
	_ = os.Remove(snapshot)
	if err != nil {
		_ = os.RemoveAll(dir)
		return nil, err
	}
	reader := &emlReader{path: path, data: data}
	a.emlMu.Lock()
	if a.emlReaders == nil {
		a.emlReaders = make(map[string]*emlReader)
	}
	a.emlReaders[token] = reader
	a.emlMu.Unlock()
	// Native decoration gives document windows their own controls and geometry,
	// independent of the main window's integrated title bar and close-to-tray.
	window := newEmlReaderWindow(application.WebviewWindowOptions{
		Name: token, Title: filepath.Base(path) + " — " + appTitle(),
		URL: "/?eml=" + url.QueryEscape(token), Width: 840, Height: 720,
		MinWidth: 480, MinHeight: 360, Hidden: true,
	}, func() {
		a.emlMu.Lock()
		delete(a.emlReaders, token)
		a.emlMu.Unlock()
		_ = os.RemoveAll(dir)
	})
	a.emlMu.Lock()
	reader.window = window
	a.emlMu.Unlock()
	window.Show()
	window.Focus()
	return map[string]any{"opened": true}, nil
}

func (a *App) emlCommand(command string, payload map[string]any) (any, error) {
	if command == "eml.open" {
		var parent *application.WebviewWindow
		if token, _ := payload["session"].(string); token != "" {
			a.emlMu.Lock()
			if reader := a.emlReaders[token]; reader != nil {
				parent, _ = reader.window.(*application.WebviewWindow)
			}
			a.emlMu.Unlock()
		}
		path, err := openFileDialog(a.ctx, desktopruntime.OpenDialogOptions{
			Window: parent, Title: "Open EML file", Filters: []desktopruntime.FileFilter{{DisplayName: "Email message (*.eml)", Pattern: "*.eml"}},
		})
		if err != nil {
			return nil, err
		}
		if path == "" {
			return map[string]any{"opened": false}, nil
		}
		return a.openEml(path)
	}
	token, _ := payload["session"].(string)
	a.emlMu.Lock()
	reader := a.emlReaders[token]
	var window emlReaderWindow
	if reader != nil {
		window = reader.window
	}
	a.emlMu.Unlock()
	if reader == nil {
		return nil, errors.New("EML reader is closed")
	}
	switch command {
	case "eml.get":
		return reader.data, nil
	case "eml.close":
		if window != nil {
			window.Close()
		}
		return nil, nil
	case "eml.saveAttachment":
		key, _ := payload["key"].(string)
		// Only an attachment of this document may be saved through its window.
		filename, ok := emlAttachmentName(reader.data, key)
		if !ok {
			return nil, errors.New("attachment does not belong to this EML document")
		}
		src, err := mediaFilePath(key)
		if err != nil {
			return nil, err
		}
		dest, err := saveFileDialog(a.ctx, desktopruntime.SaveDialogOptions{
			Title: "Save attachment", DefaultFilename: safeFilename(filename), CanCreateDirectories: true, Window: nativeEmlWindow(window),
		})
		if err != nil {
			return nil, err
		}
		if dest == "" {
			return map[string]any{"saved": false}, nil
		}
		if err := copyFileTo(src, dest, 0o666); err != nil {
			return nil, err
		}
		return map[string]any{"saved": true}, nil
	}
	return nil, fmt.Errorf("unknown EML command: %s", command)
}

func emlAttachmentName(data any, key string) (string, bool) {
	message, _ := data.(map[string]any)
	attachments, _ := message["attachments"].([]any)
	for _, raw := range attachments {
		attachment, _ := raw.(map[string]any)
		if key != "" && attachment["key"] == key {
			name, _ := attachment["filename"].(string)
			return name, true
		}
	}
	return "", false
}

// OS events can arrive before the sidecar starts. Queue them until Startup has
// finished, then use the same path as launches forwarded by single-instance.
func (a *App) openEmlArguments(args []string) bool {
	found := false
	for _, raw := range args {
		if !isEmlArgument(raw) {
			continue
		}
		found = true
		a.emlMu.Lock()
		ready := a.emlReady
		if !ready {
			a.pendingEml = append(a.pendingEml, raw)
		}
		a.emlMu.Unlock()
		if ready {
			a.openEmlFromOS(raw)
		}
	}
	return found
}

func (a *App) openEmlFromOS(path string) {
	if _, err := a.openEml(path); err != nil {
		a.logf("open EML failed: %v", err)
		desktopruntime.App.Dialog.Error().SetTitle("Open EML file").SetMessage(err.Error()).Show()
	}
}

func (a *App) startEmlReaders() {
	a.emlMu.Lock()
	a.emlReady = true
	pending := append([]string(nil), a.pendingEml...)
	a.pendingEml = nil
	a.emlMu.Unlock()
	go func() {
		a.openEmlArguments(os.Args[1:])
		for _, path := range pending {
			a.openEmlFromOS(path)
		}
	}()
}

// Snapshot at most 100 MiB even if the original file grows during the copy.
func snapshotEml(src, dest string) error {
	input, err := os.Open(src)
	if err != nil {
		return err
	}
	defer input.Close()
	output, err := os.OpenFile(dest, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	count, copyErr := io.Copy(output, io.LimitReader(input, 100*1024*1024+1))
	closeErr := output.Close()
	if copyErr != nil {
		return copyErr
	}
	if count > 100*1024*1024 {
		return errors.New("EML file exceeds 100 MiB")
	}
	return closeErr
}

func nativeEmlWindow(window emlReaderWindow) *application.WebviewWindow {
	native, _ := window.(*application.WebviewWindow)
	return native
}
